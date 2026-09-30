//! One owner-only local store for model credentials, outside Git and PostgreSQL.
use crate::model_settings::ModelError;
use std::{
    env,
    fs::{self, DirBuilder, OpenOptions},
    io::Write,
    os::unix::{fs::DirBuilderExt, fs::OpenOptionsExt, fs::PermissionsExt},
    path::{Path, PathBuf},
    sync::Arc,
};
use uuid::Uuid;

const MAX_SECRET_BYTES: u64 = 16 * 1024;

pub trait ModelSecretStore: Send + Sync {
    fn put(&self, workspace: Uuid, reference: Uuid, secret: &str) -> Result<(), ModelError>;
    fn get(&self, workspace: Uuid, reference: Uuid) -> Result<String, ModelError>;
    fn delete(&self, workspace: Uuid, reference: Uuid) -> Result<(), ModelError>;
    fn is_synthetic(&self) -> bool {
        false
    }
}

pub struct LocalFileModelSecrets {
    root: Option<PathBuf>,
}

impl LocalFileModelSecrets {
    pub fn new(root: PathBuf) -> Self {
        Self { root: Some(root) }
    }

    fn from_runtime() -> Self {
        let support = env::var_os("LINGGAN_SUPPORT_DIR")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                env::var_os("HOME")
                    .filter(|value| !value.is_empty())
                    .map(|home| {
                        PathBuf::from(home).join("Library/Application Support/Linggan Intelligence")
                    })
            });
        Self {
            root: support.map(|path| path.join("model-secrets")),
        }
    }

    fn paths(&self, workspace: Uuid, reference: Uuid) -> Result<(PathBuf, PathBuf), ModelError> {
        let root = self.root.as_ref().ok_or(ModelError::SecretUnavailable)?;
        if !root.is_absolute() {
            return Err(ModelError::SecretUnavailable);
        }
        let directory = root.join(workspace.to_string());
        let file = directory.join(reference.to_string());
        Ok((directory, file))
    }
}

impl ModelSecretStore for LocalFileModelSecrets {
    fn put(&self, workspace: Uuid, reference: Uuid, secret: &str) -> Result<(), ModelError> {
        if secret.len() as u64 > MAX_SECRET_BYTES {
            return Err(ModelError::SecretUnavailable);
        }
        let (directory, destination) = self.paths(workspace, reference)?;
        ensure_private_dir(directory.parent().ok_or(ModelError::SecretUnavailable)?)?;
        ensure_private_dir(&directory)?;
        match fs::symlink_metadata(&destination) {
            Ok(_) => private_file(&destination)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(ModelError::SecretUnavailable),
        }
        let temporary = directory.join(format!(".{}.{}.tmp", reference, Uuid::new_v4()));
        let result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&temporary)
                .map_err(|_| ModelError::SecretUnavailable)?;
            file.write_all(secret.as_bytes())
                .and_then(|()| file.sync_all())
                .map_err(|_| ModelError::SecretUnavailable)?;
            fs::rename(&temporary, &destination).map_err(|_| ModelError::SecretUnavailable)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }

    fn get(&self, workspace: Uuid, reference: Uuid) -> Result<String, ModelError> {
        let (directory, file) = self.paths(workspace, reference)?;
        private_dir(directory.parent().ok_or(ModelError::SecretUnavailable)?)?;
        private_dir(&directory)?;
        private_file(&file)?;
        fs::read_to_string(file).map_err(|_| ModelError::SecretUnavailable)
    }

    fn delete(&self, workspace: Uuid, reference: Uuid) -> Result<(), ModelError> {
        let (directory, file) = self.paths(workspace, reference)?;
        private_dir(directory.parent().ok_or(ModelError::SecretUnavailable)?)?;
        private_dir(&directory)?;
        private_file(&file)?;
        fs::remove_file(file).map_err(|_| ModelError::SecretUnavailable)
    }
}

fn ensure_private_dir(path: &Path) -> Result<(), ModelError> {
    match DirBuilder::new().mode(0o700).create(path) {
        Ok(()) => private_dir(path),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => private_dir(path),
        Err(_) => Err(ModelError::SecretUnavailable),
    }
}

fn private_dir(path: &Path) -> Result<(), ModelError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| ModelError::SecretUnavailable)?;
    if !metadata.is_dir() || metadata.permissions().mode() & 0o077 != 0 {
        return Err(ModelError::SecretUnavailable);
    }
    Ok(())
}

fn private_file(path: &Path) -> Result<(), ModelError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| ModelError::SecretUnavailable)?;
    if !metadata.is_file()
        || metadata.permissions().mode() & 0o077 != 0
        || metadata.len() > MAX_SECRET_BYTES
    {
        return Err(ModelError::SecretUnavailable);
    }
    Ok(())
}

/// The isolated synthetic preview cannot access any real provider credential.
pub struct SyntheticModelSecrets;
impl ModelSecretStore for SyntheticModelSecrets {
    fn put(&self, _: Uuid, _: Uuid, secret: &str) -> Result<(), ModelError> {
        if secret == "SYNTHETIC-NOT-A-CREDENTIAL" {
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
    if env::var("LINGGAN_MODEL_SYNTHETIC_PREVIEW").as_deref() == Ok("SYNTHETIC-NOT-EVIDENCE") {
        Arc::new(SyntheticModelSecrets)
    } else {
        Arc::new(LocalFileModelSecrets::from_runtime())
    }
}
