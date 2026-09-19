mod support;

use logcove::{client::Api, credentials::CredentialStore, error::Result};
use serde_json::json;
use std::process::Command;
use support::{Directory, MemoryStore, Server, Step};

const PAT: &str = "lc_pat_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const SESSION: &str = "saved-session.signature";

fn user() -> serde_json::Value {
    json!({"id":"owner","email":"owner@example.test","email_verified":true,"name":"Owner",
        "image":null,"created_at":"2026-09-19T00:00:00Z","updated_at":"2026-09-19T00:00:00Z"})
}

struct UnavailableStore;
impl CredentialStore for UnavailableStore {
    fn read(&self) -> Result<Option<String>> {
        panic!("must not read the credential store")
    }
    fn save(&self, _: &str) -> Result<()> {
        panic!("must not save in the credential store")
    }
    fn delete(&self) -> Result<()> {
        panic!("must not delete from the credential store")
    }
}

#[test]
fn token_operates_without_a_credential_store_and_ignores_session_renewal() {
    let server = Server::start(vec![
        Step::json("GET", "/api/v1/me", Some(PAT), 200, json!({"data":user()}))
            .header("set-auth-token", SESSION),
        Step::json(
            "GET",
            "/data/v1/projects?limit=100",
            Some(PAT),
            200,
            json!({"data":[],"pagination":{"next_cursor":null}}),
        ),
    ]);
    let mut api =
        Api::with_environment_token(server.origin.clone(), UnavailableStore, Some(PAT.into()))
            .unwrap();
    assert_eq!(api.whoami().unwrap().id, "owner");
    assert!(api.projects().unwrap().is_empty());
    server.finish();
}

#[test]
fn rejected_token_never_falls_back_or_deletes_saved_login() {
    let server = Server::start(vec![
        Step::json(
            "GET",
            "/api/v1/me",
            Some(PAT),
            401,
            json!({"error":{"code":"UNAUTHENTICATED"}}),
        )
        .header("x-request-id", "request-token"),
        Step::json("GET", "/api/v1/me", Some(PAT), 401, json!({})),
    ]);
    let store = MemoryStore::with_token(SESSION);
    let mut api =
        Api::with_environment_token(server.origin.clone(), store.clone(), Some(PAT.into()))
            .unwrap();
    let error = api.whoami().err().unwrap();
    assert_eq!(error.code, "UNAUTHENTICATED");
    assert_eq!(error.request_id.as_deref(), Some("request-token"));
    assert!(error.message.contains("LOGCOVE_TOKEN"));
    assert!(!error.message.contains(PAT));
    assert!(api.whoami().is_err());
    assert_eq!(store.read().unwrap().as_deref(), Some(SESSION));
    assert_eq!(store.0.borrow().saves, 0);
    server.finish();
}

#[test]
fn token_mode_disallows_session_login_and_logout_without_any_requests() {
    let mut api = Api::with_environment_token(
        "http://127.0.0.1:1".parse().unwrap(),
        UnavailableStore,
        Some(PAT.into()),
    )
    .unwrap();
    assert_eq!(api.begin_login().err().unwrap().code, "ENV_TOKEN_ACTIVE");
    assert_eq!(api.logout().unwrap_err().code, "ENV_TOKEN_ACTIVE");
}

#[test]
fn empty_malformed_write_keys_and_sessions_never_fall_back() {
    for token in [
        "",
        "lc_pat_bad",
        "lc_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        SESSION,
        " lc_pat_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    ] {
        let result = Api::with_environment_token(
            "http://127.0.0.1:1".parse().unwrap(),
            UnavailableStore,
            Some(token.into()),
        );
        assert_eq!(result.err().unwrap().code, "INVALID_TOKEN");
    }
}

#[test]
fn binary_uses_injected_token_and_never_writes_it_to_config_or_output() {
    let server = Server::start(vec![Step::json(
        "GET",
        "/api/v1/me",
        Some(PAT),
        200,
        json!({"data":user()}),
    )]);
    let directory = Directory::default();
    let output = Command::new(env!("CARGO_BIN_EXE_logcove"))
        .env("LOGCOVE_TOKEN", PAT)
        .env("DBUS_SESSION_BUS_ADDRESS", "unix:path=/nonexistent")
        .args(["--api-url", server.origin.as_str(), "--config-dir"])
        .arg(&directory.0)
        .arg("whoami")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap()["data"]["id"],
        "owner"
    );
    assert!(!String::from_utf8_lossy(&output.stdout).contains(PAT));
    assert!(!String::from_utf8_lossy(&output.stderr).contains(PAT));
    assert_eq!(std::fs::read_dir(&directory.0).unwrap().count(), 0);
    server.finish();
}

#[test]
fn binary_reports_environment_login_logout_and_invalid_token_errors_safely() {
    let directory = Directory::default();
    for (token, command, code) in [
        (PAT, "login", "ENV_TOKEN_ACTIVE"),
        (PAT, "logout", "ENV_TOKEN_ACTIVE"),
        ("", "whoami", "INVALID_TOKEN"),
        ("wrong-secret", "whoami", "INVALID_TOKEN"),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_logcove"))
            .env("LOGCOVE_TOKEN", token)
            .args(["--api-url", "http://127.0.0.1:1", "--config-dir"])
            .arg(&directory.0)
            .arg(command)
            .output()
            .unwrap();
        assert!(!output.status.success());
        let error: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(error["error"]["code"], code);
        if !token.is_empty() {
            assert!(!String::from_utf8_lossy(&output.stderr).contains(token));
        }
    }
}
