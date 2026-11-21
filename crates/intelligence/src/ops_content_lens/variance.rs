use super::{LabelDimension, MetricDefinition, PublicationInput};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use uuid::Uuid;

/// Compensated summation keeps log-variance and tie comparisons stable at f64 precision.
pub(super) fn sum(values: impl IntoIterator<Item = f64>) -> f64 {
    let (mut total, mut correction) = (0.0_f64, 0.0);
    for x in values {
        let next = total + x;
        correction += if total.abs() >= x.abs() {
            (total - next) + x
        } else {
            (x - next) + total
        };
        total = next;
    }
    total + correction
}

pub(super) fn residuals(rows: &[(Uuid, &PublicationInput)], min_posts: u32) -> BTreeMap<Uuid, f64> {
    let mut accounts = BTreeMap::<Uuid, Vec<(Uuid, f64)>>::new();
    for &(id, r) in rows {
        if let (Some(a), Some(v)) = (r.account_ref, r.views.value) {
            accounts
                .entry(a)
                .or_default()
                .push((id, (v as f64).ln_1p()));
        }
    }
    let mut values = BTreeMap::new();
    for group in accounts.values().filter(|g| g.len() >= min_posts as usize) {
        let mean = sum(group.iter().map(|(_, v)| *v)) / group.len() as f64;
        values.extend(group.iter().map(|&(id, v)| (id, v - mean)));
    }
    values
}

// L0 freezes Park–Miller + rejection sampling + descending Fisher–Yates, per ordered account.
fn shuffle(indices: &[usize], seed: &mut u64, mut swap: impl FnMut(usize, usize)) {
    const MODULUS: u64 = 2_147_483_647;
    for end in (1..indices.len()).rev() {
        let size = end as u64 + 1;
        let limit = (MODULUS - 1) - (MODULUS - 1) % size;
        let draw = loop {
            *seed = *seed * 16_807 % MODULUS;
            if *seed - 1 < limit {
                break *seed - 1;
            }
        };
        swap(indices[end], indices[(draw % size) as usize]);
    }
}

pub(super) fn variance(
    rows: &[(Uuid, &PublicationInput)],
    factor: &str,
    within: bool,
    residual: &BTreeMap<Uuid, f64>,
    metric: &MetricDefinition,
) -> Value {
    let mut values = Vec::new();
    let mut strata = BTreeMap::<Option<Uuid>, Vec<usize>>::new();
    let mut groups = BTreeMap::new();
    let mut labels = Vec::new();
    for &(id, row) in rows {
        let value = if within {
            residual.get(&id).copied()
        } else {
            row.views.value.map(|v| (v as f64).ln_1p())
        };
        let Some(v) = value else {
            continue;
        };
        let key = match factor {
            "offer" => row.labels[&LabelDimension::Offer].value.clone(),
            "moment" => row.labels[&LabelDimension::Moment].value.clone(),
            _ => row.account_ref.map(|id| id.to_string()).unwrap_or_default(),
        };
        let next = groups.len();
        labels.push(*groups.entry(key).or_insert(next));
        strata
            .entry(if within { row.account_ref } else { None })
            .or_default()
            .push(values.len());
        values.push(v);
    }
    let n = values.len();
    let scope = if within { "within_account" } else { "all" };
    if values.iter().all(|v| Some(v) == values.first()) {
        return json!({"factor":factor,"scope":scope,"observations":n,"observed":null,"nullMean":null,"excess":null,"pValue":null,
            "reason":if n==0 {"no_data"} else {"zero_variance"}});
    }
    let mean = sum(values.iter().copied()) / n as f64;
    let total = sum(values.iter().map(|&v| (v - mean).powi(2)));
    let mut buckets = vec![Vec::new(); groups.len()];
    let mut eta = |order: &[usize]| {
        for g in &mut buckets {
            g.clear();
        }
        for (&g, &v) in order.iter().zip(&values) {
            buckets[g].push(v);
        }
        sum(buckets
            .iter()
            .map(|g| g.len() as f64 * (sum(g.iter().copied()) / g.len() as f64 - mean).powi(2)))
            / total
    };
    let observed = eta(&labels);
    let mut seed = metric.permutation_seed;
    let mut nulls = Vec::with_capacity(metric.permutation_count as usize);
    let mut draw = labels.clone();
    for _ in 0..metric.permutation_count {
        draw.copy_from_slice(&labels);
        for s in strata.values() {
            shuffle(s, &mut seed, |i, j| draw.swap(i, j));
        }
        nulls.push(eta(&draw));
    }
    let null_mean = sum(nulls.iter().copied()) / nulls.len() as f64;
    let p =
        (1 + nulls.iter().filter(|&&v| v >= observed).count()) as f64 / (1 + nulls.len()) as f64;
    json!({"factor":factor,"scope":scope,"observations":n,"observed":observed,"nullMean":null_mean,"excess":observed-null_mean,"pValue":p,"reason":null})
}

pub fn shuffle_with_seed<T>(
    items: &mut [T],
    mut seed: u64,
) -> Result<(), super::LensDefinitionError> {
    if !(1..2_147_483_647).contains(&seed) || items.len() > 2_147_483_646 {
        return Err(super::LensDefinitionError::InvalidDefinition);
    }
    let indices: Vec<_> = (0..items.len()).collect();
    shuffle(&indices, &mut seed, |i, j| items.swap(i, j));
    Ok(())
}
