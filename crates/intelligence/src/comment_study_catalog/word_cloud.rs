//! Read-only, evidence-scoped domain phrase frequency. No model invocation, cache writes,
 //! unverified synonym merging, or source-text exposure in the HTTP response.
use super::{CLEANER_VERSION, StudyCatalogError};
use linggan_storage_postgres::Database;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

const MAX_COMMENTS: usize = 20_000;
const MAX_TERMS: usize = 32;
const TEXT_PREFIX_CHARS: usize = 800;
pub const WORD_CLOUD_VERSION: &str = "comment-phrase-frequency.v1";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WordCloudQuery {
    pub domain: Uuid,
    pub days: Option<i32>,
}

#[derive(Debug)]
struct Comment {
    work_ref: Uuid,
    external_id: String,
    text: String,
}

#[derive(Debug)]
struct Term {
    text: String,
    count: usize,
    category: &'static str,
}

// The seed lexicon only classifies phrases; frequency is always measured from real
// source-qualified comments. Unlisted candidate phrases remain explicitly unclassified.
// No domain identity, diagnosis or clinical validity is inferred from these labels.
const SEEDS: &[(&str, &str)] = &[
    ("作业拖拉", "scenario"), ("写作业", "scenario"), ("上课走神", "scenario"),
    ("注意力不集中", "scenario"), ("顶嘴对抗", "scenario"),
    ("反复提醒", "scenario"), ("睡眠问题", "scenario"),
    ("学校沟通", "scenario"), ("亲子冲突", "scenario"),
    ("拖延症", "scenario"), ("情绪失控", "emotion"),
    ("情绪崩溃", "emotion"), ("父母崩溃", "emotion"),
    ("焦虑", "emotion"), ("无力感", "emotion"),
    ("反复争辩", "emotion"), ("小任务拆分", "approach"),
    ("三分钟关卡", "approach"), ("兴趣驱动", "approach"),
    ("奖励机制", "approach"), ("自主时间", "approach"),
    ("规则协商", "approach"), ("自我管理", "approach"),
    ("专注力", "scenario"), ("亲子沟通", "scenario"),
    ("控制不住", "emotion"), ("陪伴", "approach"),
    ("时间管理", "approach"), ("理解孩子", "approach"),
    ("情绪调节", "approach"), ("同伴交往", "scenario"),
];

fn is_han(c: char) -> bool {
    matches!(c as u32, 0x3400..=0x9fff | 0xf900..=0xfaff | 0x20000..=0x2a6df)
}

// This deliberately conservative candidate generator does not pretend to be a
// language model. All terms are visibly labelled lexical candidates, not Problems.
// The long-phrase path lets non-ADHD domains surface new language without guessing
// synonyms, while the short seed phrases must be exact matches in source text.
fn candidates(text: &str) -> HashSet<String> {
    let mut found: HashSet<String> = SEEDS.iter()
        .filter(|(term, _)| text.contains(term))
        .map(|(term, _)| (*term).to_owned())
        .collect();
    let mut span = Vec::<char>::new();
    let mut scan = |chars: &mut Vec<char>, found: &mut HashSet<String>| {
        if chars.len() >= 4 {
            for start in 0..chars.len() {
                for length in 4..=6 {
                    let Some(slice) = chars.get(start..start + length) else { break };
                    let first = slice[0];
                    let last = slice[length - 1];
                    if "我的你他她是也都在有不就会能还说和对这那被把给很让得了".contains(first)
                        || "的了啊吗吧呢是有在就都和我他她你们呀哦".contains(last) {
                        continue;
                    }
                    if slice.windows(2).all(|pair| pair[0] == pair[1]) { continue; }
                    found.insert(slice.iter().collect());
                }
            }
        }
        chars.clear();
    };
    for c in text.chars() {
        if is_han(c) {
            // Bound long spammed CJK runs; source text is already cleaned/versioned.
            if span.len() < 120 { span.push(c); }
        } else if !span.is_empty() {
            scan(&mut span, &mut found);
        }
    }
    scan(&mut span, &mut found);
    found
}

fn classify(term: &str) -> &'static str {
    SEEDS.iter()
        .find(|(seed, _)| *seed == term)
        .map_or("unclassified", |(_, category)| *category)
}

fn aggregate(comments: &[Comment]) -> Vec<Term> {
    let mut counts = HashMap::<String, usize>::new();
    for comment in comments {
        for phrase in candidates(&comment.text) {
            *counts.entry(phrase).or_default() += 1;
        }
    }
    let mut terms: Vec<Term> = counts.into_iter()
        .filter(|(term, count)| {
            let seed = classify(term) != "unclassified";
            *count >= if seed { 2 } else { 6 }
        })
        .map(|(text, count)| {
            let category = classify(&text);
            Term { text, count, category }
        })
        .collect();
    // Prefer full, frequent phrases to arbitrary fragments with equal support.
    terms.sort_by(|a, b| {
        b.count.cmp(&a.count)
            .then_with(|| (b.category != "unclassified").cmp(&(a.category != "unclassified")))
            .then_with(|| b.text.chars().count().cmp(&a.text.chars().count()))
            .then_with(|| a.text.cmp(&b.text))
    });
    let mut chosen = Vec::<Term>::new();
    for term in terms {
        // Dominated substrings do not crowd out their complete phrase.
        if chosen.iter().any(|full: &Term| {
            full.text.contains(&term.text) && full.text != term.text
                && full.count * 10 >= term.count * 8
        }) { continue; }
        chosen.push(term);
        if chosen.len() == MAX_TERMS { break; }
    }
    chosen
}

const WORD_CLOUD_SQL: &str = r#"
, first_seen AS MATERIALIZED (
    SELECT comment.content_public_ref, comment.comment_external_id,
           min(package.accepted_at) AS observed_at
    FROM linggan_material_comment comment
    JOIN linggan_runtime_capture_package package ON package.package_ref = comment.package_ref
    WHERE EXISTS (
        SELECT 1 FROM linggan_material_domain_usage usage
        WHERE usage.content_public_ref = comment.content_public_ref
          AND usage.domain_ref = $1
    ) AND package.accepted_at <= $2::timestamptz
    GROUP BY comment.content_public_ref, comment.comment_external_id
), scoped AS (
    SELECT qualified.content_public_ref, qualified.comment_external_id,
           left(qualified.research_text, 800) AS text,
           first_seen.observed_at,
           count(*) OVER ()::bigint AS scope_count
    FROM qualified
    JOIN first_seen USING (content_public_ref, comment_external_id)
    WHERE qualified.exclusion_reason IS NULL
      AND qualified.readable
      AND qualified.research_text IS NOT NULL
      AND first_seen.observed_at >= (
          (($2::timestamptz AT TIME ZONE 'Asia/Shanghai')::date - ($6::integer - 1))::timestamp
          AT TIME ZONE 'Asia/Shanghai'
      )
      AND first_seen.observed_at <= $2::timestamptz
)
SELECT content_public_ref, comment_external_id, text, scope_count
FROM scoped
ORDER BY observed_at DESC, content_public_ref, comment_external_id
LIMIT 20001
"#;

/// Total is exact for eligible comments in the chosen window; individual word counts
/// are exact only when the input fits MAX_COMMENTS. The API exposes truncation rather
/// than presenting partial phrase frequencies as whole-domain totals.
pub async fn read_word_cloud(
    database: &Database,
    query: &WordCloudQuery,
) -> Result<Value, StudyCatalogError> {
    let days = query.days.unwrap_or(56);
    if query.domain.is_nil() || !matches!(days, 7 | 28 | 56) {
        return Err(StudyCatalogError::InvalidQuery);
    }
    let mut tx = database.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *tx).await?;
    sqlx::query("SET LOCAL statement_timeout = '20s'").execute(&mut *tx).await?;
    sqlx::query("SET LOCAL jit = off").execute(&mut *tx).await?;
    let as_of: String = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(&mut *tx).await?;
    let sql = format!("{}{}", super::facts_sql(), WORD_CLOUD_SQL);
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
        .persistent(false)
        .bind(query.domain)
        .bind(&as_of)
        .bind(CLEANER_VERSION)
        .bind(Option::<Uuid>::None)
        .bind(Option::<&str>::None)
        .bind(days)
        .fetch_all(&mut *tx).await?;
    tx.commit().await?;
    let total = rows.first()
        .map(|row| row.try_get::<i64, _>("scope_count"))
        .transpose()?.unwrap_or(0);
    let comments = rows.into_iter().take(MAX_COMMENTS)
        .map(|row| Ok(Comment {
            work_ref: row.try_get("content_public_ref")?,
            external_id: row.try_get("comment_external_id")?,
            text: row.try_get::<String, _>("text")?.chars().take(TEXT_PREFIX_CHARS).collect(),
        }))
        .collect::<Result<Vec<_>, sqlx::Error>>()?;

    let items = aggregate(&comments).into_iter().map(|term| {
        let mut works = HashSet::new();
        let mut examples = Vec::new();
        for row in &comments {
            if !row.text.contains(&term.text) { continue; }
            works.insert(row.work_ref);
            if examples.len() < 3 {
                examples.push(json!({
                    "workRef": row.work_ref.to_string(),
                    "commentExternalId": row.external_id
                }));
            }
        }
        json!({
            "text":term.text, "count":term.count, "workCount":works.len(),
            "category":term.category, "examples":examples
        })
    }).collect::<Vec<_>>();
    Ok(json!({
        "domainRef":query.domain, "days":days, "timezone":"Asia/Shanghai",
        "asOf":as_of, "algorithmVersion":WORD_CLOUD_VERSION,
        "eligibleCommentCount":total, "processedCommentCount":comments.len(),
        "truncated":total > MAX_COMMENTS as i64,
        "textPrefixCharacters":TEXT_PREFIX_CHARS, "items":items
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn comment(work: Uuid, id: &str, text: &str) -> Comment {
        Comment { work_ref: work, external_id: id.into(), text: text.into() }
    }
    #[test]
    fn counts_each_exact_phrase_once_per_comment_and_preserves_cross_work_support() {
        let a=Uuid::new_v4();
        let words = aggregate(&[
            comment(a,"1","作业拖拉，作业拖拉，真让人焦虑。"),
            comment(a,"2","作业拖拉，我在拆分小任务"),
            comment(Uuid::new_v4(),"3","其他人的作业拖拉经历"),
        ]);
        assert_eq!(words.iter().find(|w|w.text=="作业拖拉").unwrap().count,3);
        assert!(words.iter().any(|w|w.text=="焦虑") == false);
    }
    #[test]
    fn emoji_mentions_and_single_name_fragments_are_not_candidates() {
        let terms = candidates("😊@妈妈 你好 很好 我是小王");
        assert!(!terms.contains("@妈妈"));
        assert!(!terms.contains("小王"));
        assert!(!terms.contains("我是小王"));
    }
    #[test]
    fn phrases_are_classified_only_by_exact_seed_not_by_guessed_synonyms() {
        assert_eq!(classify("作业拖拉"),"scenario");
        assert_eq!(classify("写不完作业"),"unclassified");
    }
}
