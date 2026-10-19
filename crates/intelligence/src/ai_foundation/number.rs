//! Decimal strings preserve exact counters beyond JavaScript's integer range.
pub(crate) mod u64_wire {
    use serde::{Deserialize, Deserializer, Serializer, de::Error};
    pub fn serialize<S: Serializer>(value: &u64, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&value.to_string())
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Amount {
            Number(u64),
            Text(String),
        }
        match Amount::deserialize(deserializer)? {
            Amount::Number(value) => Ok(value),
            Amount::Text(text)
                if !text.is_empty()
                    && text.len() <= 20
                    && text.bytes().all(|b| b.is_ascii_digit()) =>
            {
                text.parse().map_err(D::Error::custom)
            }
            _ => Err(D::Error::custom("invalid_unsigned_decimal")),
        }
    }
}
pub(crate) mod optional_u64_wire {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    #[derive(Serialize, Deserialize)]
    struct Exact(#[serde(with = "super::u64_wire")] u64);
    pub fn serialize<S: Serializer>(value: &Option<u64>, serializer: S) -> Result<S::Ok, S::Error> {
        value.map(Exact).serialize(serializer)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<u64>, D::Error> {
        Ok(Option::<Exact>::deserialize(deserializer)?.map(|v| v.0))
    }
}

#[cfg(test)]
mod tests {
    use super::super::control::BudgetPolicy;
    #[test]
    fn exact_decimal_budget_does_not_round_large_values_or_accept_ambiguous_numbers() {
        let value = serde_json::json!({"mode":"capped","tokens":"9223372036854775807"});
        let parsed: BudgetPolicy = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(parsed).unwrap(), value);
        for text in ["-1", "1.5", "1e5", " 1", "18446744073709551616"] {
            assert!(
                serde_json::from_value::<BudgetPolicy>(
                    serde_json::json!({"mode":"capped","tokens":text})
                )
                .is_err()
            );
        }
    }
    #[test]
    fn exact_price_strings_distinguish_missing_zero_and_large_integer_rates() {
        use super::super::usage::TokenPrices;
        let value = serde_json::json!({"input":"9223372036854775807","output":"0","cachedRead":null,"cachedWrite":null});
        let prices: TokenPrices = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(prices.input, Some(i64::MAX as u64));
        assert_eq!(prices.output, Some(0));
        assert_eq!(prices.cached_read, None);
        assert_eq!(serde_json::to_value(prices).unwrap(), value);
        for bad in ["-1", "1.5", "1e6", "18446744073709551616"] {
            assert!(
                serde_json::from_value::<TokenPrices>(serde_json::json!({"input":bad})).is_err()
            );
        }
    }
}
