//! CORPUS-CROSS-INDUSTRY-001 · 观察领域：语料页当前在看哪个行业。
//!
//! 领域不是「本领域 vs 跨行业」两个并列的东西，而是**一个当前观察对象**。ADHD 在这里
//! 只是带着「本领域」标记的普通一项——换领域就是换观察对象，页面结构不变，因此新增
//! 任何领域都不需要重新设计页面。
//!
//! 界面统一不等于数据合并。本领域读证据侧的既有只读接口，外部领域读跨行业样本，两条
//! 查询路径不共享；隔离由 `cross_industry_sample` 上的 CHECK 与复合外键在数据库层保证，
//! 不由这里的任何过滤条件承担。

use linggan_storage_postgres::Database;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum ObservationDomainError {
    #[error("observation domain schema is not applied")]
    SchemaUnavailable,
    #[error("an observation domain needs a name")]
    EmptyName,
    #[error("an observation domain with that name already exists")]
    DuplicateName,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

/// 一个可观察的领域。
///
/// 字段刻意极简。「领域描述」「领域关系」「相似度」在有真实使用经验之前都不加——没用过
/// 就先建模，建出来的一定是想象中的结构。
#[derive(Debug, Clone)]
pub struct ObservationDomain {
    pub domain_ref: Uuid,
    /// 就是那个一级关键词，本身可以直接拿去搜索。
    pub name: String,
    /// 只服务切换器展示。隔离的承担者是数据库约束，不是这个布尔值。
    pub is_own_domain: bool,
    pub status: String,
    /// 该领域下已采到的样本数。本领域恒为 `None`——它的材料在证据侧，不在这张表里，
    /// 拿 0 去填会把「不适用」说成「一条都没有」。
    pub sample_count: Option<i64>,
}

impl ObservationDomain {
    /// 这个领域的材料从哪读。两条路径不共享查询，也不共享接口。
    pub fn reads_evidence(&self) -> bool {
        self.is_own_domain
    }
}

pub async fn observation_domain_schema_is_ready(database: &Database) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar::<_, bool>(
        "SELECT to_regclass('observation_domain') IS NOT NULL \
                AND to_regclass('cross_industry_sample') IS NOT NULL",
    )
    .fetch_one(database.pool())
    .await
}

/// 读全部启用中的领域，本领域排在最前。
///
/// 顺序是稳定的：本领域优先，其余按建立时间。切换器每次打开顺序都一样，人才记得住位置。
pub async fn read_observation_domains(
    database: &Database,
) -> Result<Vec<ObservationDomain>, ObservationDomainError> {
    if !observation_domain_schema_is_ready(database).await? {
        return Err(ObservationDomainError::SchemaUnavailable);
    }
    let rows: Vec<(Uuid, String, bool, String, Option<i64>)> = sqlx::query_as(
        "SELECT domain.domain_ref, domain.name, domain.is_own_domain, domain.status, \
                CASE WHEN domain.is_own_domain THEN NULL ELSE ( \
                    SELECT count(*) FROM cross_industry_sample sample \
                    WHERE sample.domain_ref = domain.domain_ref) END \
         FROM observation_domain domain \
         WHERE domain.status = 'active' \
         ORDER BY domain.is_own_domain DESC, domain.created_at, domain.domain_ref",
    )
    .fetch_all(database.pool())
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(domain_ref, name, is_own_domain, status, sample_count)| ObservationDomain {
                domain_ref,
                name,
                is_own_domain,
                status,
                sample_count,
            },
        )
        .collect())
}

/// 解析出「当前观察领域」。
///
/// 地址里没写、写了个不存在的、或者写了个已停用的，都回落到本领域——一个读不出来的领域
/// 参数不该让页面空着，也不该让人以为自己正在看某个外部行业。
pub fn resolve_current_domain<'a>(
    domains: &'a [ObservationDomain],
    requested: Option<&str>,
) -> Option<&'a ObservationDomain> {
    requested
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(|value| {
            Uuid::parse_str(value).ok().and_then(|domain_ref| {
                domains
                    .iter()
                    .find(|domain| domain.domain_ref == domain_ref)
            })
        })
        .or_else(|| domains.iter().find(|domain| domain.is_own_domain))
        .or_else(|| domains.first())
}

/// 采集侧「全部领域」的地址取值。
///
/// 用一个显式的词而不是「不带参数」来表达全部领域：地址上看得见自己正处在哪种读法，
/// 且导航链接可以原样把它带上，不必为这一种情况写特例。
pub const ALL_DOMAINS: &str = "all";

/// 解析采集侧的当前领域。`None` 表示**全部领域**，不是「读不出来」。
///
/// 与语料侧的 [`resolve_current_domain`] 有意不同：那边读不出来回落本领域——阅读时
/// 混着看没有意义；这边回落全部领域——采集是运维视角，一眼看到所有领域在跑什么是
/// 真实需求，而且这正是加入领域之前的既有行为，不给人任何意外。
pub fn resolve_collection_domain<'a>(
    domains: &'a [ObservationDomain],
    requested: Option<&str>,
) -> Option<&'a ObservationDomain> {
    let requested = requested.map(str::trim).filter(|value| !value.is_empty())?;
    if requested.eq_ignore_ascii_case(ALL_DOMAINS) {
        return None;
    }
    Uuid::parse_str(requested).ok().and_then(|domain_ref| {
        domains
            .iter()
            .find(|domain| domain.domain_ref == domain_ref)
    })
}

/// 建一个新的观察领域。
///
/// **新领域一律是外部领域**（参照物）。本领域只能有一个，由 `0041` 预置并由一条
/// partial unique 索引保证——"我自己在做的那个行业"不是可以随手多开一个的东西。
///
/// 这件事比它听起来轻：**不需要建任何表**。跨行业材料共用同一组表，靠 `domain_ref`
/// 区分；新领域只是多了一个可以被归属的值。真正把两边隔开的是复合外键与
/// `is_own_domain` 上的 CHECK，不是"一个领域一套表"。
pub async fn create_observation_domain(
    database: &Database,
    name: &str,
) -> Result<ObservationDomain, ObservationDomainError> {
    if !observation_domain_schema_is_ready(database).await? {
        return Err(ObservationDomainError::SchemaUnavailable);
    }
    let name = name.trim();
    if name.is_empty() {
        return Err(ObservationDomainError::EmptyName);
    }
    let domain_ref = Uuid::new_v4();
    let inserted = sqlx::query(
        "INSERT INTO observation_domain (domain_ref,name,is_own_domain) \
         VALUES ($1,$2,false) ON CONFLICT (name) DO NOTHING",
    )
    .bind(domain_ref)
    .bind(name)
    .execute(database.pool())
    .await?
    .rows_affected();
    // 重名不静默复用已有那一个：人以为自己新建了一个领域，实际把目标归进了别人的领域，
    // 而这两件事在后面的材料归属上后果完全不同。
    if inserted == 0 {
        return Err(ObservationDomainError::DuplicateName);
    }
    Ok(ObservationDomain {
        domain_ref,
        name: name.to_owned(),
        is_own_domain: false,
        status: "active".to_owned(),
        sample_count: Some(0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collection_falls_back_to_every_domain_rather_than_the_home_one() {
        let domains = vec![domain("ADHD", true), domain("考研自习", false)];
        // 不带、空、写了 all、写了看不懂的值——都是「全部领域」。
        for requested in [None, Some(""), Some("all"), Some("ALL"), Some("не-uuid")] {
            assert!(resolve_collection_domain(&domains, requested).is_none());
        }
        // 一个存在的领域仍然选得中。
        let picked = resolve_collection_domain(&domains, Some(&domains[1].domain_ref.to_string()));
        assert_eq!(picked.map(|d| d.name.as_str()), Some("考研自习"));
        // 不存在的领域不该悄悄变成本领域——那会让人以为自己在看某个外部行业。
        assert!(resolve_collection_domain(&domains, Some(&Uuid::new_v4().to_string())).is_none());
    }

    fn domain(name: &str, is_own_domain: bool) -> ObservationDomain {
        ObservationDomain {
            domain_ref: Uuid::new_v4(),
            name: name.to_owned(),
            is_own_domain,
            status: "active".to_owned(),
            sample_count: if is_own_domain { None } else { Some(0) },
        }
    }

    #[test]
    fn an_unreadable_domain_parameter_falls_back_to_the_home_domain() {
        let domains = vec![domain("ADHD", true), domain("考研自习", false)];

        // 地址没写、写了不是 uuid 的、写了一个不存在的 uuid —— 三种都回落，而不是空页面。
        for requested in [None, Some(""), Some("   "), Some("not-a-uuid")] {
            let resolved = resolve_current_domain(&domains, requested).expect("有回落");
            assert!(resolved.is_own_domain, "读不出来时必须回到本领域");
        }
        let absent = Uuid::new_v4().to_string();
        assert!(
            resolve_current_domain(&domains, Some(&absent))
                .expect("有回落")
                .is_own_domain,
        );
    }

    #[test]
    fn an_external_domain_is_selected_and_reads_its_own_source() {
        let domains = vec![domain("ADHD", true), domain("考研自习", false)];
        let requested = domains[1].domain_ref.to_string();
        let resolved = resolve_current_domain(&domains, Some(&requested)).expect("选中外部领域");

        assert_eq!(resolved.name, "考研自习");
        // 界面是同一套，数据源不是。本领域读证据侧，外部领域读样本侧。
        assert!(!resolved.reads_evidence());
        assert!(domains[0].reads_evidence());
    }

    #[test]
    fn the_home_domain_has_no_sample_count_of_its_own() {
        // 本领域的材料在证据侧，不在跨行业样本表里。填 0 会把「不适用」说成「一条都没有」。
        assert_eq!(domain("ADHD", true).sample_count, None);
        assert_eq!(domain("考研自习", false).sample_count, Some(0));
    }
}
