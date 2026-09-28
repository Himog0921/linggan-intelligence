//! The ordered source gate shared by SQL catalogs and deterministic source preparation.
//! Each input is a fact, not a score. Cleaning has already happened before this gate.
const RULES: [(&str, &str); 6] = [
    ("sourceRestricted", "source_restricted"),
    ("bodyUnavailable", "body_state <> 'KNOWN' OR NOT has_body"),
    ("indexPending", "cached_source_ref IS NULL"),
    (
        "textNotResearchable",
        "clean_state NOT IN ('direct', 'context')",
    ),
    ("workAuthorUnknown", "work_author_unknown"),
    ("creatorVoice", "voice_role = 'creator'"),
];

pub(crate) fn sql_case() -> String {
    let mut sql = String::from("CASE");
    for (code, predicate) in RULES {
        sql.push_str(&format!(" WHEN {predicate} THEN '{code}'"));
    }
    sql.push_str(" ELSE NULL END");
    sql
}

pub(crate) fn exclusion(flags: [bool; 6]) -> Option<&'static str> {
    RULES
        .iter()
        .zip(flags)
        .find_map(|((code, _), excluded)| excluded.then_some(*code))
}

pub(crate) fn rule_index(code: &str) -> Option<usize> {
    RULES.iter().position(|(rule, _)| *rule == code)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_first_exclusion_wins_in_the_same_order_as_sql() {
        assert_eq!(exclusion([false; 6]), None);
        for index in 0..6 {
            let mut flags = [false; 6];
            flags[index..].fill(true);
            assert_eq!(exclusion(flags), Some(RULES[index].0));
        }
        assert_eq!(sql_case().matches(" WHEN ").count(), 6);
        assert_eq!(rule_index("commentAuthorUnknown"), None);
    }
}
