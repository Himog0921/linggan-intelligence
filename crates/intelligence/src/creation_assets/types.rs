//! CREATION-ASSETS-001 S1a typed boundary. Commands deny unknown fields; revisions are append-only.
use crate::content_workbench::BridgeGroup;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Sensitive content is intentionally not Debug: it must not be written to ordinary logs.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Experience {
    pub id: Uuid,
    pub code: String,
    pub account_ref: Uuid,
    pub state: String,
    pub revision: i32,
    pub title: Option<String>,
    pub body: Option<String>,
    pub tags: Option<Vec<String>>,
    pub origin: Option<String>,
    pub send_to_model: Option<bool>,
    pub confirmed_at: Option<String>,
    pub created_at: String,
    pub withdrawn_at: Option<String>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExperienceCreate {
    pub request_id: Uuid,
    pub account_ref: Uuid,
    pub title: String,
    pub body: String,
    pub tags: Vec<String>,
    pub origin: String,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExperienceEdit {
    pub request_id: Uuid,
    pub expected_revision: i32,
    pub title: String,
    pub body: String,
    pub tags: Vec<String>,
    pub origin: String,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExperienceConfirm {
    pub request_id: Uuid,
    pub expected_revision: i32,
    /// The operator records an actual confirmation by the subject; this is not identity proof.
    pub subject_confirmed: bool,
    pub draft_use: bool,
    pub send_to_model: Option<bool>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExperienceReceipt {
    pub experience: Experience,
    pub replayed: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsableExperience {
    pub id: Uuid,
    pub code: String,
    pub revision: i32,
    pub body: String,
}

/// How an account relates to the organisation, as the operator knows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrgRelation {
    Parent,
    Coach,
    #[serde(rename = "none")]
    NoRelation,
    Unknown,
}
impl OrgRelation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Parent => "parent",
            Self::Coach => "coach",
            Self::NoRelation => "none",
            Self::Unknown => "unknown",
        }
    }
    pub fn from_db(value: &str) -> Option<Self> {
        Some(match value {
            "parent" => Self::Parent,
            "coach" => Self::Coach,
            "none" => Self::NoRelation,
            "unknown" => Self::Unknown,
            _ => return None,
        })
    }
    /// The voice a bridge sentence must use for this relation, when the relation allows one.
    pub fn group(self) -> Option<BridgeGroup> {
        match self {
            Self::Parent => Some(BridgeGroup::Parent),
            Self::Coach => Some(BridgeGroup::Coach),
            Self::NoRelation | Self::Unknown => None,
        }
    }
}
pub fn group_str(group: BridgeGroup) -> &'static str {
    match group {
        BridgeGroup::Parent => "parent",
        BridgeGroup::Coach => "coach",
    }
}
pub fn group_from_db(value: &str) -> Option<BridgeGroup> {
    match value {
        "parent" => Some(BridgeGroup::Parent),
        "coach" => Some(BridgeGroup::Coach),
        _ => None,
    }
}

/// One revision of what is known about an account. Unknown stays unknown (`None`), never "no".
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Identity {
    pub account_ref: Uuid,
    pub revision: i32,
    pub org_relation: OrgRelation,
    pub bio_discloses_relation: Option<bool>,
    pub bio_checked_at: Option<String>,
    pub bio_checked_by: Option<String>,
    pub has_real_person: Option<bool>,
    pub backing_person_role: Option<String>,
    pub can_provide_experience: Option<bool>,
    pub saved_at: String,
    pub saved_by: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IdentityCommand {
    pub request_id: Uuid,
    /// The revision the editor started from; 0 when the account has no identity yet.
    pub expected_revision: i32,
    pub org_relation: OrgRelation,
    pub bio_discloses_relation: Option<bool>,
    pub has_real_person: Option<bool>,
    /// A role such as "妈妈" or "老师", never a real name.
    pub backing_person_role: Option<String>,
    pub can_provide_experience: Option<bool>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityReceipt {
    pub identity: Identity,
    pub replayed: bool,
}

/// Where an account stands for the bridge sentence slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BridgeState {
    /// The draft has no account yet.
    NoAccount,
    /// The account's relation to the organisation is not confirmed.
    Unconfirmed,
    /// Confirmed that the account has no relation: nothing about the product is written.
    NoRelation,
    /// "这篇不植入".
    Skipped,
    /// A relation exists but the account bio does not say so.
    BioMissing,
    /// A bridge sentence may be used.
    Ready,
}
pub fn bridge_state_of(identity: Option<&Identity>, has_account: bool, skip: bool) -> BridgeState {
    if !has_account {
        return if skip {
            BridgeState::Skipped
        } else {
            BridgeState::NoAccount
        };
    }
    match identity.map(|i| (i.org_relation, i.bio_discloses_relation)) {
        None | Some((OrgRelation::Unknown, _)) => {
            if skip {
                BridgeState::Skipped
            } else {
                BridgeState::Unconfirmed
            }
        }
        Some((OrgRelation::NoRelation, _)) => BridgeState::NoRelation,
        Some((_, bio)) => {
            if skip {
                BridgeState::Skipped
            } else if bio == Some(true) {
                BridgeState::Ready
            } else {
                BridgeState::BioMissing
            }
        }
    }
}

/// An account the creation module may write for, with its latest identity revision if any.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountView {
    pub account_ref: Uuid,
    pub account_name: String,
    pub platform: String,
    pub status: String,
    pub operator_name: Option<String>,
    pub identity: Option<Identity>,
}

/// One immutable revision of a product fact card. Only confirmed revisions drive checks.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FactCard {
    pub code: String,
    pub revision: i32,
    pub name: String,
    pub allowed_names: Vec<String>,
    pub banned_names: Vec<String>,
    pub what: String,
    pub who: String,
    pub can_say: String,
    pub cannot_say: String,
    pub must_note: String,
    pub confirmed_by: Option<String>,
    pub confirmed_at: Option<String>,
    pub saved_at: String,
    pub saved_by: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FactCardCommand {
    pub request_id: Uuid,
    /// 0 creates a new card; otherwise the latest revision the editor started from.
    pub expected_revision: i32,
    pub name: String,
    pub allowed_names: Vec<String>,
    pub banned_names: Vec<String>,
    pub what: String,
    pub who: String,
    pub can_say: String,
    pub cannot_say: String,
    pub must_note: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfirmCommand {
    pub request_id: Uuid,
    pub expected_revision: i32,
}
/// The latest revision plus the one currently in force (the latest confirmed one).
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FactCardView {
    pub code: String,
    pub latest: FactCard,
    pub confirmed: Option<FactCard>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FactCardReceipt {
    pub card: FactCard,
    pub replayed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SentenceStatus {
    /// 待审
    Review,
    /// 可用
    Ok,
    /// 停用
    Off,
    /// 待改
    Fix,
}
impl SentenceStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Review => "review",
            Self::Ok => "ok",
            Self::Off => "off",
            Self::Fix => "fix",
        }
    }
    pub fn from_db(value: &str) -> Option<Self> {
        Some(match value {
            "review" => Self::Review,
            "ok" => Self::Ok,
            "off" => Self::Off,
            "fix" => Self::Fix,
            _ => return None,
        })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SentenceOrigin {
    Manual,
    SavedBack,
}
impl SentenceOrigin {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::SavedBack => "saved_back",
        }
    }
    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "manual" => Some(Self::Manual),
            "saved_back" => Some(Self::SavedBack),
            _ => None,
        }
    }
}
/// One revision of a library sentence with the status it currently holds.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sentence {
    pub code: String,
    pub revision: i32,
    pub group: BridgeGroup,
    pub text: String,
    pub status: SentenceStatus,
    pub status_note: Option<String>,
    pub origin: SentenceOrigin,
    /// The entry checks as they were when this revision was added.
    pub entry_findings: Vec<Finding>,
    /// Drafts whose current revision uses this sentence.
    pub used_count: i64,
    pub saved_at: String,
    pub saved_by: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SentenceCreate {
    pub request_id: Uuid,
    pub group: BridgeGroup,
    pub text: String,
    pub origin: SentenceOrigin,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SentenceEdit {
    pub request_id: Uuid,
    pub expected_revision: i32,
    pub group: BridgeGroup,
    pub text: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewAction {
    Approve,
    RequestFix,
    Disable,
    Enable,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SentenceReview {
    pub request_id: Uuid,
    pub expected_revision: i32,
    pub action: ReviewAction,
    pub note: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SentenceReceipt {
    pub sentence: Sentence,
    pub replayed: bool,
}
/// The usable sentences for an account, in the order "换一句" cycles through them.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidates {
    pub state: BridgeState,
    pub group: Option<BridgeGroup>,
    pub items: Vec<Sentence>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    /// 必改：挡住复制
    Must,
    /// 提醒：不挡复制
    Warn,
    /// 提示：只告知
    Hint,
}
/// Where a finding sits. Offsets are UTF-16 code units, i.e. JavaScript string indexes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Location {
    Paragraph {
        id: Uuid,
        start: usize,
        end: usize,
    },
    Title {
        start: usize,
        end: usize,
    },
    Cover {
        start: usize,
        end: usize,
    },
    Bridge {
        start: usize,
        end: usize,
    },
    /// Standalone text such as a sentence being added to the library.
    Text {
        start: usize,
        end: usize,
    },
    Tag {
        index: usize,
    },
    /// The bridge slot or the account choice rather than any text.
    Slot,
}
/// Something the editor can do about a finding. The browser applies it through the normal save.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "camelCase")]
pub enum FixAction {
    ChooseAccount,
    OpenIdentity,
    SkipBridge,
    CopyBioSuggestion { text: String },
    ConfirmBioWritten,
    SwapBridge,
    AutoPickBridge,
    EditByHand,
    DeleteMatch,
    ReplaceText { replacement: String },
    RemoveTag,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fix {
    pub label: String,
    #[serde(flatten)]
    pub action: FixAction,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub rule: String,
    pub level: Level,
    pub message: String,
    pub location: Location,
    pub fixes: Vec<Fix>,
}
/// A rule that could not run, so that silence is never read as "passed".
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotRun {
    pub rule: String,
    pub reason: String,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardRevision {
    pub code: String,
    pub revision: i32,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckReport {
    pub ruleset_version: String,
    pub bridge_state: BridgeState,
    pub bridge_group: Option<BridgeGroup>,
    pub findings: Vec<Finding>,
    pub not_run: Vec<NotRun>,
    pub fact_cards: Vec<CardRevision>,
}
impl CheckReport {
    pub fn count(&self, level: Level) -> usize {
        self.findings.iter().filter(|f| f.level == level).count()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SentenceCheckRequest {
    pub text: String,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SentenceCheckReport {
    pub ruleset_version: String,
    pub blocked: bool,
    pub findings: Vec<Finding>,
    pub not_run: Vec<NotRun>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CopyCommand {
    pub request_id: Uuid,
    /// The revision the editor is looking at; anything newer makes the copy stale.
    pub expected_revision: Uuid,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CopyConfirm {
    pub request_id: Uuid,
}
/// Text ready for the clipboard, plus the receipt that says which rules it was checked under.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyReady {
    pub receipt_ref: Uuid,
    pub work_id: Uuid,
    pub revision_id: Uuid,
    pub ruleset_version: String,
    pub title: String,
    pub body: String,
    pub tags: Vec<String>,
    /// Body followed by the topic line, exactly what goes to the clipboard.
    pub text: String,
    pub warn_count: usize,
    pub hint_count: usize,
    pub replayed: bool,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyStatus {
    pub copied: Option<CopiedMark>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CopiedMark {
    pub receipt_ref: Uuid,
    pub revision_id: Uuid,
    pub copied_at: String,
}
