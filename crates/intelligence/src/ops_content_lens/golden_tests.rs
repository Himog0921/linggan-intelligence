//! The expected values come from L0's independent Python implementation.
use super::statistics::{RunAgreement, Vocabulary, effect, summarize};
use super::variance::{residuals, variance};
use super::*;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

const SOURCE: &str = include_str!("../../../../scripts/content-lens-reference/synthetic.json");
const GOLD: &str = include_str!("../../../../scripts/content-lens-reference/answers.json");

fn objects(v: &Value) -> &Vec<Value> {
    v.as_array().unwrap()
}
fn text(v: &Value) -> &str {
    v.as_str().unwrap()
}
fn key(v: &Value) -> String {
    text(v)
        .to_lowercase()
        .chars()
        .filter(|c| !c.is_whitespace() && !c.is_ascii_punctuation())
        .collect()
}
fn fixture(raw: &Value) -> (AnalysisInput, Vocabulary, MetricDefinition) {
    let mut metric = raw["metric"].clone();
    metric["label_batch_size"] = json!(50);
    metric["audit_sample_size"] = json!(250);
    metric["stale_after_days"] = json!(14);
    let mut terms = json!({});
    for (dim, keys) in raw["dimensionKeys"].as_object().unwrap() {
        terms[dim] = json!(
            objects(keys)
                .iter()
                .map(|k| json!({"key":k,"name":k}))
                .collect::<Vec<_>>()
        );
    }
    let labels: BTreeMap<_, _> = objects(&raw["labels"])
        .iter()
        .map(|r| (key(&r[0]), r))
        .collect();
    let mut reports = BTreeMap::new();
    for r in objects(&raw["reports"]) {
        let latest = reports.entry(text(&r[0])).or_insert((0, None));
        let seq = r[1].as_u64().unwrap();
        if seq > latest.0 {
            *latest = (seq, r[2].as_u64());
        }
    }
    let records = objects(&raw["publications"]);
    let accounts: BTreeSet<_> = records.iter().filter_map(|r| r[2].as_str()).collect();
    let accounts: BTreeMap<_, _> = accounts
        .into_iter()
        .enumerate()
        .map(|(i, k)| (k, Uuid::from_u128(i as u128 + 1)))
        .collect();
    let mut publications = BTreeMap::new();
    for r in records {
        let publication = text(&r[0]);
        let id = Uuid::from_u128(publication[1..].parse().unwrap());
        let title_key = key(&r[1]);
        let label = labels[&title_key];
        let labels = [
            LabelDimension::Moment,
            LabelDimension::Offer,
            LabelDimension::Situation,
            LabelDimension::Audience,
        ]
        .into_iter()
        .enumerate()
        .map(|(i, d)| {
            (
                d,
                EffectiveLabel {
                    value: text(&label[i + 1]).to_owned(),
                    run_ref: Uuid::nil(),
                    adjudication_ref: None,
                },
            )
        })
        .collect();
        publications.insert(
            id,
            PublicationInput {
                title_key,
                account_ref: r[2].as_str().map(|a| accounts[a]),
                business_date: r[4].as_str().map(str::to_owned),
                labels,
                views: SourcedCount {
                    value: reports.get(publication).and_then(|r| r.1),
                    source: None,
                },
                comments: SourcedCount {
                    value: r[5].as_u64(),
                    source: None,
                },
                creation_links: BTreeMap::new(),
            },
        );
    }
    let input = AnalysisInput {
        method: LabelMethod {
            taxonomy_ref: Uuid::nil(),
            task_config_ref: Uuid::nil(),
            prompt_version_ref: Uuid::nil(),
        },
        domain_ref: None,
        from: None,
        to: None,
        total_publications: publications.len() as u64,
        with_title: publications.len() as u64,
        pending_titles: 0,
        agreement_sources: BTreeMap::new(),
        publications,
    };
    (
        input,
        serde_json::from_value(terms).unwrap(),
        serde_json::from_value(metric).unwrap(),
    )
}

/// Compare each named oracle field; the panel also carries display names and source metadata.
fn check(actual: &Value, expected: &Value, path: &str) {
    if let Some(fields) = expected.as_object() {
        for (k, v) in fields {
            check(&actual[k], v, &format!("{path}.{k}"));
        }
    } else if expected.is_number() {
        let got = actual
            .as_f64()
            .unwrap_or_else(|| panic!("{path}: {actual}"));
        assert!(
            (got - expected.as_f64().unwrap()).abs() <= 1e-6,
            "{path}: {actual} vs {expected}"
        );
    } else {
        assert_eq!(actual, expected, "{path}");
    }
}
fn passing_agreement(dimension: &str) -> RunAgreement {
    RunAgreement {
        run_ref: Uuid::from_u128(2),
        audit_run_ref: Uuid::from_u128(3),
        dimension: dimension.into(),
        sample_count: 2,
        excluded_example_count: 0,
        agreement: Some(1.0),
        kappa: Some(1.0),
        reason: None,
        meets_threshold: true,
    }
}

fn reference_agreements() -> [RunAgreement; 3] {
    ["moment", "offer", "audience"].map(passing_agreement)
}

#[test]
fn snapshot_matches_frozen_statistics() {
    let raw: Value = serde_json::from_str(SOURCE).unwrap();
    let golden: Value = serde_json::from_str(GOLD).unwrap();
    let (input, vocab, metric) = fixture(&raw);
    let body = summarize(&input, &vocab, &metric, &reference_agreements()).unwrap();
    check(&body["quality"], &golden["quality"], "quality");
    check(&body["coverage"], &golden["coverage"], "coverage");
    for (dim, entries) in golden["structure"].as_object().unwrap() {
        for row in objects(&body["structure"][dim]) {
            check(row, &entries[text(&row["key"])], dim);
        }
    }
    for (dim, entries) in golden["effects"].as_object().unwrap() {
        for (key, gold) in entries.as_object().unwrap() {
            let row = objects(&body["effect"]["byDimension"][dim]["rows"])
                .iter()
                .find(|r| r["key"] == *key)
                .unwrap();
            check(row, gold, key);
        }
    }
    for v in objects(&body["effect"]["varianceShare"]) {
        let set = if v["scope"] == "all" {
            "variance"
        } else {
            "withinAccountVariance"
        };
        check(v, &golden[set][text(&v["factor"])], set);
    }
    let rows: Vec<_> = input.publications.iter().map(|(id, r)| (*id, r)).collect();
    for n in [19usize, 20, 21] {
        check(
            &effect(&rows[..n + 1], &BTreeMap::new(), &metric),
            &golden["boundaryEffects"][n.to_string()],
            "boundary",
        );
    }
    let within = select_publications(
        &input,
        &LabelFilter::default(),
        SampleMetric::WithinAccountViews,
        Subset::All,
        &metric,
    );
    assert_eq!(
        within.len() as u64,
        golden["withinAccountVariance"]["offer"]["observations"]
            .as_u64()
            .unwrap()
    );
    let zero = rows
        .iter()
        .copied()
        .filter(|(_, r)| r.views.value == Some(0))
        .collect::<Vec<_>>();
    check(
        &effect(&zero, &BTreeMap::new(), &metric)["commentsPerThousandViews"],
        &golden["zeroTotalViews"],
        "zero views",
    );
    check(
        &variance(
            &zero,
            "offer",
            false,
            &residuals(&zero, metric.min_account_posts),
            &metric,
        ),
        &golden["zeroVariance"],
        "zero variance",
    );
    check(
        &effect(&[], &BTreeMap::new(), &metric),
        &golden["emptyGroupEffect"],
        "empty",
    );
    for (name, case) in raw["agreementCases"].as_object().unwrap() {
        let pairs = objects(case)
            .iter()
            .map(|r| (text(&r[1]), text(&r[2]), r[3].as_bool().unwrap()))
            .collect::<Vec<_>>();
        check(
            &json!(agreement(&pairs, &metric)),
            &golden["agreement"][name],
            name,
        );
    }
    let low = RunAgreement {
        run_ref: Uuid::nil(),
        audit_run_ref: Uuid::from_u128(1),
        dimension: "offer".into(),
        sample_count: 4,
        excluded_example_count: 1,
        agreement: Some(0.75),
        kappa: Some(0.5),
        reason: None,
        meets_threshold: false,
    };
    let high = passing_agreement("offer");
    let gated = summarize(&input, &vocab, &metric, &[high, low]).unwrap();
    assert_eq!(gated["structure"], body["structure"]);
    assert_eq!(
        gated["effect"]["byDimension"]["offer"]["rows"],
        golden["mixedRunAgreement"]["effect"]
    );
    assert_eq!(
        gated["effect"]["byDimension"]["offer"]["unavailableReason"],
        golden["mixedRunAgreement"]["reason"]
    );
    assert_eq!(gated["effect"]["grade"], "hypothesis");
}

#[test]
#[ignore = "requires a local anonymized November retrospective in L0 fixture shape"]
fn historical_retrospective() {
    let raw = std::fs::read(std::env::var("OPS_LENS_RETRO_REFERENCE").unwrap()).unwrap();
    let (input, vocab, m) = fixture(&serde_json::from_slice(&raw).unwrap());
    let body = summarize(&input, &vocab, &m, &reference_agreements()).unwrap();
    check(
        &body["quality"],
        &json!({"publications":956,"titleKeys":886,"withViews":442}),
        "historical cohort",
    );
    let (mut specified, mut rest) = ((0, 0), (0, 0));
    for e in objects(&body["effect"]["byDimension"]["offer"]["rows"]) {
        let bucket = if matches!(
            text(&e["key"]),
            "mechanism_explainer"
                | "actionable_method"
                | "checklist_tool_resource"
                | "urgency_warning"
        ) {
            &mut specified
        } else {
            &mut rest
        };
        bucket.0 += e["midRate"]["numerator"].as_u64().unwrap();
        bucket.1 += e["midRate"]["denominator"].as_u64().unwrap();
    }
    assert_eq!((specified, rest), ((3, 134), (31, 308)));
    for v in objects(&body["effect"]["varianceShare"])
        .iter()
        .filter(|v| v["scope"] == "all")
    {
        let expected = match text(&v["factor"]) {
            "account" => 0.298,
            "moment" => 0.054,
            "offer" => 0.080,
            _ => unreachable!(),
        };
        assert!((v["observed"].as_f64().unwrap() - expected).abs() < 0.01);
    }
}

#[test]
fn undated_input_still_contributes_to_structure() {
    let raw = serde_json::from_str(SOURCE).unwrap();
    let (mut input, vocab, metric) = fixture(&raw);
    for r in input.publications.values_mut() {
        r.business_date = None;
    }
    let stats = summarize(&input, &vocab, &metric, &[]).unwrap();
    assert_eq!(
        stats["effect"]["byDimension"]["offer"]["unavailableReason"],
        "no_data"
    );
    assert!(stats["effect"]["byDimension"]["offer"]["rows"].is_null());
    assert_eq!(stats["quality"]["publications"], 22);
    assert!(stats["quality"]["latestPublishedDate"].is_null());
    assert_eq!(stats["coverage"]["labeledPublications"], 22);
}
