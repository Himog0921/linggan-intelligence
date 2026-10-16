//! Preserved return navigation for existing manual observation command receipts.
use super::target_drawer::percent_encode_component;
use uuid::Uuid;
pub(super) fn return_path(
    target_ref: Uuid,
    domain: Option<&str>,
    receipt_ref: Option<Uuid>,
    error: Option<&str>,
) -> String {
    let mut path = action_path(domain);
    path.push_str(&format!("&target={target_ref}"));
    if let Some(receipt_ref) = receipt_ref {
        path.push_str(&format!("&receipt={receipt_ref}"));
    }
    if let Some(error) = error {
        path.push_str(&format!("&error={}", percent_encode_component(error)));
    }
    path
}

fn page_path(domain: Option<&str>) -> String {
    domain.map_or_else(
        || "/collection/tasks".to_owned(),
        |domain| {
            format!(
                "/collection/tasks?domain={}",
                percent_encode_component(domain)
            )
        },
    )
}

fn action_path(domain: Option<&str>) -> String {
    let path = page_path(domain);
    let separator = if domain.is_some() { '&' } else { '?' };
    format!("{path}{separator}once=1")
}
