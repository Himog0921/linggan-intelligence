use std::time::Duration;

/// Request-local elapsed time only. No identifiers, materials or shared state.
#[derive(Default, Debug)]
pub struct ReadTimings {
    pub pool: Duration,
    pub scope: Duration,
    pub current: Duration,
    pub qualification: Duration,
    pub projection: Duration,
    pub dossier: Duration,
    pub aggregate: Duration,
    pub representatives: Duration,
    pub read: Duration,
}

impl ReadTimings {
    pub fn server_timing(&self) -> String {
        [
            ("pool", self.pool),
            ("scope", self.scope),
            ("current", self.current),
            ("qualification", self.qualification),
            ("projection", self.projection),
            ("dossier", self.dossier),
            ("aggregate", self.aggregate),
            ("representatives", self.representatives),
            ("read", self.read),
        ]
        .into_iter()
        .map(|(name, duration)| format!("{name};dur={:.2}", duration.as_secs_f64() * 1000.0))
        .collect::<Vec<_>>()
        .join(", ")
    }
}
