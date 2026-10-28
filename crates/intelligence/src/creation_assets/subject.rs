//! What the checks and the copy format look at: a snapshot of one draft plus the assets it
//! depends on. Building it is the only place that reads a `WorkDocument`.
use super::types::{BridgeState, CardRevision, Identity, bridge_state_of};
use crate::content_workbench::{BridgeChoice, PageKind, WorkDocument};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct Subject {
    pub has_account: bool,
    pub title: String,
    pub cover_title: String,
    pub paragraphs: Vec<(Uuid, String)>,
    pub experience_gaps: Vec<Uuid>,
    pub tags: Vec<String>,
    pub bridge: Option<BridgeChoice>,
    pub cover_ai: Option<bool>,
}

impl Subject {
    pub fn from_document(doc: &WorkDocument) -> Self {
        Self {
            has_account: doc.account_ref.is_some(),
            title: doc.title.clone(),
            cover_title: doc
                .pages
                .iter()
                .find(|page| page.kind == PageKind::Cover)
                .map(|page| page.title.clone())
                .unwrap_or_default(),
            paragraphs: doc
                .paragraphs
                .iter()
                .map(|p| (p.id, p.text.clone()))
                .collect(),
            experience_gaps: doc
                .experience_gaps
                .iter()
                .copied()
                .chain(doc.experience_slots.iter().filter_map(|slot| {
                    doc.paragraphs
                        .iter()
                        .find(|p| p.id == slot.paragraph_id)
                        .filter(|p| p.text.trim().is_empty())
                        .map(|p| p.id)
                }))
                .collect(),
            tags: doc.tags.clone(),
            bridge: doc.bridge.clone(),
            cover_ai: doc.cover_ai,
        }
    }
    pub fn skipped(&self) -> bool {
        self.bridge.as_ref().is_some_and(|bridge| bridge.skip)
    }
}

/// The names of the confirmed product fact cards. `None` in [`Context`] means no card is
/// confirmed yet, so name-based rules cannot run.
#[derive(Clone, Debug, Default)]
pub struct CardTerms {
    /// The canonical name of the first confirmed card, used when suggesting wording.
    pub canonical: String,
    /// Canonical names plus the allowed spellings of every confirmed card.
    pub allowed: Vec<String>,
    pub banned: Vec<String>,
    pub revisions: Vec<CardRevision>,
}

#[derive(Clone, Debug, Default)]
pub struct Context {
    pub identity: Option<Identity>,
    pub cards: Option<CardTerms>,
}

impl Context {
    pub fn bridge_state(&self, subject: &Subject) -> BridgeState {
        bridge_state_of(
            self.identity.as_ref(),
            subject.has_account,
            subject.skipped(),
        )
    }
    /// The bridge sentence that would be written into the note: only when the account may carry
    /// one and a sentence was chosen. Everything else prints nothing (spec A.4).
    pub fn bridge_output<'a>(&self, subject: &'a Subject) -> Option<(&'a str, Option<Uuid>)> {
        if self.bridge_state(subject) != BridgeState::Ready {
            return None;
        }
        let bridge = subject.bridge.as_ref()?;
        let sentence = bridge.sentence.as_ref()?;
        Some((sentence.text.as_str(), bridge.after_paragraph))
    }
}
