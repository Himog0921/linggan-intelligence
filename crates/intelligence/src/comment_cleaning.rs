//! Conservative, versioned transformations. Coordinates always map back to Unicode scalars
//! in the immutable source; ZWJ/emoji and negations are never stripped.
use crate::{comment_research::comment_source_hash, model_settings::ModelError};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use sqlx::Row;
pub const CLEANER_VERSION: &str = "comment-clean.v1";
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CleanComment {
    pub text: String,
    pub offsets: Vec<(usize, usize)>,
    pub state: String,
    pub reasons: Vec<String>,
}
impl CleanComment {
    /// Only unambiguous exact outbound quotes are accepted. Masked text is not evidence.
    pub fn resolve(&self, quote: &str, raw: &str) -> Result<(i32, i32, String), &'static str> {
        if quote.is_empty() || quote.contains('█') {
            return Err("quote_redacted_or_empty");
        }
        let chars: Vec<_> = self.text.chars().collect();
        let needle: Vec<_> = quote.chars().collect();
        let positions: Vec<_> = chars
            .windows(needle.len())
            .enumerate()
            .filter(|(_, w)| *w == needle.as_slice())
            .map(|(i, _)| i)
            .collect();
        if positions.len() != 1 {
            return Err("quote_missing_or_ambiguous");
        }
        let start = positions[0];
        let end = start + needle.len();
        let a = self.offsets.get(start).ok_or("quote_out_of_bounds")?.0;
        let b = self.offsets.get(end - 1).ok_or("quote_out_of_bounds")?.1;
        let original: String = raw.chars().skip(a).take(b - a).collect();
        Ok((a as i32, b as i32, original))
    }
}
pub fn clean(raw: &str) -> CleanComment {
    let chars: Vec<_> = raw.chars().collect();
    let mut out = CleanComment {
        text: String::new(),
        offsets: vec![],
        state: "direct".into(),
        reasons: vec![],
    };
    if chars.len() > 16000 {
        out.state = "anomaly".into();
        out.reasons.push("source_too_long".into());
        return out;
    }
    let mut i = 0;
    while i < chars.len() {
        let start = i;
        let mut c = chars[i];
        i += 1;
        if matches!(c, '\u{200b}' | '\u{feff}') {
            continue;
        }
        if c == '&' {
            let rest: String = chars[start..].iter().take(12).collect();
            if let Some(end) = rest.find(';') {
                let entity = &rest[1..end];
                let decoded = match entity {
                    "amp" => Some('&'),
                    "lt" => Some('<'),
                    "gt" => Some('>'),
                    "quot" => Some('"'),
                    "apos" | "#39" => Some('\''),
                    "nbsp" => Some(' '),
                    _ => None,
                };
                if let Some(d) = decoded {
                    c = d;
                    i = start + end + 1;
                }
            }
        }
        if ('\u{ff01}'..='\u{ff5e}').contains(&c) {
            c = char::from_u32(c as u32 - 0xfee0).unwrap_or(c);
        }
        if c.is_whitespace() {
            if out.text.is_empty() {
                continue;
            }
            if out.text.ends_with(' ') {
                if let Some(last) = out.offsets.last_mut() {
                    last.1 = i;
                }
                continue;
            }
            c = ' ';
        }
        out.text.push(c);
        out.offsets.push((start, i));
    }
    if out.text.ends_with(' ') {
        out.text.pop();
        out.offsets.pop();
    }
    if out.text != raw {
        out.reasons.push("whitespace_or_encoding".into());
    }
    if out.text.is_empty() || out.text.chars().all(|c| c == '�') {
        out.state = "anomaly".into();
        out.reasons.push("empty_or_damaged".into());
    } else if out.text.chars().all(|c| !c.is_alphanumeric()) {
        out.state = "low_information".into();
        out.reasons.push("reaction_only".into());
    } else if [
        "我也是",
        "我们也是",
        "同问",
        "同感",
        "是的",
        "真的",
        "对",
        "嗯",
        "求",
        "怎么做",
    ]
    .iter()
    .any(|t| {
        out.text
            .trim_matches(|c: char| c.is_ascii_punctuation() || "！？。～".contains(c))
            == *t
    }) || out.text.chars().count() < 4
    {
        out.state = "context".into();
        out.reasons.push("context_dependent".into());
    }
    out
}
/// Outbound only. Conservative contact-token masking never changes the local source.
pub fn outbound(mut value: CleanComment) -> CleanComment {
    let mut cs: Vec<char> = value.text.chars().collect();
    let mut start = 0;
    while start < cs.len() {
        if cs[start].is_ascii_alphanumeric() || matches!(cs[start], '@' | '+' | '_' | '-' | '.') {
            let mut end = start + 1;
            while end < cs.len()
                && (cs[end].is_ascii_alphanumeric()
                    || matches!(cs[end], '@' | '+' | '_' | '-' | '.'))
            {
                end += 1;
            }
            let token: String = cs[start..end].iter().collect();
            let prefix: String = cs[..start].iter().rev().take(12).rev().collect();
            if token.chars().filter(char::is_ascii_digit).count() >= 7
                || (token.contains('@') && token.contains('.'))
                || ["微信", "微信号", "加我", "vx", "VX", "电话", "联系"]
                    .iter()
                    .any(|s| prefix.contains(s))
            {
                cs[start..end].fill('█');
                value.reasons.push("contact_masked".into());
            }
            start = end;
        } else {
            start += 1;
        }
    }
    let mut i = 0;
    while i < cs.len() {
        if !cs[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let start = i;
        let mut digits = 0;
        while i < cs.len()
            && (cs[i].is_ascii_digit() || matches!(cs[i], ' ' | '-' | '+' | '(' | ')'))
        {
            if cs[i].is_ascii_digit() {
                digits += 1;
            }
            i += 1;
        }
        if digits >= 7 {
            cs[start..i].fill('█');
            value.reasons.push("contact_masked".into());
        }
    }
    value.text = cs.into_iter().collect();
    value
}
pub async fn clean_pending(db: &Database) -> Result<u64, ModelError> {
    let rows=sqlx::query("SELECT s.material_ref,s.body_text FROM linggan_comment_research_readable s WHERE NOT EXISTS(SELECT 1 FROM linggan_comment_clean c WHERE c.source_ref=s.material_ref AND c.cleaner_version=$1) AND (EXISTS(SELECT 1 FROM linggan_material_comment_current c WHERE c.material_ref=s.material_ref) OR EXISTS(SELECT 1 FROM linggan_comment_daily_item i WHERE i.source_ref=s.material_ref AND i.state='pending')) ORDER BY s.created_at,s.material_ref LIMIT 100")
        .bind(CLEANER_VERSION).fetch_all(db.pool()).await?;
    let mut count = 0;
    for row in rows {
        let raw: Option<String> = row.get("body_text");
        let raw = raw.unwrap_or_default();
        let result = clean(&raw);
        count+=sqlx::query("INSERT INTO linggan_comment_clean(source_ref,cleaner_version,source_sha256,state,result) VALUES($1,$2,$3,$4,$5) ON CONFLICT DO NOTHING")
            .bind(row.get::<uuid::Uuid,_>("material_ref")).bind(CLEANER_VERSION).bind(comment_source_hash(&raw)).bind(&result.state).bind(serde_json::to_value(result).map_err(|_|ModelError::Invalid)?).execute(db.pool()).await?.rows_affected();
    }
    Ok(count)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_mapping_preserves_negation_and_family_emoji() {
        let raw = "  我👨‍👩‍👧不想\n  催促 &amp; 监督  ";
        let c = clean(raw);
        assert!(c.text.contains("不想 催促 & 监督"));
        let (a, b, q) = c.resolve("不想 催促", raw).unwrap();
        assert_eq!(q, "不想\n  催促");
        assert_eq!(
            raw.chars()
                .skip(a as usize)
                .take((b - a) as usize)
                .collect::<String>(),
            q
        );
    }
    #[test]
    fn low_information_is_retained_and_short_replies_need_context() {
        assert_eq!(clean("😭😭😭").state, "low_information");
        assert_eq!(clean("我也是！！").state, "context");
        assert_eq!(clean("没有效果，一催就吵").state, "direct");
    }
    #[test]
    fn contact_masking_does_not_create_fake_evidence() {
        let raw = "没有效果 联系微信 abc123，电话13812345678";
        let c = outbound(clean(raw));
        assert!(!c.text.contains("abc123"));
        assert!(!c.text.contains("13812345678"));
        assert!(c.resolve("██", raw).is_err());
        assert!(c.resolve("没有效果", raw).is_ok());
        assert!(
            !outbound(clean("电话１３８ １２３４ ５６７８"))
                .text
                .contains("1234")
        );
        assert!(clean("aaa").resolve("aa", "aaa").is_err());
        assert!(clean("同问同问").resolve("同问", "同问同问").is_err());
    }
}
