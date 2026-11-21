use super::MetricDefinition;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Agreement {
    pub sample_count: usize,
    pub excluded_example_count: usize,
    pub agreement: Option<f64>,
    pub kappa: Option<f64>,
    pub reason: Option<&'static str>,
    pub meets_threshold: bool,
}

/// Only paired machine decisions contribute to the independent agreement estimate.
pub fn agreement(pairs: &[(&str, &str, bool)], metric: &MetricDefinition) -> Agreement {
    let mut left = BTreeMap::new();
    let mut right = BTreeMap::new();
    let mut matches = 0;
    let mut n = 0;
    for &(primary, audit, example) in pairs {
        if !example {
            *left.entry(primary).or_insert(0usize) += 1;
            *right.entry(audit).or_insert(0usize) += 1;
            matches += usize::from(primary == audit);
            n += 1;
        }
    }
    let chance = left
        .into_iter()
        .map(|(key, count)| count as u128 * *right.get(key).unwrap_or(&0) as u128)
        .sum::<u128>();
    let square = n as u128 * n as u128;
    let agreement = (n > 0).then(|| matches as f64 / n as f64);
    let kappa = (square > chance)
        .then(|| ((matches as u128 * n as u128) as f64 - chance as f64) / (square - chance) as f64);
    Agreement {
        sample_count: n,
        excluded_example_count: pairs.len() - n,
        agreement,
        kappa,
        reason: if n == 0 {
            Some("no_data")
        } else if kappa.is_none() {
            Some("kappa_undefined")
        } else {
            None
        },
        meets_threshold: agreement.is_some_and(|a| a >= metric.min_agreement)
            && kappa.is_some_and(|k| k >= metric.min_kappa),
    }
}
