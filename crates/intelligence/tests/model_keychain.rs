use linggan_intelligence::model_secrets::{KeychainModelSecrets, ModelSecretStore};
use uuid::Uuid;
#[test]
#[ignore = "creates and deletes only a random synthetic macOS Keychain item"]
fn isolated_keychain_roundtrip_and_replacement() {
    let workspace = Uuid::new_v4();
    let reference = Uuid::new_v4();
    let store = KeychainModelSecrets;
    store
        .put(workspace, reference, "SYNTHETIC-KEYCHAIN-PROOF-A")
        .unwrap();
    let outcome = std::panic::catch_unwind(|| {
        assert_eq!(
            store.get(workspace, reference).unwrap(),
            "SYNTHETIC-KEYCHAIN-PROOF-A"
        );
        store
            .put(workspace, reference, "SYNTHETIC-KEYCHAIN-PROOF-B")
            .unwrap();
        assert_eq!(
            store.get(workspace, reference).unwrap(),
            "SYNTHETIC-KEYCHAIN-PROOF-B"
        );
    });
    store.delete(workspace, reference).unwrap();
    assert!(store.get(workspace, reference).is_err());
    outcome.unwrap();
}
