//! Pure statistical functions from playbook §6.4.1. No database access, no configuration reads.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Interval {
    pub low: f64,
    pub high: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatsError {
    EmptySample,
    CountExceedsTotal { k: u64, n: u64 },
    InvalidZ,
}

/// Wilson score interval. `n = 0` returns `EmptySample`; callers map that to null (R5).
pub fn wilson_interval(k: u64, n: u64, z: f64) -> Result<Interval, StatsError> {
    if !(z.is_finite() && z > 0.0) {
        return Err(StatsError::InvalidZ);
    }
    if n == 0 {
        return Err(StatsError::EmptySample);
    }
    if k > n {
        return Err(StatsError::CountExceedsTotal { k, n });
    }
    let n_f = n as f64;
    let p = k as f64 / n_f;
    let z2 = z * z;
    let denom = 1.0 + z2 / n_f;
    let center = p + z2 / (2.0 * n_f);
    let margin = z * (p * (1.0 - p) / n_f + z2 / (4.0 * n_f * n_f)).sqrt();
    // The exact endpoint is known in these cases. Floating cancellation can otherwise
    // give two zero-count samples different tiny positive lower bounds when ranking.
    let low = if k == 0 {
        0.0
    } else {
        ((center - margin) / denom).clamp(0.0, 1.0)
    };
    let high = if k == n {
        1.0
    } else {
        ((center + margin) / denom).clamp(0.0, 1.0)
    };
    Ok(Interval { low, high })
}

pub fn rate_per_1k(k: u64, n: u64) -> Option<f64> {
    if n == 0 {
        None
    } else {
        Some(k as f64 * 1000.0 / n as f64)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceTier {
    Candidate,
    Established,
    Significant,
}

#[derive(Debug, Clone, Copy)]
pub struct TierInput {
    pub mentioners_total: u64,
    pub k_cur: u64,
    pub n_cur: u64,
    pub k_base: u64,
    pub n_base: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct TierParams {
    pub z: f64,
    pub min_k_significant: u64,
    pub min_n_period: u64,
    pub min_total_for_significant: u64,
}

pub fn evidence_tier(input: &TierInput, params: &TierParams) -> EvidenceTier {
    if input.mentioners_total < 2 {
        return EvidenceTier::Candidate;
    }
    if input.mentioners_total < params.min_total_for_significant
        || input.k_cur < params.min_k_significant
        || input.n_cur < params.min_n_period
        || input.n_base < params.min_n_period
    {
        return EvidenceTier::Established;
    }
    let baseline = input.k_base as f64 / input.n_base as f64;
    match wilson_interval(input.k_cur, input.n_cur, params.z) {
        Ok(interval) if interval.low > baseline => EvidenceTier::Significant,
        _ => EvidenceTier::Established,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STANDARD_Z: f64 = 1.959963984540054;

    fn tier_params() -> TierParams {
        TierParams {
            z: STANDARD_Z,
            min_k_significant: 3,
            min_n_period: 500,
            min_total_for_significant: 5,
        }
    }

    #[test]
    fn wilson_interval_matches_every_standard_answer() {
        let cases = [
            (7, 120, 0.028541, 0.115526),
            (3, 40, 0.025836, 0.198642),
            (5, 96, 0.022450, 0.116185),
            (6, 1850, 0.001487, 0.007058),
            (0, 10, 0.000000, 0.277533),
            (10, 10, 0.722467, 1.000000),
            (1, 1, 0.206549, 1.000000),
            (50, 100, 0.403832, 0.596168),
            (10, 1000, 0.005441, 0.018309),
            (11, 1003, 0.006135, 0.019531),
            (3, 1000, 0.001021, 0.008783),
            (4, 60, 0.026229, 0.159254),
        ];
        for (k, n, low, high) in cases {
            let interval = wilson_interval(k, n, STANDARD_Z).unwrap();
            assert!((interval.low - low).abs() < 1e-6, "low for {k}/{n}");
            assert!((interval.high - high).abs() < 1e-6, "high for {k}/{n}");
        }
    }

    #[test]
    fn wilson_interval_rejects_invalid_inputs_instead_of_defaulting() {
        assert_eq!(
            wilson_interval(0, 0, STANDARD_Z),
            Err(StatsError::EmptySample)
        );
        assert_eq!(
            wilson_interval(5, 3, STANDARD_Z),
            Err(StatsError::CountExceedsTotal { k: 5, n: 3 })
        );
        assert_eq!(wilson_interval(1, 2, 0.0), Err(StatsError::InvalidZ));
        assert_eq!(wilson_interval(1, 2, f64::NAN), Err(StatsError::InvalidZ));
    }

    #[test]
    fn wilson_zero_and_full_counts_have_exact_endpoints() {
        for n in [1, 40, 100, 120, 1850] {
            assert_eq!(wilson_interval(0, n, STANDARD_Z).unwrap().low, 0.0);
            assert_eq!(wilson_interval(n, n, STANDARD_Z).unwrap().high, 1.0);
        }
    }

    #[test]
    fn rate_per_1k_requires_a_nonempty_sample() {
        assert_eq!(rate_per_1k(0, 0), None);
        assert!((rate_per_1k(7, 120).unwrap() - 58.333333333333336).abs() < 1e-9);
    }

    #[test]
    fn evidence_tier_matches_every_standard_answer() {
        let cases = [
            (
                TierInput {
                    mentioners_total: 14,
                    k_cur: 10,
                    n_cur: 1000,
                    k_base: 4,
                    n_base: 4800,
                },
                EvidenceTier::Significant,
            ),
            (
                TierInput {
                    mentioners_total: 3,
                    k_cur: 3,
                    n_cur: 1000,
                    k_base: 0,
                    n_base: 4800,
                },
                EvidenceTier::Established,
            ),
            (
                TierInput {
                    mentioners_total: 1,
                    k_cur: 1,
                    n_cur: 1000,
                    k_base: 0,
                    n_base: 4800,
                },
                EvidenceTier::Candidate,
            ),
            (
                TierInput {
                    mentioners_total: 20,
                    k_cur: 2,
                    n_cur: 1000,
                    k_base: 4,
                    n_base: 4800,
                },
                EvidenceTier::Established,
            ),
            (
                TierInput {
                    mentioners_total: 20,
                    k_cur: 10,
                    n_cur: 400,
                    k_base: 4,
                    n_base: 4800,
                },
                EvidenceTier::Established,
            ),
            (
                TierInput {
                    mentioners_total: 20,
                    k_cur: 10,
                    n_cur: 1000,
                    k_base: 4,
                    n_base: 300,
                },
                EvidenceTier::Established,
            ),
            (
                TierInput {
                    mentioners_total: 20,
                    k_cur: 3,
                    n_cur: 1000,
                    k_base: 30,
                    n_base: 4800,
                },
                EvidenceTier::Established,
            ),
        ];
        for (input, expected) in cases {
            assert_eq!(evidence_tier(&input, &tier_params()), expected, "{input:?}");
        }
    }

    struct Lcg(u64);

    impl Lcg {
        fn next_u64(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            self.0
        }

        fn below(&mut self, upper: u64) -> u64 {
            self.next_u64() % upper
        }
    }

    fn sample(rng: &mut Lcg) -> (u64, u64) {
        let n = rng.below(100_001);
        let k = if n == 0 { 0 } else { rng.below(n + 1) };
        (k, n)
    }

    #[test]
    fn property_interval_brackets_the_point_estimate() {
        let mut rng = Lcg(0x9e37_79b9_7f4a_7c15);
        for _ in 0..10_000 {
            let (k, n) = sample(&mut rng);
            let interval = match wilson_interval(k, n, STANDARD_Z) {
                Ok(interval) => interval,
                Err(StatsError::EmptySample) => continue,
                Err(error) => panic!("unexpected error for {k}/{n}: {error:?}"),
            };
            let p = k as f64 / n as f64;
            assert!(0.0 <= interval.low && interval.low <= p);
            assert!(p <= interval.high && interval.high <= 1.0);
        }
    }

    #[test]
    fn property_low_bound_is_monotone_in_the_count() {
        let mut rng = Lcg(0xbf58_476d_1ce4_e5b9);
        for _ in 0..10_000 {
            let (k, n) = sample(&mut rng);
            if n == 0 || k == n {
                continue;
            }
            let lower = wilson_interval(k, n, STANDARD_Z).unwrap().low;
            let higher = wilson_interval(k + 1, n, STANDARD_Z).unwrap().low;
            assert!(
                lower <= higher,
                "low decreased for {k}/{n} -> {}/{n}",
                k + 1
            );
        }
    }

    #[test]
    fn property_interval_width_does_not_grow_with_the_sample_size() {
        let mut rng = Lcg(0x94d0_49bb_1331_11eb);
        for _ in 0..10_000 {
            let (k, n) = sample(&mut rng);
            if k == 0 || n == 0 || n > 50_000 {
                continue;
            }
            let width = wilson_interval(k, n, STANDARD_Z).unwrap().high
                - wilson_interval(k, n, STANDARD_Z).unwrap().low;
            let doubled_k = 2 * k;
            let doubled_n = 2 * n;
            let doubled = wilson_interval(doubled_k, doubled_n, STANDARD_Z).unwrap();
            assert!(
                doubled.high - doubled.low <= width + 1e-12,
                "width grew for {k}/{n} -> {doubled_k}/{doubled_n}"
            );
        }
    }
}
