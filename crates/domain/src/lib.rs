//! Pure domain language and invariants. Infrastructure dependencies are forbidden here.

pub mod comment_research;

pub mod lifecycle {
    /// The bootstrap intentionally distinguishes unknown facts from negative facts.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum KnowledgeState {
        Unknown,
        Observed,
    }
}

#[cfg(test)]
mod tests {
    use super::lifecycle::KnowledgeState;

    #[test]
    fn unknown_is_not_observed() {
        assert_ne!(KnowledgeState::Unknown, KnowledgeState::Observed);
    }
}
