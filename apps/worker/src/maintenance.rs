//! Patrol owns deterministic catalog maintenance. It never loads secrets, Pi or embeddings.
use linggan_storage_postgres::Database;
use std::time::Duration;
use tokio::{sync::watch, task::JoinHandle};

pub const ATTEMPT_LIMIT: Duration = Duration::from_secs(15);

pub type MaintenanceTask = JoinHandle<Result<(), &'static str>>;

pub async fn run(db: Database, mut stop: watch::Receiver<bool>) -> Result<(), &'static str> {
    let mut tick = tokio::time::interval(Duration::from_secs(10));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut last_error = None;
    let mut last_unparseable_count = None;
    loop {
        if *stop.borrow() {
            return last_error.map_or(Ok(()), Err);
        }
        tokio::select! {
            changed=stop.changed()=>{if changed.is_err() {return last_error.map_or(Ok(()),Err);}},
            _=tick.tick()=>{
                let outcome=tokio::time::timeout(ATTEMPT_LIMIT,
                    linggan_intelligence::comment_study_catalog::maintain_comment_catalog(&db)).await;
                let error=match outcome {
                    Ok(Ok(Some(report)))=>{
                        let count=report.unparseable_observation_count;
                        if last_unparseable_count!=Some(count) && (count>0 || last_unparseable_count.is_some_and(|old| old>0)) {
                            // A source shared by two active domains contributes once per domain.
                            eprintln!("linggan worker: comment_observation_time_unparseable domain_observation_occurrences={count}");
                        }
                        last_unparseable_count=Some(count);
                        None
                    },
                    Ok(Ok(None))=>{last_unparseable_count=None;None},
                    Ok(Err(_))=>Some("catalog_maintenance_unavailable"),
                    Err(_)=>Some("catalog_maintenance_timed_out"),
                };
                if error!=last_error {
                    eprintln!("linggan worker: {}",error.unwrap_or("catalog_maintenance_recovered"));
                }
                last_error=error;
            }
        }
    }
}
