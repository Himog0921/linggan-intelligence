//! PostgreSQL adapter. It owns connections, storage errors and proof-database helpers only;
//! business SQL stays in the crate that owns the invariant it protects.

mod error;
mod pool;
pub mod testing;

pub use error::StorageError;
pub use pool::Database;
