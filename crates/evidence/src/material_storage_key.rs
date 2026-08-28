//! Storage-key grammar shared by media admission and derivative completion.

use std::path::Path;

pub(crate) fn is_safe_storage_key(value: &str) -> bool {
    let path = Path::new(value);
    !value.is_empty()
        && !path.is_absolute()
        && value
            .split('/')
            .all(|component| !component.is_empty() && !matches!(component, "." | ".."))
}
