//! DOMAIN-UNIFICATION-001 · Domain is a peer research boundary. Target and Material identities
//! remain global; this module only reads and manages Domain configuration.

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
    #[error("no observation domain with that reference")]
    UnknownDomain,
    #[error("an observation domain status must be active or paused")]
    InvalidStatus,
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
    pub name: String,
    pub description: Option<String>,
    pub research_goal: Option<String>,
    pub status: String,
    pub sample_count: Option<i64>,
    pub target_count: i64,
    pub primary_target_count: i64,
    pub reference_target_count: i64,
}

pub async fn observation_domain_schema_is_ready(database: &Database) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar::<_, bool>("SELECT to_regclass('observation_domain') IS NOT NULL")
        .fetch_one(database.pool())
        .await
}

/// Read active and paused Domains for management and explicit selection.
pub async fn read_observation_domains(
    database: &Database,
) -> Result<Vec<ObservationDomain>, ObservationDomainError> {
    if !observation_domain_schema_is_ready(database).await? {
        return Err(ObservationDomainError::SchemaUnavailable);
    }
    let rows: Vec<(Uuid, String, Option<String>, Option<String>, String, Option<i64>, i64, i64, i64)> = sqlx::query_as(
        "SELECT domain.domain_ref, domain.name, domain.description, domain.research_goal, domain.status, \
                (SELECT count(DISTINCT usage.content_public_ref) \
                   FROM linggan_material_domain_usage usage \
                  WHERE usage.domain_ref=domain.domain_ref), \
                count(relation.target_ref), \
                count(relation.target_ref) FILTER (WHERE relation.role='primary'), \
                count(relation.target_ref) FILTER (WHERE relation.role='reference') \
         FROM observation_domain domain \
         LEFT JOIN observation_domain_target relation USING(domain_ref) \
         GROUP BY domain.domain_ref \
         ORDER BY domain.created_at, domain.domain_ref",
    )
    .fetch_all(database.pool())
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(
                domain_ref,
                name,
                description,
                research_goal,
                status,
                sample_count,
                target_count,
                primary_target_count,
                reference_target_count,
            )| ObservationDomain {
                domain_ref,
                name,
                description,
                research_goal,
                status,
                sample_count,
                target_count,
                primary_target_count,
                reference_target_count,
            },
        )
        .collect())
}

/// Resolve only the Domain explicitly present in the URL. Missing and unknown refs stay unresolved.
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
}

/// 采集侧「全部领域」的地址取值。
///
/// 用一个显式的词而不是「不带参数」来表达全部领域：地址上看得见自己正处在哪种读法，
/// 且导航链接可以原样把它带上，不必为这一种情况写特例。
pub const ALL_DOMAINS: &str = "all";

/// 解析采集侧的当前领域。`None` 表示**全部领域**，不是「读不出来」。
///
/// An absent ref or the explicit `all` marker means the operational all-Domains view. An unknown
/// explicit ref resolves to no Domain and is never silently replaced.
pub fn resolve_collection_domain<'a>(
    domains: &'a [ObservationDomain],
    requested: Option<&str>,
) -> Option<&'a ObservationDomain> {
    let Some(requested) = requested.map(str::trim).filter(|value| !value.is_empty()) else {
        return None;
    };
    if requested.eq_ignore_ascii_case(ALL_DOMAINS) {
        return None;
    }
    Uuid::parse_str(requested).ok().and_then(|domain_ref| {
        domains
            .iter()
            .find(|domain| domain.domain_ref == domain_ref)
    })
}

/// Create a peer Domain. Configuration creation never starts collection or platform access.
pub async fn create_observation_domain(
    database: &Database,
    name: &str,
) -> Result<ObservationDomain, ObservationDomainError> {
    create_observation_domain_with_details(database, name, None, None).await
}

pub async fn create_observation_domain_with_details(
    database: &Database,
    name: &str,
    description: Option<&str>,
    research_goal: Option<&str>,
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
        "INSERT INTO observation_domain (domain_ref,name,description,research_goal) \
         VALUES ($1,$2,$3,$4) ON CONFLICT (name) DO NOTHING",
    )
    .bind(domain_ref)
    .bind(name)
    .bind(description.map(str::trim).filter(|value| !value.is_empty()))
    .bind(
        research_goal
            .map(str::trim)
            .filter(|value| !value.is_empty()),
    )
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
        description: description
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned),
        research_goal: research_goal
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned),
        status: "active".to_owned(),
        sample_count: Some(0),
        target_count: 0,
        primary_target_count: 0,
        reference_target_count: 0,
    })
}

pub async fn update_observation_domain(
    database: &Database,
    domain_ref: Uuid,
    name: &str,
    description: Option<&str>,
    research_goal: Option<&str>,
    status: &str,
) -> Result<(), ObservationDomainError> {
    if !observation_domain_schema_is_ready(database).await? {
        return Err(ObservationDomainError::SchemaUnavailable);
    }
    let name = name.trim();
    if name.is_empty() {
        return Err(ObservationDomainError::EmptyName);
    }
    if !matches!(status, "active" | "paused") {
        return Err(ObservationDomainError::InvalidStatus);
    }
    let changed = sqlx::query(
        "UPDATE observation_domain \
         SET name=$2, description=$3, research_goal=$4, status=$5, updated_at=scope_001_now() \
         WHERE domain_ref=$1",
    )
    .bind(domain_ref)
    .bind(name)
    .bind(description.map(str::trim).filter(|value| !value.is_empty()))
    .bind(
        research_goal
            .map(str::trim)
            .filter(|value| !value.is_empty()),
    )
    .bind(status)
    .execute(database.pool())
    .await;
    match changed {
        Ok(result) if result.rows_affected() == 1 => Ok(()),
        Ok(_) => Err(ObservationDomainError::UnknownDomain),
        Err(sqlx::Error::Database(error)) if error.code().as_deref() == Some("23505") => {
            Err(ObservationDomainError::DuplicateName)
        }
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collection_falls_back_to_every_domain_when_scope_is_omitted_or_invalid() {
        let domains = vec![domain("ADHD"), domain("考研自习")];
        // 不带、空、写了 all、写了看不懂的值——都是「全部领域」。
        for requested in [None, Some(""), Some("all"), Some("ALL"), Some("не-uuid")] {
            assert!(resolve_collection_domain(&domains, requested).is_none());
        }
        // 一个存在的领域仍然选得中。
        let picked = resolve_collection_domain(&domains, Some(&domains[1].domain_ref.to_string()));
        assert_eq!(picked.map(|d| d.name.as_str()), Some("考研自习"));
        // 不存在的领域不该悄悄变成某个有效领域。
        assert!(resolve_collection_domain(&domains, Some(&Uuid::new_v4().to_string())).is_none());
    }

    fn domain(name: &str) -> ObservationDomain {
        ObservationDomain {
            domain_ref: Uuid::new_v4(),
            name: name.to_owned(),
            description: None,
            research_goal: None,
            status: "active".to_owned(),
            sample_count: Some(0),
            target_count: 0,
            primary_target_count: 0,
            reference_target_count: 0,
        }
    }

    #[test]
    fn missing_or_unknown_domain_does_not_fall_back() {
        let domains = vec![domain("ADHD"), domain("考研自习")];

        for requested in [None, Some(""), Some("   "), Some("not-a-uuid")] {
            assert!(resolve_current_domain(&domains, requested).is_none());
        }
        assert!(resolve_current_domain(&domains, Some(&Uuid::new_v4().to_string())).is_none());
    }

    #[test]
    fn an_external_domain_is_selected_and_reads_its_own_source() {
        let domains = vec![domain("ADHD"), domain("考研自习")];
        let requested = domains[1].domain_ref.to_string();
        let resolved = resolve_current_domain(&domains, Some(&requested)).expect("选中外部领域");

        assert_eq!(resolved.name, "考研自习");
        assert_eq!(resolved.domain_ref, domains[1].domain_ref);
    }

    #[test]
    fn all_domains_have_the_same_material_count_semantics() {
        assert_eq!(domain("ADHD").sample_count, Some(0));
        assert_eq!(domain("考研自习").sample_count, Some(0));
    }
}
