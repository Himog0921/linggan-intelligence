//! Fixed bounded-review prompt and response shape.
//!
//! This module contains no database or provider access.  It only makes the
//! allowed model decision and its sampled evidence explicit for the worker.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ReviewSampleRoles {
    pub(super) core_atom_refs: Vec<Uuid>,
    pub(super) boundary_atom_refs: Vec<Uuid>,
    pub(super) neighbor_atom_refs: Vec<Uuid>,
}

impl ReviewSampleRoles {
    pub(super) fn role_for(&self, atom_ref: Uuid) -> Option<&'static str> {
        if self.core_atom_refs.contains(&atom_ref) {
            Some("core")
        } else if self.boundary_atom_refs.contains(&atom_ref) {
            Some("boundary")
        } else if self.neighbor_atom_refs.contains(&atom_ref) {
            Some("neighbor")
        } else {
            None
        }
    }

    fn valid_for(&self, sample_refs: &[Uuid]) -> bool {
        if self.core_atom_refs.is_empty()
            || self.core_atom_refs.len() > 5
            || self.boundary_atom_refs.len() > 4
            || self.neighbor_atom_refs.len() > 3
        {
            return false;
        }
        let mut combined = Vec::with_capacity(
            self.core_atom_refs.len()
                + self.boundary_atom_refs.len()
                + self.neighbor_atom_refs.len(),
        );
        combined.extend(self.core_atom_refs.iter().copied());
        combined.extend(self.boundary_atom_refs.iter().copied());
        combined.extend(self.neighbor_atom_refs.iter().copied());
        combined == sample_refs
            && combined
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                == combined.len()
    }
}

pub(super) struct ReviewExpression {
    pub(super) atom_ref: Uuid,
    pub(super) meaning: String,
    pub(super) target: Option<String>,
    pub(super) position: Option<String>,
}

pub(super) fn sample_roles(receipt: &Value, sample_refs: &[Uuid]) -> Option<ReviewSampleRoles> {
    let roles: ReviewSampleRoles =
        serde_json::from_value(receipt.get("sampleRoles")?.clone()).ok()?;
    roles.valid_for(sample_refs).then_some(roles)
}

pub(super) fn review_system() -> &'static str {
    "你只能审查给出的同类型原子表达。返回一个JSON对象；same表示这些表达可保守组成一个独立语义组，independent表示保持独立。不得调用工具，不得声称外部事实、Topic、市场结论或用户身份。"
}

pub(super) fn review_prompt(
    kind: String,
    algorithm: String,
    cluster_key: String,
    expressions: &[ReviewExpression],
    roles: &ReviewSampleRoles,
) -> String {
    let atoms = expressions
        .iter()
        .map(|expression| {
            let mut atom = serde_json::Map::new();
            atom.insert("atomRef".into(), json!(expression.atom_ref));
            atom.insert(
                "sampleRole".into(),
                json!(roles.role_for(expression.atom_ref)),
            );
            atom.insert("meaning".into(), json!(&expression.meaning));
            if let Some(target) = &expression.target {
                atom.insert("target".into(), json!(target));
            }
            if let Some(position) = &expression.position {
                atom.insert("position".into(), json!(position));
            }
            Value::Object(atom)
        })
        .collect::<Vec<_>>();
    json!({
        "task":"bounded_semantic_cluster_review.v1",
        "kind":kind,
        "algorithm":algorithm,
        "clusterKey":cluster_key,
        "atoms":atoms,
        "response":{"decision":"same|independent","sameScope":"same时cluster|core；边界不一致时必须core","name":"same时1-120字符","definition":"same时1-1000字符","coreAtomRefs":"same时给1-5个输入atomRef","relatedAtomRefs":"same时给0-3个输入atomRef"}
    })
    .to_string()
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ReviewDecision {
    pub(super) decision: String,
    pub(super) name: Option<String>,
    pub(super) definition: Option<String>,
    #[serde(default)]
    pub(super) same_scope: Option<String>,
    #[serde(default)]
    pub(super) core_atom_refs: Vec<Uuid>,
    #[serde(default)]
    pub(super) related_atom_refs: Vec<Uuid>,
}
