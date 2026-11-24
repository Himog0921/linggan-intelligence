use super::variance::{residuals, sum, variance};
use super::{
    AnalysisInput, LabelDimension, LensDefinitionError, MetricDefinition, PublicationInput,
    SampleMetric, Subset,
};
use crate::opportunity::wilson_interval;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

#[derive(Deserialize)]
pub(super) struct Category {
    pub key: String,
    pub name: String,
}
#[derive(Deserialize)]
pub(super) struct Vocabulary {
    pub moment: Vec<Category>,
    pub offer: Vec<Category>,
    pub situation: Vec<Category>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RunAgreement {
    pub run_ref: Uuid,
    pub audit_run_ref: Uuid,
    pub dimension: String,
    pub sample_count: i32,
    pub excluded_example_count: i32,
    pub agreement: Option<f64>,
    pub kappa: Option<f64>,
    pub reason: Option<String>,
    pub meets_threshold: bool,
}

type Row<'a> = (Uuid, &'a PublicationInput);
fn rate(rows: &[Row<'_>], metric: SampleMetric, def: &MetricDefinition) -> Value {
    let count = |subset| {
        rows.iter()
            .filter(|(_, r)| metric.contains(r, subset, def))
            .count() as u64
    };
    let (k, n) = (count(Subset::Numerator), count(Subset::Denominator));
    if n == 0 || n < u64::from(def.min_n_rate) {
        return json!({"numerator":k,"denominator":n,"value":null,"low":null,"high":null,"reason":if n==0 {"no_data"} else {"below_min_n"}});
    }
    // Input definition is validated; the numerator is a subset of this denominator.
    let interval = wilson_interval(k, n, def.wilson_z).expect("validated Wilson inputs");
    json!({"numerator":k,"denominator":n,"value":k as f64/n as f64,"low":interval.low,"high":interval.high,"reason":null})
}

pub(super) fn effect(
    rows: &[Row<'_>],
    residual: &BTreeMap<Uuid, f64>,
    m: &MetricDefinition,
) -> Value {
    let mut views: Vec<_> = rows.iter().filter_map(|(_, r)| r.views.value).collect();
    views.sort_unstable();
    let n = views.len();
    let median = (n > 0).then(|| (views[(n - 1) / 2] as f64 + views[n / 2] as f64) / 2.);
    let mut paired = 0;
    let mut total_views = 0_u128;
    let mut total_comments = 0_u128;
    for (_, row) in rows {
        if let (Some(v), Some(c)) = (row.views.value, row.comments.value) {
            paired += 1;
            total_views += u128::from(v);
            total_comments += u128::from(c);
        }
    }
    let adjusted: Vec<_> = rows
        .iter()
        .filter_map(|(id, _)| residual.get(id).copied())
        .collect();
    json!({"publications":rows.len(),"withViews":n,"medianViews":median,
        "midRate":rate(rows, SampleMetric::OverThousand,m),"highCount":rows.iter().filter(|(_,r)|SampleMetric::Hit.contains(r,Subset::Numerator,m)).count(),
        "activeCommentRate":rate(rows,SampleMetric::ActiveComments,m),
        "commentsPerThousandViews":{"value":(total_views>0).then(||1000.*total_comments as f64/total_views as f64),"publications":paired,
            "reason":if paired==0 {Some("no_data")}else if total_views==0{Some("zero_total_views")}else{None}},
        "withinAccountResidual":{"publications":adjusted.len(),"value":(!adjusted.is_empty()).then(||sum(adjusted.iter().copied())/adjusted.len() as f64),
            "reason":adjusted.is_empty().then_some("no_data")}})
}

fn coverage(rows: &[Row<'_>], vocabulary: &Vocabulary) -> Value {
    let mut counts = BTreeMap::new();
    for (_, r) in rows {
        *counts
            .entry((
                &r.labels[&LabelDimension::Moment].value,
                &r.labels[&LabelDimension::Offer].value,
            ))
            .or_insert(0usize) += 1;
    }
    let mut ranked = counts.values().copied().collect::<Vec<_>>();
    ranked.sort_unstable_by(|a, b| b.cmp(a));
    let top = ranked.into_iter().take(10).sum::<usize>();
    let mut cells = Vec::new();
    let (mut empty, mut sparse) = (0, 0);
    let mainstream = |c: &&Category| !matches!(c.key.as_str(), "other" | "unclear");
    for moment in vocabulary.moment.iter().filter(mainstream) {
        for offer in vocabulary.offer.iter().filter(mainstream) {
            let count = *counts.get(&(&moment.key, &offer.key)).unwrap_or(&0);
            let state = match count {
                0 => {
                    empty += 1;
                    "empty"
                }
                1 | 2 => {
                    sparse += 1;
                    "sparse"
                }
                _ => "populated",
            };
            cells.push(
                json!({"row":moment.key,"column":offer.key,"publications":count,"state":state}),
            );
        }
    }
    json!({"rowDimension":"moment","columnDimension":"offer","cells":cells,"emptyCells":empty,"sparseCells":sparse,
        "top10Publications":top,"labeledPublications":rows.len(),"top10Share":(!rows.is_empty()).then(||top as f64/rows.len() as f64)})
}

pub(super) fn summarize(
    input: &AnalysisInput,
    vocab: &Vocabulary,
    m: &MetricDefinition,
    agreements: &[RunAgreement],
) -> Result<Value, LensDefinitionError> {
    if !(m.wilson_z > 0. && m.wilson_z.is_finite())
        || !(1..2_147_483_647).contains(&m.permutation_seed)
        || m.permutation_count == 0
        || m.min_account_posts == 0
    {
        return Err(LensDefinitionError::InvalidDefinition);
    }
    let rows: Vec<_> = input
        .publications
        .iter()
        .filter(|(_, r)| !r.labels.is_empty())
        .map(|(id, r)| (*id, r))
        .collect();
    let residual = residuals(&rows, m.min_account_posts);
    let mut structure = json!({});
    let mut effects = json!({});
    let mut variances = vec![variance(&rows, "account", false, &residual, m)];
    for (name, dim, entries) in [
        ("moment", LabelDimension::Moment, &vocab.moment),
        ("offer", LabelDimension::Offer, &vocab.offer),
        ("situation", LabelDimension::Situation, &vocab.situation),
    ] {
        let mut groups = BTreeMap::<&str, Vec<Row<'_>>>::new();
        for &(id, r) in &rows {
            let l = r
                .labels
                .get(&dim)
                .ok_or(LensDefinitionError::MissingAgreement)?;
            groups.entry(&l.value).or_default().push((id, r));
        }
        let mut counts = Vec::new();
        let mut effect_rows = Vec::new();
        let mut dimension_agreements = agreements.iter().filter(|a| a.dimension == name).peekable();
        let unavailable_reason = if dimension_agreements.peek().is_none() {
            Some("no_data")
        } else if !dimension_agreements.all(|a| a.meets_threshold) {
            Some("agreement_below_threshold")
        } else {
            None
        };
        let passed = unavailable_reason.is_none();
        for entry in entries {
            let group = groups.remove(entry.key.as_str()).unwrap_or_default();
            counts.push(json!({"key":entry.key,"name":entry.name,"publications":group.len(),"share":(!rows.is_empty()).then(||group.len() as f64/rows.len() as f64)}));
            if passed && name != "situation" {
                let mut value = effect(&group, &residual, m);
                value["key"] = json!(entry.key);
                effect_rows.push(value);
            }
        }
        if !groups.is_empty() {
            return Err(LensDefinitionError::InvalidDefinition);
        }
        structure[name] = json!(counts);
        if name != "situation" {
            effects[name] = json!({"available":passed,"unavailableReason":unavailable_reason,"rows":passed.then_some(effect_rows)});
            if passed {
                for within in [false, true] {
                    variances.push(variance(&rows, name, within, &residual, m));
                }
            }
        }
    }
    let adjudicated = rows
        .iter()
        .filter(|(_, r)| r.labels.values().any(|l| l.adjudication_ref.is_some()))
        .map(|(_, r)| &r.title_key)
        .collect::<BTreeSet<_>>();
    let title_keys = input
        .publications
        .values()
        .map(|r| &r.title_key)
        .collect::<BTreeSet<_>>();
    let with_views = input
        .publications
        .values()
        .filter(|r| r.views.value.is_some())
        .count();
    Ok(
        json!({"schemaVersion":"ops-content-lens.snapshot.v1","scope":{"domainRef":input.domain_ref,"publishedFrom":input.from,"publishedTo":input.to},
        "quality":{"publications":input.total_publications,"titled":input.with_title,"titleKeys":title_keys.len(),"pendingTitles":input.pending_titles,
            "withViews":with_views,"withViewsShare":(input.with_title>0).then(||with_views as f64/input.with_title as f64),
            "latestPublishedDate":input.publications.values().filter_map(|r|r.business_date.as_deref()).max(),"agreement":agreements,"adjudicatedTitles":adjudicated.len()},
        "structure":structure,"coverage":coverage(&rows,vocab),"effect":{"grade":"hypothesis","byDimension":effects,"varianceShare":variances},"hypotheses":[]}),
    )
}
