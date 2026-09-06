//! No secret is returned by settings reads. Production never falls back to plaintext storage.
use crate::model_settings::ModelError;
use std::sync::Arc;
use uuid::Uuid;

pub trait ModelSecretStore: Send + Sync {
    fn put(&self, workspace: Uuid, reference: Uuid, secret: &str) -> Result<(), ModelError>;
    fn get(&self, workspace: Uuid, reference: Uuid) -> Result<String, ModelError>;
    fn delete(&self, workspace: Uuid, reference: Uuid) -> Result<(), ModelError>;
    fn is_synthetic(&self) -> bool {
        false
    }
}
pub struct KeychainModelSecrets;
#[cfg(target_os = "macos")]
fn entry(workspace: Uuid, reference: Uuid) -> Result<keyring::Entry, ModelError> {
    keyring::Entry::new(
        &format!("Linggan.Intelligence.Models.{workspace}"),
        &reference.to_string(),
    )
    .map_err(|_| ModelError::SecretUnavailable)
}
#[cfg(target_os = "macos")]
impl ModelSecretStore for KeychainModelSecrets {
    fn put(&self, w: Uuid, r: Uuid, s: &str) -> Result<(), ModelError> {
        entry(w, r)?
            .set_password(s)
            .map_err(|_| ModelError::SecretUnavailable)
    }
    fn get(&self, w: Uuid, r: Uuid) -> Result<String, ModelError> {
        entry(w, r)?
            .get_password()
            .map_err(|_| ModelError::SecretUnavailable)
    }
    fn delete(&self, w: Uuid, r: Uuid) -> Result<(), ModelError> {
        entry(w, r)?
            .delete_credential()
            .map_err(|_| ModelError::SecretUnavailable)
    }
}
#[cfg(not(target_os = "macos"))]
impl ModelSecretStore for KeychainModelSecrets {
    fn put(&self, _: Uuid, _: Uuid, _: &str) -> Result<(), ModelError> {
        Err(ModelError::SecretUnavailable)
    }
    fn get(&self, _: Uuid, _: Uuid) -> Result<String, ModelError> {
        Err(ModelError::SecretUnavailable)
    }
    fn delete(&self, _: Uuid, _: Uuid) -> Result<(), ModelError> {
        Err(ModelError::SecretUnavailable)
    }
}
/// Only the deliberately separate synthetic preview uses this store. It accepts one public
/// fixture marker, never a user's key, and permits no non-loopback connection.
pub struct SyntheticModelSecrets;
impl ModelSecretStore for SyntheticModelSecrets {
    fn put(&self, _: Uuid, _: Uuid, s: &str) -> Result<(), ModelError> {
        if s == "SYNTHETIC-NOT-A-CREDENTIAL" {
            Ok(())
        } else {
            Err(ModelError::SecretUnavailable)
        }
    }
    fn get(&self, _: Uuid, _: Uuid) -> Result<String, ModelError> {
        Ok("SYNTHETIC-NOT-A-CREDENTIAL".into())
    }
    fn delete(&self, _: Uuid, _: Uuid) -> Result<(), ModelError> {
        Ok(())
    }
    fn is_synthetic(&self) -> bool {
        true
    }
}
pub fn model_secret_store() -> Arc<dyn ModelSecretStore> {
    if std::env::var("LINGGAN_MODEL_SYNTHETIC_PREVIEW").as_deref() == Ok("SYNTHETIC-NOT-EVIDENCE") {
        Arc::new(SyntheticModelSecrets)
    } else {
        Arc::new(KeychainModelSecrets)
    }
}
