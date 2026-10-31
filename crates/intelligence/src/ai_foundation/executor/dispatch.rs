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
    call_with_submitted(db, item, adapter, request, None).await
}

pub(super) async fn call_with_submitted(
    db: &Database,
    item: &ClaimedItem,
    adapter: &PiAdapter,
    request: &PiRequest,
    submitted: Option<tokio::sync::oneshot::Sender<()>>,
) -> Option<PiResponse> {
    let response = adapter.call_with_submitted(request, submitted);
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
