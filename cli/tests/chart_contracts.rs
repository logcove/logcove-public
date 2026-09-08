mod support;

use logcove::{
    charts::{create_body, result_file, update_body, CreateArgs, UpdateArgs},
    client::Api,
};
use serde_json::{json, Value};
use support::{Directory, MemoryStore, Server, Step};

const CHART: &str = "chart_00000000-0000-4000-8000-000000000001";
const PROJECT: &str = "prj_00000000-0000-4000-8000-000000000001";
const TOKEN: &str = "session.signature";

fn result() -> Value {
    json!({"computed_at":"2026-09-08T00:00:00Z","data":[{"service":"api","requests":12}]})
}
fn spec() -> Value {
    json!({"$schema":"https://vega.github.io/schema/vega-lite/v6.json","data":{"name":"result"},"mark":"bar"})
}
fn create(dir: &Directory) -> CreateArgs {
    CreateArgs {
        name: "Requests".into(),
        description: Some("Request count".into()),
        project_ids: vec![PROJECT.into()],
        sql_file: dir.write(
            "query.sql",
            "select service, count(*) as requests from logs group by service",
        ),
        spec_file: dir.write("spec.json", &spec().to_string()),
        result_file: Some(dir.write("result.json", &result().to_string())),
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
fn create_sends_definition_and_initial_result_without_project_lookup() {
    let dir = Directory::default();
    let args = create(&dir);
    let body = create_body(&args).unwrap();
    assert_eq!(body["project_ids"], json!([PROJECT]));
    let server = Server::start(vec![Step::json(
        "POST",
        "/api/v1/charts",
        Some(TOKEN),
        201,
        json!({"data":{"id":CHART,"revision":1,"result":result()}}),
    )
    .body(body)]);
    let mut api = Api::new(server.origin.clone(), MemoryStore::with_token(TOKEN)).unwrap();
    let chart = api.create_chart(&args).unwrap();
    assert_eq!(chart["id"], CHART);
    assert_eq!(chart["result"]["data"][0]["requests"], 12);
    server.finish();
}

#[test]
fn update_omits_unmodified_fields_and_supports_explicit_clear() {
    let mut args = update();
    args.name = Some("Renamed".into());
    assert_eq!(
        update_body(&args).unwrap(),
        json!({"revision":1,"name":"Renamed"})
    );
    args.clear_description = true;
    args.clear_projects = true;
    assert_eq!(
        update_body(&args).unwrap(),
        json!({"revision":1,"name":"Renamed","description":null,"project_ids":[]})
    );
    let server = Server::start(vec![Step::json(
        "PATCH",
        &format!("/api/v1/charts/{CHART}"),
        Some(TOKEN),
        200,
        json!({"data":{"id":CHART,"revision":2,"result":{"row_count":1}}}),
    )
    .body(update_body(&args).unwrap())]);
    let mut api = Api::new(server.origin.clone(), MemoryStore::with_token(TOKEN)).unwrap();
    assert_eq!(api.update_chart(&args).unwrap()["result"]["row_count"], 1);
    server.finish();
}

#[test]
fn sql_update_null_result_is_preserved_and_upload_uses_existing_result_contract() {
    let dir = Directory::default();
    let mut args = update();
    args.sql_file = Some(dir.write("query.sql", "select 1"));
    let file = dir.write("result.json", &result().to_string());
    let server = Server::start(vec![
        Step::json(
            "PATCH",
            &format!("/api/v1/charts/{CHART}"),
            Some(TOKEN),
            200,
            json!({"data":{"id":CHART,"revision":2,"result":null}}),
        )
        .body(json!({"revision":1,"sql":"select 1"})),
        Step::json(
            "PUT",
            &format!("/api/v1/charts/{CHART}/result"),
            Some(TOKEN),
            200,
            json!({"data":result()}),
        )
        .body(result()),
    ]);
    let mut api = Api::new(server.origin.clone(), MemoryStore::with_token(TOKEN)).unwrap();
    assert!(api.update_chart(&args).unwrap()["result"].is_null());
    assert_eq!(api.put_chart_result(CHART, &file).unwrap(), result());
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
    assert_eq!(error.code, "CONFLICT");
    assert_eq!(error.request_id.as_deref(), Some("chart-conflict"));
    server.finish();
}

#[test]
fn delete_handles_empty_204_body_and_errors_remain_errors() {
    let mut deleted = Step::json(
        "DELETE",
        &format!("/api/v1/charts/{CHART}"),
        Some(TOKEN),
        204,
        Value::Null,
    );
    deleted.response.clear();
    let server = Server::start(vec![
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
    api.delete_chart(CHART).unwrap();
    assert_eq!(api.chart(CHART).unwrap_err().code, "NOT_FOUND");
    server.finish();
}

#[test]
fn list_keeps_tag_filter_across_pages_and_detail_can_exceed_old_two_mib_limit() {
    let large = "x".repeat(2 * 1024 * 1024 + 100);
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
        Step::json(
            "GET",
            &format!("/api/v1/charts/{CHART}"),
            Some(TOKEN),
            200,
            json!({"data":{"id":CHART,"result":{"data":[{"message":large}]}}}),
        ),
    ]);
    let mut api = Api::new(server.origin.clone(), MemoryStore::with_token(TOKEN)).unwrap();
    assert_eq!(api.charts(Some(PROJECT)).unwrap().len(), 1);
    assert!(
        api.chart(CHART).unwrap()["result"]["data"][0]["message"]
            .as_str()
            .unwrap()
            .len()
            > 2 * 1024 * 1024
    );
    server.finish();
}

#[test]
fn file_inputs_validate_shapes_limits_and_missing_updates() {
    let dir = Directory::default();
    for value in [
        json!([]),
        json!({"computed_at":"2026-09-08T00:00:00","data":[]}),
        json!({"computed_at":"2026-09-08T00:00:00Z","data":[1]}),
        json!({"computed_at":"2026-09-08T00:00:00Z","data":[],"query_params":{}}),
        json!({"computed_at":"2026-09-08T00:00:00Z","data":vec![json!({});10001]}),
    ] {
        assert!(result_file(&dir.write("bad.json", &value.to_string())).is_err());
    }
    assert!(result_file(&dir.write(
        "empty.json",
        "{\"computed_at\":\"2026-09-08T00:00:00Z\",\"data\":[]}"
    ))
    .is_ok());
    assert!(update_body(&update()).is_err());
    let mut args = update();
    args.name = Some("Test".into());
    args.revision = 0;
    assert!(update_body(&args).is_err());
    let mut args = create(&dir);
    args.sql_file = dir.write("empty.sql", "  ");
    assert!(create_body(&args).is_err());
    args.sql_file = dir.write("large.sql", &"x".repeat(64 * 1024 + 1));
    assert_eq!(create_body(&args).unwrap_err().code, "PAYLOAD_TOO_LARGE");
    args.sql_file = dir.write("query.sql", "select 1");
    args.spec_file = dir.write(
        "external.json",
        "{\"data\":{\"url\":\"https://example.test\"}}",
    );
    assert!(create_body(&args).is_err());
}

#[test]
fn chart_operations_reject_id_path_injection_before_request() {
    let mut api = Api::new(
        reqwest::Url::parse("http://127.0.0.1:1").unwrap(),
        MemoryStore::with_token(TOKEN),
    )
    .unwrap();
    assert_eq!(api.chart("../keys").unwrap_err().code, "INVALID_INPUT");
    assert_eq!(
        api.delete_chart("chart_bad?all=true").unwrap_err().code,
        "INVALID_INPUT"
    );
}

#[test]
fn result_timestamps_are_normalized_to_utc_milliseconds() {
    let dir = Directory::default();
    for (input, expected) in [
        (
            "2026-09-08T00:00:00.123456+00:00",
            "2026-09-08T00:00:00.123Z",
        ),
        (
            "2026-09-08T08:00:00.999999999+08:00",
            "2026-09-08T00:00:00.999Z",
        ),
        ("2026-09-08T00:00:00Z", "2026-09-08T00:00:00Z"),
    ] {
        let input = json!({"computed_at":input,"data":[{"value":1}]}).to_string();
        let file = dir.write("result.json", &input);
        let result = result_file(&file).unwrap();
        assert_eq!(result["computed_at"], expected);
        assert_eq!(result["data"], json!([{"value":1}]));
        assert_eq!(std::fs::read_to_string(file).unwrap(), input);
    }
    for invalid in [
        "2026-02-30T00:00:00Z",
        "2100-01-01T00:00:00Z",
        "2026-09-08T00:00:00",
    ] {
        assert!(result_file(&dir.write(
            "bad.json",
            &json!({"computed_at":invalid,"data":[]}).to_string()
        ))
        .is_err());
    }
}

#[test]
fn utf8_bom_is_accepted_for_sql_spec_and_results_without_changing_payload_text() {
    let dir = Directory::default();
    let mut args = create(&dir);
    let sql = "SELECT '\u{feff}value' AS message;\n";
    args.sql_file = dir.write("query.sql", &format!("\u{feff}{sql}"));
    args.spec_file = dir.write("spec.json", &format!("\u{feff}{}", spec()));
    args.result_file = Some(dir.write("result.json", &format!("\u{feff}{}", result())));
    let body = create_body(&args).unwrap();
    assert_eq!(body["sql"], sql);
    assert_eq!(body["vega_lite_spec"], spec());
    assert_eq!(body["result"], result());
    let file = dir.write("boundary.txt", "\u{feff}abcd");
    assert_eq!(logcove::files::text(&file, 4).unwrap(), "abcd");
    assert_eq!(
        logcove::files::text(&file, 3).unwrap_err().code,
        "PAYLOAD_TOO_LARGE"
    );
}
