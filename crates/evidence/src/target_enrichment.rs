//! COLLECTION-001 · 用采回来的博主信息补全观察目标。
//!
//! 采集与观察目标此前是两条不相交的线：插件采到的博主资料进了语料库，观察目标列表却
//! 还只有一个平台 ID。人看着一串十六进制，认不出那是谁。
//!
//! 这里做的事很窄：**一个 `author_profile` 采集包被接纳后，把其中的公开资料写回同一个
//! 观察目标**。归属靠平台 ID——URL 会变、名字会改，只有平台 ID 是身份。
//!
//! **只写页面上已经可见的公开事实**（头像、简介、粉丝数…）。缺的字段保持缺失，不补 0、
//! 不用旧值顶替：能力登记表明确记着 `userPageData` 可能整个拿不到，那时粉丝数是真的
//! 「不知道」，而不是「0」。

use linggan_storage_postgres::Database;
use serde_json::{Value, json};

/// 从一个 `author_profile` 采集包里取出公开资料，回填到对应的观察目标。
///
/// 返回是否真的更新了某个目标。找不到对应目标不是错误：人可能还没把这个博主加进观察。
pub async fn enrich_target_from_author_profile(
    database: &Database,
    package_kind: &str,
    platform: &str,
    records: &[Value],
) -> Result<bool, sqlx::Error> {
    if package_kind != "author_profile" {
        return Ok(false);
    }
    let Some(author) = records.first().and_then(|record| record.get("payload")) else {
        return Ok(false);
    };
    // 平台 ID 才是身份。取不到就不猜——宁可让目标停在只有 ID 的样子。
    let Some(identity_key) = text(author, "userId").or_else(|| text(author, "platformAuthorId"))
    else {
        return Ok(false);
    };

    let facts = public_facts(author);
    let display_name = text(author, "name");

    let affected = sqlx::query(
        "UPDATE collection_observation_target \
         SET identity_facts = $3, \
             display_name = coalesce($4, display_name) \
         WHERE platform = $1 AND target_kind = 'creator' AND identity_key = $2",
    )
    .bind(platform)
    .bind(&identity_key)
    .bind(&facts)
    .bind(display_name.as_deref())
    .execute(database.pool())
    .await?
    .rows_affected();
    Ok(affected > 0)
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
