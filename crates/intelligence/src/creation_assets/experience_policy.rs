//! Eligibility for using a personal experience in a draft.
//!
//! This is the policy kernel, not a persistence or authorization implementation. The caller must
//! load the current status and consent from its authoritative store in the same transaction as
//! the operation using the experience. A caller-supplied assertion is not evidence of consent.
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExperienceState {
    AwaitingSubject,
    Confirmed,
    Disabled,
    Withdrawn,
}

/// The decision records why material was unavailable without containing the material itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExperienceUnavailable {
    MissingDraftAccount,
    DifferentAccount,
    AwaitingSubject,
    ConsentMissing,
    Disabled,
    Withdrawn,
}

/// An experience is usable only for its owning account, after the subject has confirmed this
/// revision and agreed to its use in drafts. `send_to_model` is intentionally not an input:
/// DEC-0012 records it but does not make it an additional external-send condition.
pub fn use_in_draft(
    owner: Uuid,
    draft_account: Option<Uuid>,
    state: ExperienceState,
    subject_confirmed_revision: bool,
    consent_for_drafts: bool,
) -> Result<(), ExperienceUnavailable> {
    let draft_account = draft_account.ok_or(ExperienceUnavailable::MissingDraftAccount)?;
    if owner != draft_account {
        return Err(ExperienceUnavailable::DifferentAccount);
    }
    match state {
        ExperienceState::AwaitingSubject => Err(ExperienceUnavailable::AwaitingSubject),
        ExperienceState::Disabled => Err(ExperienceUnavailable::Disabled),
        ExperienceState::Withdrawn => Err(ExperienceUnavailable::Withdrawn),
        ExperienceState::Confirmed if !subject_confirmed_revision || !consent_for_drafts => {
            Err(ExperienceUnavailable::ConsentMissing)
        }
        ExperienceState::Confirmed => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_owning_account_can_use_a_confirmed_and_consented_revision() {
        let owner = Uuid::new_v4();
        let other = Uuid::new_v4();
        assert_eq!(
            use_in_draft(owner, Some(owner), ExperienceState::Confirmed, true, true),
            Ok(())
        );
        assert_eq!(
            use_in_draft(owner, None, ExperienceState::Confirmed, true, true),
            Err(ExperienceUnavailable::MissingDraftAccount)
        );
        assert_eq!(
            use_in_draft(owner, Some(other), ExperienceState::Confirmed, true, true),
            Err(ExperienceUnavailable::DifferentAccount)
        );
        assert_eq!(
            use_in_draft(
                owner,
                Some(owner),
                ExperienceState::AwaitingSubject,
                false,
                false
            ),
            Err(ExperienceUnavailable::AwaitingSubject)
        );
        assert_eq!(
            use_in_draft(owner, Some(owner), ExperienceState::Confirmed, false, true),
            Err(ExperienceUnavailable::ConsentMissing)
        );
        assert_eq!(
            use_in_draft(owner, Some(owner), ExperienceState::Confirmed, true, false),
            Err(ExperienceUnavailable::ConsentMissing)
        );
        assert_eq!(
            use_in_draft(owner, Some(owner), ExperienceState::Disabled, true, true),
            Err(ExperienceUnavailable::Disabled)
        );
        assert_eq!(
            use_in_draft(owner, Some(owner), ExperienceState::Withdrawn, true, true),
            Err(ExperienceUnavailable::Withdrawn)
        );
    }
}
