use linggan_storage_postgres::Database;
use serde::Deserialize;
use serde_json::Value;
use sqlx::Row;

#[derive(Debug, thiserror::Error)]
pub enum OpportunityMetricError {
    #[error("opportunity_metric_definition_missing")]
    Missing,
    #[error("opportunity_metric_definition_invalid: {0}")]
    Invalid(String),
    #[error("opportunity_metric_database_unavailable")]
    Database(#[from] sqlx::Error),
}

impl OpportunityMetricError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Missing => "opportunity_metric_definition_missing",
            Self::Invalid(_) => "opportunity_metric_definition_invalid",
            Self::Database(_) => "opportunity_metric_database_unavailable",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Period {
    Week,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PeriodBasis {
    WorkPublishedCohort,
    CommentPublished,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WeekStart {
    Monday,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpportunityMetricParams {
    pub z: f64,
    pub period: Period,
    pub period_basis: PeriodBasis,
    pub period_tz: String,
    pub week_start: WeekStart,
    pub baseline_periods: u32,
    pub series_periods: u32,
    pub min_k_significant: u32,
    pub min_n_period: u32,
    pub min_total_for_significant: u32,
    pub creator_window_days: u32,
    pub creator_min_n: u32,
    pub supply_min_coverage: f64,
    pub supply_max_answers_for_gap: u32,
    pub supply_engagement_ref: String,
    pub supply_max_unjudged_ratio: f64,
    pub gate_min_labeled: u32,
    pub gate_min_kappa: f64,
    pub gate_min_precision: f64,
    pub gate_min_recall: f64,
    pub gate_classes: Vec<String>,
    pub collection_volume_shift_alert: f64,
    pub max_unanchored_share: f64,
    pub max_author_unknown_share: f64,
    pub collection_target_per_period: u32,
    pub comment_time_switch_coverage: f64,
    pub card_rebuild_debounce_seconds: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OpportunityMetricDefinition {
    pub version: i32,
    pub params: OpportunityMetricParams,
}

pub async fn load_current_metric_definition(
    database: &Database,
) -> Result<OpportunityMetricDefinition, OpportunityMetricError> {
    let row = sqlx::query(
        "SELECT d.version, d.params
         FROM linggan_opportunity_metric_definition_current c
         JOIN linggan_opportunity_metric_definition d ON d.version = c.version
         WHERE c.singleton",
    )
    .fetch_optional(database.pool())
    .await?
    .ok_or(OpportunityMetricError::Missing)?;
    Ok(OpportunityMetricDefinition {
        version: row.try_get("version")?,
        params: parse_params(row.try_get("params")?)?,
    })
}

fn parse_params(value: Value) -> Result<OpportunityMetricParams, OpportunityMetricError> {
    let params: OpportunityMetricParams = serde_json::from_value(value)
        .map_err(|error| OpportunityMetricError::Invalid(format!("params: {error}")))?;
    params.validate()?;
    Ok(params)
}

impl OpportunityMetricParams {
    fn validate(&self) -> Result<(), OpportunityMetricError> {
        require_positive("z", self.z)?;
        require_positive_count("baseline_periods", self.baseline_periods)?;
        require_positive_count("series_periods", self.series_periods)?;
        require_positive_count("min_k_significant", self.min_k_significant)?;
        require_positive_count("min_n_period", self.min_n_period)?;
        require_positive_count("min_total_for_significant", self.min_total_for_significant)?;
        require_positive_count("creator_window_days", self.creator_window_days)?;
        require_positive_count("creator_min_n", self.creator_min_n)?;
        require_positive_count("gate_min_labeled", self.gate_min_labeled)?;
        require_positive_count(
            "collection_target_per_period",
            self.collection_target_per_period,
        )?;
        require_positive_count(
            "card_rebuild_debounce_seconds",
            self.card_rebuild_debounce_seconds,
        )?;
        require_share("supply_min_coverage", self.supply_min_coverage)?;
        require_share("supply_max_unjudged_ratio", self.supply_max_unjudged_ratio)?;
        require_share("gate_min_kappa", self.gate_min_kappa)?;
        require_share("gate_min_precision", self.gate_min_precision)?;
        require_share("gate_min_recall", self.gate_min_recall)?;
        require_share(
            "collection_volume_shift_alert",
            self.collection_volume_shift_alert,
        )?;
        require_share("max_unanchored_share", self.max_unanchored_share)?;
        require_share("max_author_unknown_share", self.max_author_unknown_share)?;
        require_share(
            "comment_time_switch_coverage",
            self.comment_time_switch_coverage,
        )?;
        if self.period_tz.trim().is_empty() {
            return Err(invalid("period_tz must not be empty"));
        }
        if self.supply_engagement_ref.trim().is_empty() {
            return Err(invalid("supply_engagement_ref must not be empty"));
        }
        if self.gate_classes.is_empty() {
            return Err(invalid("gate_classes must not be empty"));
        }
        let mut classes = std::collections::HashSet::new();
        for class in &self.gate_classes {
            if !matches!(class.as_str(), "answers" | "mentions") {
                return Err(invalid("gate_classes contains an unsupported value"));
            }
            if !classes.insert(class) {
                return Err(invalid("gate_classes must not repeat a value"));
            }
        }
        Ok(())
    }
}

fn invalid(reason: &str) -> OpportunityMetricError {
    OpportunityMetricError::Invalid(reason.to_owned())
}

fn require_positive(name: &str, value: f64) -> Result<(), OpportunityMetricError> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(invalid(&format!("{name} must be a positive finite number")))
    }
}

fn require_positive_count(name: &str, value: u32) -> Result<(), OpportunityMetricError> {
    if value > 0 {
        Ok(())
    } else {
        Err(invalid(&format!("{name} must be greater than zero")))
    }
}

fn require_share(name: &str, value: f64) -> Result<(), OpportunityMetricError> {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(invalid(&format!("{name} must be between zero and one")))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    const TEST_PARAMS: &str = r#"{
      "z": 1.959963984540054,
      "period": "week",
      "period_basis": "work_published_cohort",
      "period_tz": "Asia/Shanghai",
      "week_start": "monday",
      "baseline_periods": 4,
      "series_periods": 8,
      "min_k_significant": 3,
      "min_n_period": 500,
      "min_total_for_significant": 5,
      "creator_window_days": 90,
      "creator_min_n": 50,
      "supply_min_coverage": 0.6,
      "supply_max_answers_for_gap": 2,
      "supply_engagement_ref": "domain_median_likes_90d",
      "supply_max_unjudged_ratio": 0.2,
      "gate_min_labeled": 250,
      "gate_min_kappa": 0.70,
      "gate_min_precision": 0.85,
      "gate_min_recall": 0.80,
      "gate_classes": ["answers", "mentions"],
      "collection_volume_shift_alert": 0.30,
      "max_unanchored_share": 0.20,
      "max_author_unknown_share": 0.20,
      "collection_target_per_period": 700,
      "comment_time_switch_coverage": 0.90,
      "card_rebuild_debounce_seconds": 300
    }"#;

    fn params() -> Value {
        serde_json::from_str(TEST_PARAMS).expect("test parameter set must be valid JSON")
    }

    #[test]
    fn the_fixed_test_parameter_set_loads_and_keeps_its_basis() {
        let parsed = parse_params(params()).expect("the fixed test parameter set must load");
        assert_eq!(parsed.period, Period::Week);
        assert_eq!(parsed.week_start, WeekStart::Monday);
        assert_eq!(parsed.period_basis, PeriodBasis::WorkPublishedCohort);
        assert_eq!(parsed.collection_target_per_period, 700);
    }

    #[test]
    fn invalid_parameter_values_are_rejected() {
        let cases: &[(&str, Value)] = &[
            ("z", json!(0)),
            ("z", json!(-1.0)),
            ("period", json!("month")),
            ("period_basis", json!("unknown")),
            ("period_tz", json!("")),
            ("week_start", json!("sunday")),
            ("baseline_periods", json!(0)),
            ("series_periods", json!(0)),
            ("min_k_significant", json!(0)),
            ("min_n_period", json!(0)),
            ("min_n_period", json!("500")),
            ("min_total_for_significant", json!(0)),
            ("creator_window_days", json!(0)),
            ("creator_min_n", json!(0)),
            ("supply_min_coverage", json!(-0.1)),
            ("supply_min_coverage", json!(1.1)),
            ("supply_max_answers_for_gap", json!(-1)),
            ("supply_engagement_ref", json!("")),
            ("supply_max_unjudged_ratio", json!(1.1)),
            ("gate_min_labeled", json!(0)),
            ("gate_min_kappa", json!(1.1)),
            ("gate_min_precision", json!(-1)),
            ("gate_min_recall", json!(2)),
            ("gate_classes", json!([])),
            ("gate_classes", json!(["answers", "other"])),
            ("gate_classes", json!(["answers", "answers"])),
            ("collection_volume_shift_alert", json!(-0.1)),
            ("max_unanchored_share", json!(1.1)),
            ("max_author_unknown_share", json!(-0.1)),
            ("collection_target_per_period", json!(0)),
            ("comment_time_switch_coverage", json!(1.01)),
            ("card_rebuild_debounce_seconds", json!(0)),
        ];
        for (field, bad_value) in cases {
            let mut value = params();
            value[field] = bad_value.clone();
            assert!(
                parse_params(value).is_err(),
                "{field} should reject {bad_value}"
            );
        }
    }

    #[test]
    fn missing_and_unknown_parameters_are_rejected() {
        let mut missing = params();
        missing.as_object_mut().expect("object").remove("z");
        assert!(parse_params(missing).is_err());

        let mut unknown = params();
        unknown["unknown_threshold"] = json!(1);
        assert!(parse_params(unknown).is_err());
    }
}
