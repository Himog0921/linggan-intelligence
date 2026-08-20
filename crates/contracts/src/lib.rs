//! Versioned boundary contracts. SCOPE-001 authorizes only the frozen
//! `content-detail.synthetic.v1` proof contract; real producer contracts require a later scope.

pub const BOOTSTRAP_CONTRACT_VERSION: &str = "unimplemented";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bootstrap_does_not_claim_a_runtime_contract() {
        assert_eq!(BOOTSTRAP_CONTRACT_VERSION, "unimplemented");
    }
}
