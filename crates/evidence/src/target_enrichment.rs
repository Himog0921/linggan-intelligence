//! COLLECTION-001 · 用接纳的博主资料创建或补全观察目标。
//!
//! 采集与观察目标此前是两条不相交的线：插件采到的博主资料进了语料库，观察目标列表却
//! 还只有一个平台 ID。人看着一串十六进制，认不出那是谁。
//!
//! 这里做的事很窄：**一个 `author_profile` 采集包被接纳后，按其中的平台稳定 ID 创建或
//! 补全同一个观察目标**。归属靠平台 ID——URL 会变、名字会改，只有平台 ID 是身份。
//!
//! **只写页面上已经可见的公开事实**（头像、简介、粉丝数…）。缺的字段保持缺失，不补 0、
//! 不用旧值顶替：能力登记表明确记着 `userPageData` 可能整个拿不到，那时粉丝数是真的
//! 「不知道」，而不是「0」。

use crate::{CollectionTargetError, StoreOutcome, store_pending_target};
use linggan_contracts::{CollectionContractError, TargetIdentity, TargetSource};
use linggan_storage_postgres::Database;
use serde::Serialize;
use serde_json::{Value, json};
use thiserror::Error;
use uuid::Uuid;

/// The target-side result of an accepted author package.  This is deliberately independent of
/// the Package Receipt: a Package is Evidence ingress; a target is an observation intent read
/// model derived from that already accepted public profile.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "state"
)]
pub enum TargetSyncOutcome {
    NotApplicable { reason: &'static str },
    Created { target_ref: Uuid },
    Updated { target_ref: Uuid },
}

#[derive(Debug, Error)]
pub enum TargetEnrichmentError {
    #[error(transparent)]
    Target(#[from] CollectionTargetError),
    #[error(transparent)]
    Contract(#[from] CollectionContractError),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

/// 从一个已接纳的 `author_profile` 采集包里取出公开资料，创建或补全对应的观察目标。
///
/// The unique constraint remains the authority for concurrent submissions.  An existing target
/// is enriched field-wise, so an incomplete later collector response cannot erase an already
/// observed avatar, biography, or count.  A target creation is not an Acquisition Request and
/// does not authorize any platform access.
pub async fn sync_target_from_author_profile(
    database: &Database,
    package_kind: &str,
    platform: &str,
    records: &[Value],
) -> Result<TargetSyncOutcome, TargetEnrichmentError> {
    if package_kind != "author_profile" {
        return Ok(TargetSyncOutcome::NotApplicable {
            reason: "package_kind_not_author_profile",
        });
    }
    // Observation targets currently only have an XHS contract.  A valid non-XHS Evidence
    // package must not be turned into a permanent retry loop merely because target monitoring
    // for that platform has not been opened.
    if platform != "xhs" {
        return Ok(TargetSyncOutcome::NotApplicable {
            reason: "target_platform_not_supported",
        });
    }
    let Some(author) = records.first().and_then(|record| record.get("payload")) else {
        return Ok(TargetSyncOutcome::NotApplicable {
            reason: "author_payload_missing",
        });
    };
    // 平台 ID 才是身份。取不到就不猜——宁可让目标停在只有 ID 的样子。
    let Some(identity_key) = text(author, "userId").or_else(|| text(author, "platformAuthorId"))
    else {
        return Ok(TargetSyncOutcome::NotApplicable {
            reason: "author_identity_missing",
        });
    };

    let facts = public_facts(author);
    let display_name = text(author, "name");
    let identity = TargetIdentity::creator(platform, &identity_key)?;
    let facts = facts
        .as_object()
        .filter(|facts| !facts.is_empty())
        .map(|_| &facts);
    let (target, store_outcome) = store_pending_target(
        database,
        &identity,
        TargetSource::PluginPush,
        display_name.as_deref(),
        facts,
        // 插件不知道业务领域，也不该猜：它只建立候选目标，之后由人在观察目标页明确分配。
        None,
    )
    .await?;

    if store_outcome == StoreOutcome::Stored {
        return Ok(TargetSyncOutcome::Created {
            target_ref: target.target_ref,
        });
    }

    // JSONB concatenation retains fields omitted by this collector run and only replaces facts
    // that this immutable package actually observed.  It never manufactures zero/empty values.
    let fact_patch = facts.cloned().unwrap_or_else(|| json!({}));
    sqlx::query(
        "UPDATE collection_observation_target \
         SET identity_facts = CASE WHEN $2::jsonb = '{}'::jsonb THEN identity_facts \
                                   ELSE coalesce(identity_facts, '{}'::jsonb) || $2::jsonb END, \
             display_name = coalesce($3, display_name) \
         WHERE target_ref = $1",
    )
    .bind(target.target_ref)
    .bind(fact_patch)
    .bind(display_name.as_deref())
    .execute(database.pool())
    .await?;
    Ok(TargetSyncOutcome::Updated {
        target_ref: target.target_ref,
    })
}

/// 页面上已经可见的公开资料。
///
/// **缺失的字段整个不写进去**，而不是写成 0 或空串：一个写着「粉丝 0」的档案，与一个
/// 写着「粉丝未知」的档案，对判断这个博主值不值得看是完全不同的两件事。
fn public_facts(author: &Value) -> Value {
    let mut facts = serde_json::Map::new();
    for key in ["avatar", "description", "redId", "ipLocation", "profileUrl"] {
        if let Some(value) = text(author, key) {
            facts.insert(key.to_owned(), json!(value));
        }
    }
    for key in ["fans", "follows", "interactions"] {
        if let Some(value) = count(author, key) {
            facts.insert(key.to_owned(), json!(value));
        }
    }
    Value::Object(facts)
}

fn text(author: &Value, key: &str) -> Option<String> {
    author
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

/// 计数字段可能是数字，也可能是「1.2万」这样的页面文本。
///
/// **只接受能确定为整数的值**。「1.2万」这类文本不在这里猜算成 12000——那是解释，不是
/// 观察；真要做换算，得由一个知道各平台写法的地方统一做，并且留下它换算过的痕迹。
fn count(author: &Value, key: &str) -> Option<i64> {
    match author.get(key) {
        Some(Value::Number(number)) => number.as_i64(),
        Some(Value::String(text)) => text.trim().parse::<i64>().ok(),
        _ => None,
    }
}
