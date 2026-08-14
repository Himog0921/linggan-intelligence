//! Versioned boundary contracts. No contract is added without a real producer fixture.

pub const BOOTSTRAP_CONTRACT_VERSION: &str = "unimplemented";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bootstrap_does_not_claim_a_runtime_contract() {
        assert_eq!(BOOTSTRAP_CONTRACT_VERSION, "unimplemented");
    }
}
