mod support;

use logcove::{
    client::Api,
    management::{
        CreateKey, KeyCommand, KeyStatus, ProjectCommand, ProjectStatus, SetKeyProjects,
        UpdateProject,
    },
    models::IngestionProtocol,
};
use serde_json::{json, Value};
use std::{fs, path::PathBuf, process::Command};
use support::{Directory, MemoryStore, Server, Step};

const PROJECT: &str = "prj_00000000-0000-4000-8000-000000000001";
const SECOND: &str = "prj_00000000-0000-4000-8000-000000000002";
const KEY: &str = "key_00000000-0000-4000-8000-000000000001";
const TOKEN: &str = "fake-session";
fn secret() -> String {
    format!("lc_{}", "a".repeat(64))
}
fn project() -> Value {
    json!({"id":PROJECT,"name":"Logs","description":null,"status":"active","ingestion_protocol":"http_json","data_prefix":format!("logs/project={PROJECT}/"),"write_key_id":null,"ingestion":{"desired_revision":1},"created_at":"2026-09-10T00:00:00Z","updated_at":"2026-09-10T00:00:00Z"})
}
fn key() -> Value {
    json!({"id":KEY,"name":"Collector","key_prefix":"lc_aaaaaaaa","masked_key":"lc_aaaaaaaa***","project_ids":[PROJECT],"revoked_at":null,"created_at":"2026-09-10T00:00:00Z","updated_at":"2026-09-10T00:00:00Z"})
}
fn create(output: PathBuf) -> CreateKey {
    CreateKey {
        name: "Collector".into(),
        project_ids: vec![PROJECT.into()],
        output,
    }
}
fn update() -> UpdateProject {
    UpdateProject {
        id: PROJECT.into(),
        name: None,
        description: None,
        clear_description: false,
        write_key_id: None,
        clear_write_key: false,
    }
}
fn step(method: &'static str, path: &str, data: Value) -> Step {
    Step::json(method, path, Some(TOKEN), 200, json!({"data":data}))
}
fn page(data: Value, cursor: Option<&str>) -> Value {
    json!({"data":data,"pagination":{"next_cursor":cursor}})
}

#[test]
fn project_lifecycle_preserves_binding_and_ingestion_metadata() {
    let mut bound = project();
    bound["write_key_id"] = json!(KEY);
    bound["ingestion"]["desired_revision"] = json!(2);
    let mut archived = bound.clone();
    archived["status"] = json!("archived");
    let route = format!("/api/v1/projects/{PROJECT}");
    let server = Server::start(vec![
        Step::json(
            "POST",
            "/api/v1/projects",
            Some(TOKEN),
            201,
            json!({"data":project()}),
        )
        .body(json!({"name":"Logs","description":"Backend","ingestion_protocol":"http_json"})),
        step("GET", &route, project()),
        step("PATCH", &route, bound.clone())
            .body(json!({"name":"Renamed","description":null,"write_key_id":KEY})),
        step("PATCH", &route, archived.clone()).body(json!({"status":"archived"})),
        step("PATCH", &route, bound.clone()).body(json!({"status":"active"})),
        step("PATCH", &route, project()).body(json!({"write_key_id":null})),
    ]);
    let mut api = Api::new(server.origin.clone(), MemoryStore::with_token(TOKEN)).unwrap();
    assert_eq!(
        api.project_command(ProjectCommand::Create {
            ingestion_protocol: IngestionProtocol::HttpJson,
            name: " Logs ".into(),
            description: Some("Backend".into())
        })
        .unwrap()["data"],
        project()
    );
    assert_eq!(
        api.project_command(ProjectCommand::Get { id: PROJECT.into() })
            .unwrap()["data"],
        project()
    );
    let mut args = update();
    args.name = Some("Renamed".into());
    args.clear_description = true;
    args.write_key_id = Some(KEY.into());
    assert_eq!(
        api.project_command(ProjectCommand::Update(args)).unwrap()["data"],
        bound
    );
    assert_eq!(
        api.project_command(ProjectCommand::Archive { id: PROJECT.into() })
            .unwrap()["data"],
        archived
    );
    assert_eq!(
        api.project_command(ProjectCommand::Restore { id: PROJECT.into() })
            .unwrap()["data"],
        bound
    );
    let mut args = update();
    args.clear_write_key = true;
    assert!(
        api.project_command(ProjectCommand::Update(args)).unwrap()["data"]["write_key_id"]
            .is_null()
    );
    server.finish();
}

#[test]
fn otlp_protocol_is_sent_on_creation_and_preserved_in_output() {
    let mut otlp = project();
    otlp["ingestion_protocol"] = json!("otlp_http");
    let server = Server::start(vec![
        Step::json(
            "POST",
            "/api/v1/projects",
            Some(TOKEN),
            201,
            json!({"data":otlp}),
        )
        .body(json!({"name":"OTel", "ingestion_protocol":"otlp_http"})),
        step("GET", &format!("/api/v1/projects/{PROJECT}"), otlp.clone()),
    ]);
    let mut api = Api::new(server.origin.clone(), MemoryStore::with_token(TOKEN)).unwrap();
    assert_eq!(
        api.project_command(ProjectCommand::Create {
            name: "OTel".into(),
            description: None,
            ingestion_protocol: IngestionProtocol::OtlpHttp,
        })
        .unwrap()["data"],
        otlp
    );
    assert_eq!(
        api.project_command(ProjectCommand::Get { id: PROJECT.into() })
            .unwrap()["data"],
        otlp
    );
}

#[test]
fn project_listing_preserves_status_across_empty_pages_and_supports_all() {
    let server = Server::start(vec![
        Step::json(
            "GET",
            "/api/v1/projects?status=archived&limit=100",
            Some(TOKEN),
            200,
            page(json!([]), Some("next")),
        ),
        Step::json(
            "GET",
            "/api/v1/projects?status=archived&limit=100&cursor=next",
            Some(TOKEN),
            200,
            page(json!([project()]), None),
        ),
        Step::json(
            "GET",
            "/api/v1/projects?limit=100",
            Some(TOKEN),
            200,
            page(json!([]), None),
        ),
        Step::json(
            "GET",
            "/api/v1/projects?status=active&limit=100",
            Some(TOKEN),
            200,
            page(json!([project()]), None),
        ),
    ]);
    let mut api = Api::new(server.origin.clone(), MemoryStore::with_token(TOKEN)).unwrap();
    assert_eq!(
        api.project_command(ProjectCommand::List {
            status: ProjectStatus::Archived
        })
        .unwrap()["data"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        api.project_command(ProjectCommand::List {
            status: ProjectStatus::All
        })
        .unwrap()["data"],
        json!([])
    );
    assert_eq!(
        api.project_command(ProjectCommand::List {
            status: ProjectStatus::Active
        })
        .unwrap()["data"][0]["ingestion"]["desired_revision"],
        1
    );
    server.finish();
}

#[test]
fn created_secret_is_only_in_the_private_file_not_returned_json() {
    let directory = Directory::default();
    let output = directory.0.join("collector.key");
    let mut response = key();
    response["key"] = json!(secret());
    response["key_hash"] = json!("not-public");
    let server = Server::start(vec![Step::json(
        "POST",
        "/api/v1/keys",
        Some(TOKEN),
        201,
        json!({"data":response}),
    )
    .body(json!({"name":"Collector","project_ids":[PROJECT]}))]);
    let mut api = Api::new(server.origin.clone(), MemoryStore::with_token(TOKEN)).unwrap();
    let result = api.create_key(create(output.clone())).unwrap();
    assert_eq!(fs::read_to_string(&output).unwrap(), secret() + "\n");
    assert_eq!(
        PathBuf::from(result["data"]["key_file"].as_str().unwrap()),
        output.canonicalize().unwrap()
    );
    assert_eq!(result["data"]["id"], KEY);
    assert!(result["data"].get("key").is_none());
    assert!(result["data"].get("key_hash").is_none());
    assert!(!result.to_string().contains(&secret()));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&output).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    server.finish();
}

#[test]
fn key_lifecycle_uses_complete_binding_sets_and_masked_output() {
    let route = format!("/api/v1/keys/{KEY}");
    let mut revoked = key();
    revoked["revoked_at"] = json!("2026-09-10T01:00:00Z");
    revoked["project_ids"] = json!([]);
    let mut response = key();
    response["key"] = json!(secret());
    let server = Server::start(vec![
        step("GET", &route, response),
        step("PATCH", &route, key()).body(json!({"name":"New name"})),
        step("PUT", &format!("{route}/projects"), key())
            .body(json!({"project_ids":[PROJECT,SECOND]})),
        step("PUT", &format!("{route}/projects"), key()).body(json!({"project_ids":[]})),
        step("DELETE", &route, revoked.clone()),
        step("DELETE", &route, revoked.clone()),
    ]);
    let mut api = Api::new(server.origin.clone(), MemoryStore::with_token(TOKEN)).unwrap();
    assert!(
        api.key_command(KeyCommand::Get { id: KEY.into() }).unwrap()["data"]
            .get("key")
            .is_none()
    );
    api.key_command(KeyCommand::Update {
        id: KEY.into(),
        name: " New name ".into(),
    })
    .unwrap();
    api.key_command(KeyCommand::SetProjects(SetKeyProjects {
        id: KEY.into(),
        project_ids: vec![PROJECT.into(), SECOND.into()],
        clear_projects: false,
    }))
    .unwrap();
    api.key_command(KeyCommand::SetProjects(SetKeyProjects {
        id: KEY.into(),
        project_ids: vec![],
        clear_projects: true,
    }))
    .unwrap();
    for _ in 0..2 {
        assert_eq!(
            api.key_command(KeyCommand::Revoke { id: KEY.into() })
                .unwrap()["data"],
            revoked
        );
    }
    server.finish();
}

#[test]
fn key_filters_paginate_and_never_return_secret_fields() {
    let route = format!("/api/v1/keys?status=active&project_id={PROJECT}&limit=100");
    let mut item = key();
    item["key"] = json!(secret());
    let server = Server::start(vec![
        Step::json(
            "GET",
            &route,
            Some(TOKEN),
            200,
            page(json!([]), Some("next")),
        ),
        Step::json(
            "GET",
            &format!("{route}&cursor=next"),
            Some(TOKEN),
            200,
            page(json!([item]), None),
        ),
        Step::json(
            "GET",
            "/api/v1/keys?status=revoked&limit=100",
            Some(TOKEN),
            200,
            page(json!([]), None),
        ),
        Step::json(
            "GET",
            "/api/v1/keys?limit=100",
            Some(TOKEN),
            200,
            page(json!([]), None),
        ),
    ]);
    let mut api = Api::new(server.origin.clone(), MemoryStore::with_token(TOKEN)).unwrap();
    let result = api
        .key_command(KeyCommand::List {
            status: KeyStatus::Active,
            project_id: Some(PROJECT.into()),
        })
        .unwrap();
    assert_eq!(result["data"][0]["id"], KEY);
    assert!(!result.to_string().contains(&secret()));
    for status in [KeyStatus::Revoked, KeyStatus::All] {
        assert_eq!(
            api.key_command(KeyCommand::List {
                status,
                project_id: None
            })
            .unwrap()["data"],
            json!([])
        );
    }
    server.finish();
}

#[test]
fn repeated_cursor_and_later_page_failure_do_not_return_partial_lists() {
    for second in [
        page(json!([]), Some("same")),
        json!({"error":{"message":secret()}}),
    ] {
        let status = if second.get("error").is_some() {
            403
        } else {
            200
        };
        let server = Server::start(vec![
            Step::json(
                "GET",
                "/api/v1/keys?limit=100",
                Some(TOKEN),
                200,
                page(json!([key()]), Some("same")),
            ),
            Step::json(
                "GET",
                "/api/v1/keys?limit=100&cursor=same",
                Some(TOKEN),
                status,
                second,
            ),
        ]);
        let mut api = Api::new(server.origin.clone(), MemoryStore::with_token(TOKEN)).unwrap();
        let error = api
            .key_command(KeyCommand::List {
                status: KeyStatus::All,
                project_id: None,
            })
            .unwrap_err();
        assert_eq!(
            error.code,
            if status == 403 {
                "ACCESS_DENIED"
            } else {
                "INVALID_PAGINATION"
            }
        );
        assert!(!error.to_string().contains(&secret()));
        server.finish();
    }
}

#[test]
fn api_failures_remove_reserved_files_do_not_leak_secrets_or_retry_creates() {
    for (status, code) in [
        (400, "INVALID_INPUT"),
        (401, "UNAUTHENTICATED"),
        (403, "ACCESS_DENIED"),
        (404, "NOT_FOUND"),
        (409, "CONFLICT"),
        (409, "WRITE_KEY_CONFLICT"),
        (409, "INVALID_KEY_BINDING"),
        (409, "KEY_REVOKED"),
        (500, "SERVER_ERROR"),
    ] {
        let directory = Directory::default();
        let output = directory.0.join("writer.key");
        let server = Server::start(vec![Step::json(
            "POST",
            "/api/v1/keys",
            Some(TOKEN),
            status,
            json!({"error":{"code":code,"message":secret()}}),
        )
        .header("X-Request-ID", "test-request")]);
        let store = MemoryStore::with_token(TOKEN);
        let mut api = Api::new(server.origin.clone(), store.clone()).unwrap();
        let error = api.create_key(create(output.clone())).unwrap_err();
        assert_eq!(error.code, code);
        assert_eq!(error.request_id.as_deref(), Some("test-request"));
        assert!(!error.to_string().contains(&secret()));
        assert!(!output.exists());
        assert_eq!(store.0.borrow().token.is_none(), status == 401);
        if status == 500 {
            assert!(error.message.contains("before retrying"));
        } else if status == 409 {
            assert!(!error.message.contains("may have reached the server"));
        }
        server.finish();
    }
}

#[test]
fn project_quota_errors_explain_create_and_restore_failures_without_retrying() {
    for (method, route, command) in [
        (
            "POST",
            "/api/v1/projects".to_owned(),
            ProjectCommand::Create {
                name: "Logs".into(),
                description: None,
                ingestion_protocol: IngestionProtocol::HttpJson,
            },
        ),
        (
            "PATCH",
            format!("/api/v1/projects/{PROJECT}"),
            ProjectCommand::Restore { id: PROJECT.into() },
        ),
    ] {
        let server = Server::start(vec![Step::json(
            method,
            &route,
            Some(TOKEN),
            409,
            json!({"error":{"code":"PROJECT_LIMIT_REACHED","message":secret()}}),
        )
        .header("X-Request-ID", "quota-request")]);
        let store = MemoryStore::with_token(TOKEN);
        let mut api = Api::new(server.origin.clone(), store.clone()).unwrap();
        let error = api.project_command(command).unwrap_err();
        assert_eq!(error.code, "PROJECT_LIMIT_REACHED");
        assert!(error.message.contains("Archive a Project or upgrade"));
        assert!(!error.message.contains(&secret()));
        assert_eq!(error.request_id.as_deref(), Some("quota-request"));
        assert!(store.0.borrow().token.is_some());
        server.finish();
    }
}

#[test]
fn invalid_once_only_response_does_not_publish_a_key_file() {
    for valid_metadata in [true, false] {
        let directory = Directory::default();
        let output = directory.0.join("key");
        let mut response = if valid_metadata { key() } else { json!({}) };
        response["key"] = json!("invalid-secret");
        let server = Server::start(vec![step("POST", "/api/v1/keys", response)]);
        let mut api = Api::new(server.origin.clone(), MemoryStore::with_token(TOKEN)).unwrap();
        let error = api.create_key(create(output.clone())).unwrap_err();
        assert_eq!(error.code, "INVALID_RESPONSE");
        assert!(!error.message.contains("invalid-secret"));
        assert!(!output.exists());
        if valid_metadata {
            assert!(error.message.contains(KEY));
            assert!(error.message.contains("revoke"));
        }
        server.finish();
    }
}

#[test]
fn invalid_local_inputs_never_send_a_request_or_overwrite_files() {
    let directory = Directory::default();
    let output = directory.write("exists.key", "existing-content");
    let mut api = Api::new(
        "http://127.0.0.1:1".parse().unwrap(),
        MemoryStore::with_token(TOKEN),
    )
    .unwrap();
    for path in [
        output.clone(),
        directory.0.clone(),
        directory.0.join("missing/key"),
    ] {
        assert_eq!(
            api.create_key(create(path)).unwrap_err().code,
            "KEY_FILE_ERROR"
        );
    }
    assert_eq!(fs::read_to_string(output).unwrap(), "existing-content");
    assert_eq!(
        api.project_command(ProjectCommand::Update(update()))
            .unwrap_err()
            .code,
        "INVALID_INPUT"
    );
    for id in [
        "invalid",
        "../keys",
        "key_00000000-0000-4000-8000-000000000001?x",
    ] {
        assert_eq!(
            api.key_command(KeyCommand::Revoke { id: id.into() })
                .unwrap_err()
                .code,
            "INVALID_INPUT"
        );
    }
    for ids in [
        vec![PROJECT.into(), PROJECT.into()],
        vec![PROJECT.into(); 101],
        vec!["wrong".into()],
    ] {
        let mut args = create(directory.0.join("unused"));
        args.project_ids = ids;
        assert_eq!(api.create_key(args).unwrap_err().code, "INVALID_INPUT");
    }
    assert_eq!(
        api.project_command(ProjectCommand::Create {
            ingestion_protocol: IngestionProtocol::HttpJson,
            name: " ".into(),
            description: None
        })
        .unwrap_err()
        .code,
        "INVALID_INPUT"
    );
    assert_eq!(
        api.project_command(ProjectCommand::Create {
            ingestion_protocol: IngestionProtocol::HttpJson,
            name: "x".into(),
            description: Some("x".repeat(2001))
        })
        .unwrap_err()
        .code,
        "INVALID_INPUT"
    );
    let mut args = update();
    args.write_key_id = Some(secret());
    assert_eq!(
        api.project_command(ProjectCommand::Update(args))
            .unwrap_err()
            .code,
        "INVALID_INPUT"
    );
}

#[cfg(unix)]
#[test]
fn output_symlink_is_not_followed_even_when_its_target_is_missing() {
    let directory = Directory::default();
    let target = directory.0.join("target");
    let output = directory.0.join("key");
    std::os::unix::fs::symlink(&target, &output).unwrap();
    let mut api = Api::new(
        "http://127.0.0.1:1".parse().unwrap(),
        MemoryStore::with_token(TOKEN),
    )
    .unwrap();
    assert_eq!(
        api.create_key(create(output)).unwrap_err().code,
        "KEY_FILE_ERROR"
    );
    assert!(!target.exists());
}

#[test]
fn cli_requires_an_output_file_and_explicit_binding_changes() {
    let directory = Directory::default();
    for args in [
        vec!["keys", "create", "--name", "Collector"],
        vec!["keys", "set-projects", KEY],
        vec![
            "keys",
            "set-projects",
            KEY,
            "--project-id",
            PROJECT,
            "--clear-projects",
        ],
        vec![
            "projects",
            "update",
            PROJECT,
            "--write-key-id",
            KEY,
            "--clear-write-key",
        ],
        vec![
            "projects",
            "update",
            PROJECT,
            "--description",
            "x",
            "--clear-description",
        ],
        vec!["projects", "list", "--status", "revoked"],
        vec![
            "projects",
            "create",
            "--name",
            "Logs",
            "--ingestion-protocol",
            "grpc",
        ],
        vec![
            "projects",
            "update",
            PROJECT,
            "--ingestion-protocol",
            "otlp_http",
        ],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_logcove"))
            .env_remove("LOGCOVE_API_URL")
            .env("LOGCOVE_CONFIG_DIR", &directory.0)
            .args(args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
    for args in [
        vec!["projects", "--help"],
        vec!["keys", "--help"],
        vec!["keys", "create", "--help"],
        vec!["keys", "set-projects", "--help"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_logcove"))
            .env_remove("LOGCOVE_API_URL")
            .env("LOGCOVE_CONFIG_DIR", &directory.0)
            .args(args)
            .output()
            .unwrap();
        assert!(output.status.success());
    }
    for args in [
        vec![
            "keys",
            "set-projects",
            KEY,
            "--project-id",
            PROJECT,
            "--project-id",
            SECOND,
        ],
        vec!["keys", "set-projects", KEY, "--clear-projects"],
        vec![
            "keys",
            "create",
            "--name",
            "Collector",
            "--output",
            "writer.key",
        ],
        vec!["keys", "delete", KEY],
        vec!["projects", "list"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_logcove"))
            .env_remove("LOGCOVE_API_URL")
            .env("LOGCOVE_CONFIG_DIR", &directory.0)
            .args(args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&output.stderr).contains("API_NOT_CONFIGURED"));
    }
}
