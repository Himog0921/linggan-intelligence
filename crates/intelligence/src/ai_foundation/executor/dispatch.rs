//! A lost item lease stops publication; it does not discard an already-started bounded call.
use super::queue::ClaimedItem;
use crate::pi_adapter::{PiAdapter, PiRequest, PiResponse};
use linggan_storage_postgres::Database;
use std::time::Duration;

pub(super) async fn call(
    db: &Database,
    item: &ClaimedItem,
    adapter: &PiAdapter,
    request: &PiRequest,
) -> Option<PiResponse> {
    let response = adapter.call(request);
    let renewal = async {
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            match item.renew(db).await {
                Ok(()) => {}
                Err(error) if error.is_database() => {}
                Err(_) => break,
            }
        }
    };
    tokio::pin!(response, renewal);
    tokio::select! {
        result = &mut response => result.ok(),
        _ = &mut renewal => response.await.ok(),
    }
}
