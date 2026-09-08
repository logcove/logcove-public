use logcove::config;
use serde_json::Value;
use std::{
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

static SEQUENCE: AtomicUsize = AtomicUsize::new(0);

struct Directory(PathBuf);

impl Directory {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!(
            "logcove-cli-test-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        )))
    }
    fn command(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_logcove"))
            .env_remove("LOGCOVE_API_URL")
            .env_remove("LOGCOVE_CONFIG_DIR")
            .arg("--config-dir")
            .arg(&self.0)
            .args(args)
            .output()
            .unwrap()
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn origin_rules_and_environment_isolation() {
    for (input, expected) in [
        ("https://API.example.test:443/", "https://api.example.test"),
        ("http://localhost:8787", "http://localhost:8787"),
        ("http://127.0.0.2:8787", "http://127.0.0.2:8787"),
        ("http://[::1]:8787/", "http://[::1]:8787"),
    ] {
        assert_eq!(
            config::origin(input)
                .unwrap()
                .origin()
                .ascii_serialization(),
            expected
        );
    }
    for input in [
        "http://api.example.test",
        "ftp://example.test",
        "https://user:secret@example.test",
        "https://example.test/api",
        "https://example.test/?token=secret",
        "https://example.test/#fragment",
        "http://localhost.evil.test",
        "file:///tmp/local",
        "not a url",
    ] {
        let error = config::origin(input).unwrap_err();
        assert_eq!(error.code, "INVALID_API_URL");
        assert!(!error.message.contains("secret"));
    }
    assert_ne!(
        config::origin("http://localhost:8787").unwrap().origin(),
        config::origin("http://localhost:8788").unwrap().origin()
    );
}

#[test]
fn cli_config_persists_normalized_origin_and_needs_no_keyring() {
    let directory = Directory::new();
    let result = directory.command(&["config", "show"]);
    assert!(result.status.success());
    assert!(serde_json::from_slice::<Value>(&result.stdout).unwrap()["data"]["api_url"].is_null());
    let result = directory.command(&["config", "set-api-url", "https://API.example.test:443/"]);
    assert!(result.status.success());
    assert!(result.stderr.is_empty());
    let file = std::fs::read_to_string(directory.0.join("config.json")).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&file).unwrap(),
        serde_json::json!({"api_url":"https://api.example.test"})
    );
    let result = directory.command(&["config", "show"]);
    assert_eq!(
        serde_json::from_slice::<Value>(&result.stdout).unwrap()["data"]["api_url"],
        "https://api.example.test"
    );
}

#[test]
fn api_flag_overrides_environment_which_overrides_saved_configuration() {
    let directory = Directory::new();
    assert!(directory
        .command(&["config", "set-api-url", "https://saved.example.test"])
        .status
        .success());
    let run = |flags: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_logcove"))
            .env("LOGCOVE_API_URL", "https://environment.example.test")
            .env("LOGCOVE_CONFIG_DIR", &directory.0)
            .args(flags)
            .args(["config", "show"])
            .output()
            .unwrap()
    };
    let env = run(&[]);
    assert_eq!(
        serde_json::from_slice::<Value>(&env.stdout).unwrap()["data"]["api_url"],
        "https://environment.example.test"
    );
    let flag = run(&["--api-url", "https://flag.example.test"]);
    assert_eq!(
        serde_json::from_slice::<Value>(&flag.stdout).unwrap()["data"]["api_url"],
        "https://flag.example.test"
    );
    assert_eq!(
        config::load(&directory.0).unwrap().api_url.as_deref(),
        Some("https://saved.example.test")
    );
}

#[test]
fn missing_configuration_returns_structured_stderr_without_touching_keyring() {
    let directory = Directory::new();
    let result = directory.command(&["whoami"]);
    assert_eq!(result.status.code(), Some(1));
    assert!(result.stdout.is_empty());
    let error: Value = serde_json::from_slice(&result.stderr).unwrap();
    assert_eq!(error["error"]["code"], "API_NOT_CONFIGURED");
}

#[test]
fn malformed_config_reports_error_and_can_be_replaced_explicitly() {
    let directory = Directory::new();
    std::fs::create_dir_all(&directory.0).unwrap();
    std::fs::write(directory.0.join("config.json"), "not-json-with-secret").unwrap();
    let result = directory.command(&["config", "show"]);
    assert!(!result.status.success());
    assert!(!String::from_utf8_lossy(&result.stderr).contains("not-json-with-secret"));
    assert!(directory
        .command(&["config", "set-api-url", "http://localhost:8787"])
        .status
        .success());
}

#[test]
fn help_and_usage_are_available_without_environment_or_credentials() {
    let directory = Directory::new();
    let result = directory.command(&["--help"]);
    assert!(result.status.success());
    let text = String::from_utf8(result.stdout).unwrap();
    for name in ["login", "logout", "whoami", "projects", "config"] {
        assert!(text.contains(name));
    }
    assert_eq!(
        directory.command(&["projects", "unknown"]).status.code(),
        Some(2)
    );
    assert_eq!(
        directory.command(&["data", "describe"]).status.code(),
        Some(2)
    );
}
