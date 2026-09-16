mod support;
use logcove::{
    charts::{create_body, update_body, CreateArgs, UpdateArgs},
    client::Api,
};
use serde_json::{json, Value};
use support::{Directory, MemoryStore, Server, Step};

const PROJECT: &str = "prj_00000000-0000-4000-8000-000000000001";
const CHART: &str = "chart_00000000-0000-4000-8000-000000000001";
const TOKEN: &str = "session-token";
fn spec() -> Value {
    json!({"data":{"name":"result"},"mark":"bar"})
}
fn create(dir: &Directory) -> CreateArgs {
    CreateArgs {
        name: "Requests".into(),
        description: None,
        project_ids: vec![PROJECT.into()],
        sql_file: dir.write(
            "query.sql",
            &format!("SELECT count(*) AS requests FROM \"{PROJECT}\""),
        ),
        spec_file: dir.write("spec.json", &spec().to_string()),
    }
}
fn update() -> UpdateArgs {
    UpdateArgs {
        id: CHART.into(),
        revision: 1,
        name: None,
        description: None,
        clear_description: false,
        project_ids: vec![],
        clear_projects: false,
        sql_file: None,
        spec_file: None,
    }
}

#[test]
fn creates_and_reads_definition_without_result_upload() {
    let dir = Directory::default();
    let args = create(&dir);
    let body = create_body(&args).unwrap();
    assert_eq!(body["project_ids"], json!([PROJECT]));
    assert!(body.get("result").is_none());
    let definition = json!({"id":CHART,"revision":1,"sql":body["sql"],"project_ids":[PROJECT],"vega_lite_spec":spec()});
    let server = Server::start(vec![
        Step::json(
            "POST",
            "/api/v1/charts",
            Some(TOKEN),
            201,
            json!({"data":definition}),
        )
        .body(body),
        Step::json(
            "GET",
            &format!("/api/v1/charts/{CHART}"),
            Some(TOKEN),
            200,
            json!({"data":definition}),
        ),
    ]);
    let mut api = Api::new(server.origin.clone(), MemoryStore::with_token(TOKEN)).unwrap();
    assert_eq!(api.create_chart(&args).unwrap(), definition);
    assert_eq!(api.chart(CHART).unwrap(), definition);
    server.finish();
}
#[test]
fn updates_preserve_omitted_fields_and_require_explicit_sql_dependencies() {
    let dir = Directory::default();
    let mut args = update();
    args.name = Some("Renamed".into());
    assert_eq!(
        update_body(&args).unwrap(),
        json!({"revision":1,"name":"Renamed"})
    );
    args.sql_file = Some(dir.write("query.sql", "SELECT 1"));
    assert_eq!(update_body(&args).unwrap_err().code, "INVALID_INPUT");
    args.project_ids = vec![PROJECT.into()];
    assert_eq!(update_body(&args).unwrap()["project_ids"], json!([PROJECT]));
    args.project_ids.clear();
    args.clear_projects = true;
    args.clear_description = true;
    assert_eq!(
        update_body(&args).unwrap(),
        json!({"revision":1,"name":"Renamed","sql":"SELECT 1","project_ids":[],"description":null})
    );
    let server = Server::start(vec![Step::json(
        "PATCH",
        &format!("/api/v1/charts/{CHART}"),
        Some(TOKEN),
        200,
        json!({"data":{"id":CHART,"revision":2}}),
    )
    .body(update_body(&args).unwrap())]);
    let mut api = Api::new(server.origin.clone(), MemoryStore::with_token(TOKEN)).unwrap();
    assert_eq!(api.update_chart(&args).unwrap()["revision"], 2);
    server.finish();
}
#[test]
fn revision_conflict_is_not_automatically_retried() {
    let mut args = update();
    args.name = Some("Conflict".into());
    let server = Server::start(vec![Step::json(
        "PATCH",
        &format!("/api/v1/charts/{CHART}"),
        Some(TOKEN),
        409,
        json!({"error":{"code":"CHART_CONFLICT"}}),
    )
    .header("X-Request-ID", "chart-conflict")]);
    let mut api = Api::new(server.origin.clone(), MemoryStore::with_token(TOKEN)).unwrap();
    let error = api.update_chart(&args).unwrap_err();
    assert_eq!(error.code, "CHART_CONFLICT");
    assert_eq!(error.request_id.as_deref(), Some("chart-conflict"));
    server.finish();
}
#[test]
fn deletion_and_source_filter_pagination_keep_their_contracts() {
    let mut deleted = Step::json(
        "DELETE",
        &format!("/api/v1/charts/{CHART}"),
        Some(TOKEN),
        204,
        Value::Null,
    );
    deleted.response.clear();
    let server = Server::start(vec![
        Step::json(
            "GET",
            &format!("/api/v1/charts?limit=100&project_id={PROJECT}"),
            Some(TOKEN),
            200,
            json!({"data":[],"pagination":{"next_cursor":"next"}}),
        ),
        Step::json(
            "GET",
            &format!("/api/v1/charts?limit=100&project_id={PROJECT}&cursor=next"),
            Some(TOKEN),
            200,
            json!({"data":[{"id":CHART}],"pagination":{"next_cursor":null}}),
        ),
        deleted,
        Step::json(
            "GET",
            &format!("/api/v1/charts/{CHART}"),
            Some(TOKEN),
            404,
            json!({}),
        ),
    ]);
    let mut api = Api::new(server.origin.clone(), MemoryStore::with_token(TOKEN)).unwrap();
    assert_eq!(api.charts(Some(PROJECT)).unwrap().len(), 1);
    api.delete_chart(CHART).unwrap();
    assert_eq!(api.chart(CHART).unwrap_err().code, "NOT_FOUND");
    server.finish();
}
#[test]
fn validates_files_bom_limits_ids_and_removed_commands() {
    let dir = Directory::default();
    let mut args = create(&dir);
    args.sql_file = dir.write("query.sql", "\u{feff}SELECT 1;");
    args.spec_file = dir.write("spec.json", &format!("\u{feff}{}", spec()));
    assert_eq!(create_body(&args).unwrap()["sql"], "SELECT 1;");
    args.sql_file = dir.write("empty.sql", " ");
    assert!(create_body(&args).is_err());
    args.sql_file = dir.write("large.sql", &"x".repeat(65537));
    assert_eq!(create_body(&args).unwrap_err().code, "PAYLOAD_TOO_LARGE");
    args.sql_file = dir.write("query.sql", "SELECT 1");
    args.spec_file = dir.write("spec.json", "{\"data\":{\"url\":\"https://example.test\"}}");
    assert!(create_body(&args).is_err());
    assert!(update_body(&update()).is_err());
    let mut api = Api::new(
        reqwest::Url::parse("http://127.0.0.1:1").unwrap(),
        MemoryStore::with_token(TOKEN),
    )
    .unwrap();
    assert_eq!(api.chart("../keys").unwrap_err().code, "INVALID_INPUT");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_logcove"))
        .args(["charts", "result", "put", CHART, "--file", "result.json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_logcove"))
        .args([
            "charts",
            "create",
            "--name",
            "Old",
            "--sql-file",
            "q.sql",
            "--spec-file",
            "s.json",
            "--result-file",
            "r.json",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn chart_text_limits_match_api_utf16_units_for_create_and_update() {
    let dir = Directory::default();
    let mut args = create(&dir);
    args.name = "\u{1f600}".repeat(50);
    args.description = Some("\u{1f600}".repeat(1000));
    assert!(create_body(&args).is_ok());
    let mut edit = update();
    edit.name = Some(args.name.clone());
    edit.description = args.description.clone();
    assert!(update_body(&edit).is_ok());
    args.name.push('a');
    edit.name = Some(args.name.clone());
    assert_eq!(create_body(&args).unwrap_err().code, "INVALID_INPUT");
    assert_eq!(update_body(&edit).unwrap_err().code, "INVALID_INPUT");
    args.name = "a".repeat(100);
    edit.name = Some(args.name.clone());
    args.description.as_mut().unwrap().push('a');
    edit.description = args.description.clone();
    assert_eq!(create_body(&args).unwrap_err().code, "INVALID_INPUT");
    assert_eq!(update_body(&edit).unwrap_err().code, "INVALID_INPUT");
}
