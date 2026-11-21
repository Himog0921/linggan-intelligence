use super::{AnalysisInput, LabelDimension, MetricDefinition, PublicationInput};
use serde::Deserialize;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SampleMetric {
    Publications,
    Views,
    WithinAccountViews,
    OverThousand,
    Hit,
    ActiveComments,
    CommentsPerThousandViews,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Subset {
    All,
    Numerator,
    Denominator,
}

#[derive(Debug, Default, Deserialize)]
pub struct LabelFilter {
    pub moment: Option<String>,
    pub offer: Option<String>,
    pub situation: Option<String>,
}

impl LabelFilter {
    fn matches(&self, row: &PublicationInput) -> bool {
        [
            (&self.moment, LabelDimension::Moment),
            (&self.offer, LabelDimension::Offer),
            (&self.situation, LabelDimension::Situation),
        ]
        .into_iter()
        .all(|(key, dim)| {
            key.as_ref()
                .is_none_or(|k| row.labels.get(&dim).is_some_and(|l| &l.value == k))
        })
    }
}

impl SampleMetric {
    pub(super) fn contains(
        self,
        row: &PublicationInput,
        subset: Subset,
        m: &MetricDefinition,
    ) -> bool {
        let numerator = matches!(subset, Subset::Numerator);
        match self {
            Self::Publications => true,
            Self::Views | Self::WithinAccountViews => row.views.value.is_some(),
            Self::OverThousand => row
                .views
                .value
                .is_some_and(|v| !numerator || v >= m.views_threshold_mid),
            Self::Hit => row
                .views
                .value
                .is_some_and(|v| !numerator || v >= m.views_threshold_high),
            Self::ActiveComments => row
                .comments
                .value
                .is_some_and(|v| !numerator || v >= m.comments_threshold_active),
            Self::CommentsPerThousandViews => {
                row.comments.value.is_some() && row.views.value.is_some()
            }
        }
    }
}

/// Uses only the saved input and its metric version; pagination can follow selection.
pub fn select_publications(
    input: &AnalysisInput,
    filter: &LabelFilter,
    metric: SampleMetric,
    subset: Subset,
    m: &MetricDefinition,
) -> Vec<Uuid> {
    let rows = input
        .publications
        .iter()
        .filter(|(_, row)| !row.labels.is_empty())
        .map(|(id, row)| (*id, row))
        .collect::<Vec<_>>();
    let within = matches!(metric, SampleMetric::WithinAccountViews)
        .then(|| super::variance::residuals(&rows, m.min_account_posts));
    rows.into_iter()
        .filter(|(id, row)| {
            within
                .as_ref()
                .is_none_or(|retained| retained.contains_key(id))
                && filter.matches(row)
                && metric.contains(row, subset, m)
        })
        .map(|(id, _)| id)
        .collect()
}
