use logcove::credentials::{CredentialStore, OsCredentials};
use std::{
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
#[ignore = "uses isolated fake credentials in the real OS store; run explicitly on a configured desktop"]
fn native_credential_persistence_across_processes() {
    const ACCOUNT: &str = "LOGCOVE_NATIVE_TEST_ACCOUNT";
    const PHASE: &str = "LOGCOVE_NATIVE_TEST_PHASE";
    if let Ok(account) = std::env::var(ACCOUNT) {
        let store = OsCredentials::new(&account).unwrap();
        match std::env::var(PHASE).unwrap().as_str() {
            "restore" => {
                assert!(store.read().unwrap().as_deref() == Some("fake-initial.signature"));
                store.save("fake-updated.signature").unwrap();
            }
            "delete" => {
                assert!(store.read().unwrap().as_deref() == Some("fake-updated.signature"));
                store.delete_if_matches("fake-initial.signature").unwrap();
                assert!(store.read().unwrap().as_deref() == Some("fake-updated.signature"));
                store.delete_if_matches("fake-updated.signature").unwrap();
            }
            _ => panic!("Unknown test phase"),
        }
        return;
    }
    let id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let account = format!("https://cli-credential-test-{id}.invalid");
    let store = OsCredentials::new(&account).unwrap();
    let other = OsCredentials::new(&format!("{account}:8443")).unwrap();
    assert!(store.read().unwrap().is_none());
    store.save("fake-initial.signature").unwrap();
    let mut successful = true;
    for phase in ["restore", "delete"] {
        let result = Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "native_credential_persistence_across_processes",
            ])
            .env(ACCOUNT, &account)
            .env(PHASE, phase)
            .status()
            .unwrap();
        if !result.success() {
            successful = false;
            break;
        }
    }
    let removed = store.read().unwrap().is_none();
    let isolated = other.read().unwrap().is_none();
    store.delete().unwrap();
    assert!(successful && removed && isolated);
}
