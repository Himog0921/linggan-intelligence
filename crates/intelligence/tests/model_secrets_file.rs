use linggan_intelligence::model_secrets::{LocalFileModelSecrets, ModelSecretStore};
use std::{fs, os::unix::fs::PermissionsExt};
use uuid::Uuid;

#[test]
fn local_credentials_persist_and_reject_unsafe_files() {
    let root = std::env::temp_dir().join(format!("linggan-model-secret-proof-{}", Uuid::new_v4()));
    let workspace = Uuid::new_v4();
    let reference = Uuid::new_v4();
    let store = LocalFileModelSecrets::new(root.clone());
    store.put(workspace, reference, "SYNTHETIC-A").unwrap();

    let directory = root.join(workspace.to_string());
    let path = directory.join(reference.to_string());
    assert_eq!(
        fs::metadata(&root).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(&directory).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        LocalFileModelSecrets::new(root.clone())
            .get(workspace, reference)
            .unwrap(),
        "SYNTHETIC-A"
    );

    store.put(workspace, reference, "SYNTHETIC-B").unwrap();
    assert_eq!(store.get(workspace, reference).unwrap(), "SYNTHETIC-B");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(store.get(workspace, reference).is_err());
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    store.delete(workspace, reference).unwrap();
    assert!(store.get(workspace, reference).is_err());

    std::os::unix::fs::symlink("/dev/null", &path).unwrap();
    assert!(store.get(workspace, reference).is_err());
    assert!(store.put(workspace, reference, "SYNTHETIC-C").is_err());
    fs::remove_file(&path).unwrap();

    store.put(workspace, reference, "").unwrap();
    assert_eq!(store.get(workspace, reference).unwrap(), "");
    store.delete(workspace, reference).unwrap();
    fs::remove_dir_all(root).unwrap();

    assert!(
        LocalFileModelSecrets::new("relative/model-secrets".into())
            .put(workspace, reference, "SYNTHETIC-D")
            .is_err()
    );
}
