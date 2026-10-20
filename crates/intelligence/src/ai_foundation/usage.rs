//! Preserve unknown and reserved usage. Cache/reasoning are subsets, never extra tokens.
use super::RuleError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DispatchEvidence {
    NotDispatched,
    Started,
    Unknown,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UsageState {
    Measured,
    UnknownReserved,
    NotDispatched,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TokenUsage {
    pub input: Option<u64>,
    pub output: Option<u64>,
    pub cached_read: Option<u64>,
    pub cached_write: Option<u64>,
    pub reasoning: Option<u64>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UsageSettlement {
    pub state: UsageState,
    pub measured_tokens: Option<u64>,
    pub quota_tokens: u64,
}
impl TokenUsage {
    pub fn validate(&self) -> Result<(), RuleError> {
        for n in [
            self.input,
            self.output,
            self.cached_read,
            self.cached_write,
            self.reasoning,
        ]
        .into_iter()
        .flatten()
        {
            if n > i64::MAX as u64 {
                return Err(RuleError::Usage);
            }
        }
        let cached = self
            .cached_read
            .unwrap_or(0)
            .checked_add(self.cached_write.unwrap_or(0))
            .ok_or(RuleError::Overflow)?;
        if self.input.is_some_and(|input| cached > input)
            || self
                .output
                .zip(self.reasoning)
                .is_some_and(|(output, reasoning)| reasoning > output)
        {
            return Err(RuleError::Usage);
        }
        Ok(())
    }
    pub fn settle(
        &self,
        reserved: u64,
        dispatch: DispatchEvidence,
    ) -> Result<UsageSettlement, RuleError> {
        self.validate()?;
        if reserved > i64::MAX as u64 {
            return Err(RuleError::Usage);
        }
        if dispatch == DispatchEvidence::NotDispatched {
            if [
                self.input,
                self.output,
                self.cached_read,
                self.cached_write,
                self.reasoning,
            ]
            .into_iter()
            .flatten()
            .any(|v| v != 0)
            {
                return Err(RuleError::Usage);
            }
            return Ok(UsageSettlement {
                state: UsageState::NotDispatched,
                measured_tokens: Some(0),
                quota_tokens: 0,
            });
        }
        match self.input.zip(self.output) {
            Some((input, output)) => {
                let total = input
                    .checked_add(output)
                    .filter(|v| *v <= i64::MAX as u64)
                    .ok_or(RuleError::Overflow)?;
                Ok(UsageSettlement {
                    state: UsageState::Measured,
                    measured_tokens: Some(total),
                    quota_tokens: total,
                })
            }
            None => {
                // Partial measurements are a lower bound even when a provider omitted the
                // totals. A large partial response must never release quota below that bound.
                let input = self.input.unwrap_or_else(|| {
                    self.cached_read.unwrap_or(0) + self.cached_write.unwrap_or(0)
                });
                let output = self.output.unwrap_or(self.reasoning.unwrap_or(0));
                let lower_bound = input
                    .checked_add(output)
                    .filter(|v| *v <= i64::MAX as u64)
                    .ok_or(RuleError::Overflow)?;
                Ok(UsageSettlement {
                    state: UsageState::UnknownReserved,
                    measured_tokens: None,
                    quota_tokens: reserved.max(lower_bound),
                })
            }
        }
    }
}

/// Micro-USD per million tokens. A quote is exact pico-USD; missing rates/details stay unknown.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TokenPrices {
    #[serde(default, with = "super::number::optional_u64_wire")]
    pub input: Option<u64>,
    #[serde(default, with = "super::number::optional_u64_wire")]
    pub output: Option<u64>,
    #[serde(default, with = "super::number::optional_u64_wire")]
    pub cached_read: Option<u64>,
    #[serde(default, with = "super::number::optional_u64_wire")]
    pub cached_write: Option<u64>,
}
impl TokenPrices {
    pub fn quote_pico_usd(&self, usage: &TokenUsage) -> Result<Option<u128>, RuleError> {
        usage.validate()?;
        let (Some(input), Some(output), Some(read), Some(write)) = (
            usage.input,
            usage.output,
            usage.cached_read,
            usage.cached_write,
        ) else {
            return Ok(None);
        };
        let components = [
            (input - read - write, self.input),
            (output, self.output),
            (read, self.cached_read),
            (write, self.cached_write),
        ];
        let mut total = 0_u128;
        for (count, price) in components {
            if count == 0 {
                continue;
            }
            let Some(price) = price else { return Ok(None) };
            total = total
                .checked_add(u128::from(count) * u128::from(price))
                .ok_or(RuleError::Overflow)?;
        }
        Ok(Some(total))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cache_and_reasoning_do_not_double_charge_and_unknown_does_not_become_zero() {
        let usage = TokenUsage {
            input: Some(100),
            output: Some(40),
            cached_read: Some(60),
            cached_write: Some(10),
            reasoning: Some(30),
        };
        assert_eq!(
            usage
                .settle(500, DispatchEvidence::Started)
                .unwrap()
                .measured_tokens,
            Some(140)
        );
        assert_eq!(
            usage
                .settle(500, DispatchEvidence::Started)
                .unwrap()
                .quota_tokens,
            140
        );
        let unknown = TokenUsage::default()
            .settle(500, DispatchEvidence::Unknown)
            .unwrap();
        assert_eq!(unknown.state, UsageState::UnknownReserved);
        assert_eq!(unknown.measured_tokens, None);
        assert_eq!(unknown.quota_tokens, 500);
        assert_eq!(
            TokenUsage {
                input: Some(700),
                ..Default::default()
            }
            .settle(500, DispatchEvidence::Unknown)
            .unwrap()
            .quota_tokens,
            700
        );
        assert_eq!(
            TokenUsage {
                cached_read: Some(700),
                reasoning: Some(90),
                ..Default::default()
            }
            .settle(500, DispatchEvidence::Started)
            .unwrap()
            .quota_tokens,
            790
        );
        assert_eq!(
            TokenUsage::default()
                .settle(500, DispatchEvidence::NotDispatched)
                .unwrap()
                .quota_tokens,
            0
        );
        assert!(usage.settle(500, DispatchEvidence::NotDispatched).is_err());
        assert!(
            TokenUsage {
                input: Some(10),
                cached_read: Some(11),
                ..Default::default()
            }
            .validate()
            .is_err()
        );
        assert!(
            TokenUsage {
                input: Some(i64::MAX as u64),
                output: Some(1),
                ..Default::default()
            }
            .settle(1, DispatchEvidence::Started)
            .is_err()
        );
    }
    #[test]
    fn cost_is_exact_and_missing_price_or_cache_information_stays_unknown() {
        let usage = TokenUsage {
            input: Some(100),
            output: Some(40),
            cached_read: Some(60),
            cached_write: Some(10),
            reasoning: Some(30),
        };
        let price = TokenPrices {
            input: Some(140_000),
            output: Some(280_000),
            cached_read: Some(2_800),
            cached_write: Some(0),
        };
        assert_eq!(price.quote_pico_usd(&usage).unwrap(), Some(15_568_000));
        assert_eq!(TokenPrices::default().quote_pico_usd(&usage).unwrap(), None);
        assert_eq!(
            price
                .quote_pico_usd(&TokenUsage {
                    input: Some(100),
                    output: Some(40),
                    ..Default::default()
                })
                .unwrap(),
            None
        );
        assert_eq!(
            TokenPrices::default()
                .quote_pico_usd(&TokenUsage {
                    input: Some(0),
                    output: Some(0),
                    cached_read: Some(0),
                    cached_write: Some(0),
                    reasoning: None
                })
                .unwrap(),
            Some(0)
        );
    }
}
