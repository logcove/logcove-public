mod support;

use logcove::{client::Api, credentials::CredentialStore};
use serde_json::json;
use support::{FakeClock, MemoryStore, Server, Step};

const TOKEN: &str = "test-session.signature";
const NEXT: &str = "renewed-session.signature";
const PROJECT_ID: &str = "prj_00000000-0000-4000-8000-000000000001";

fn user() -> serde_json::Value {
    json!({"id":"user-test","email":"test@example.test","email_verified":true,"name":"Test",
        "image":null,"created_at":"2026-09-08T00:00:00Z","updated_at":"2026-09-08T00:00:00Z"})
}

fn project() -> serde_json::Value {
    json!({"id":PROJECT_ID,"name":"API logs","description":"Request durations in milliseconds", "status":"active",
        "data_prefix":format!("logs/project={PROJECT_ID}/"),"created_at":"2026-09-08T00:00:00Z","updated_at":"2026-09-08T00:00:00Z"})
}

fn begin(expires_in: u64) -> Step {
    Step::json("POST", "/api/auth/device/code", None, 200, json!({
        "device_code":"private-device-code", "user_code":"TESTCODE", "interval":5, "expires_in":expires_in,
        "verification_uri":"http://localhost:5173/cli/login",
        "verification_uri_complete":"http://localhost:5173/cli/login?user_code=TESTCODE"
    })).body(json!({"client_id":"logcove-cli"}))
}

fn poll(status: u16, data: serde_json::Value) -> Step {
    Step::json("POST", "/api/auth/device/token", None, status, data).body(json!({
        "client_id":"logcove-cli", "device_code":"private-device-code", "grant_type":"urn:ietf:params:oauth:grant-type:device_code"
    }))
}

fn approved() -> Step {
    poll(
        200,
        json!({"access_token":TOKEN,"token_type":"Bearer","expires_in":604800}),
    )
}

#[test]
fn browser_login_pending_slow_down_and_persistence_across_clients() {
    let server = Server::start(vec![
        begin(600),
        poll(400, json!({"error":"authorization_pending"})),
        poll(400, json!({"error":"slow_down"})),
        approved(),
        Step::json(
            "GET",
            "/api/v1/me",
            Some(TOKEN),
            200,
            json!({"data":user()}),
        ),
    ]);
    let store = MemoryStore::default();
    let mut api = Api::new(server.origin.clone(), store.clone()).unwrap();
    let authorization = api.begin_login().unwrap();
    assert_eq!(
        authorization.verification_uri_complete,
        "http://localhost:5173/cli/login?user_code=TESTCODE"
    );
    let mut clock = FakeClock::default();
    api.complete_login(&authorization, &mut clock).unwrap();
    assert_eq!(clock.waits, vec![5, 5, 10]);
    drop(api);
    let mut restarted = Api::new(server.origin.clone(), store.clone()).unwrap();
    assert_eq!(restarted.whoami().unwrap().email, "test@example.test");
    assert_eq!(store.0.borrow().saves, 1);
    server.finish();
}

#[test]
fn polling_respects_rate_limit_retry_after() {
    let server = Server::start(vec![
        begin(600),
        poll(429, json!({})).header("Retry-After", "45"),
        approved(),
    ]);
    let mut api = Api::new(server.origin.clone(), MemoryStore::default()).unwrap();
    let authorization = api.begin_login().unwrap();
    let mut clock = FakeClock::default();
    api.complete_login(&authorization, &mut clock).unwrap();
    assert_eq!(clock.waits, vec![5, 45]);
    server.finish();
}

#[test]
fn denied_expired_and_consumed_authorizations_never_persist() {
    for (remote, code) in [
        ("access_denied", "AUTHORIZATION_DENIED"),
        ("expired_token", "AUTHORIZATION_EXPIRED"),
        ("invalid_grant", "AUTHORIZATION_INVALID"),
    ] {
        let server = Server::start(vec![begin(600), poll(400, json!({"error":remote}))]);
        let store = MemoryStore::default();
        let mut api = Api::new(server.origin.clone(), store.clone()).unwrap();
        let authorization = api.begin_login().unwrap();
        assert_eq!(
            api.complete_login(&authorization, &mut FakeClock::default())
                .unwrap_err()
                .code,
            code
        );
        assert!(store.read().unwrap().is_none());
        server.finish();
    }
}

#[test]
fn local_expiry_stops_without_an_extra_exchange() {
    let server = Server::start(vec![
        begin(6),
        poll(400, json!({"error":"authorization_pending"})),
    ]);
    let mut api = Api::new(server.origin.clone(), MemoryStore::default()).unwrap();
    let authorization = api.begin_login().unwrap();
    let mut clock = FakeClock::default();
    assert_eq!(
        api.complete_login(&authorization, &mut clock)
            .unwrap_err()
            .code,
        "AUTHORIZATION_EXPIRED"
    );
    assert_eq!(clock.waits, vec![5, 1]);
    server.finish();
}

#[test]
fn successful_exchange_with_unsigned_token_is_rejected() {
    let server = Server::start(vec![
        begin(600),
        poll(
            200,
            json!({"access_token":"unsigned-token","token_type":"Bearer"}),
        ),
    ]);
    let store = MemoryStore::default();
    let mut api = Api::new(server.origin.clone(), store.clone()).unwrap();
    let authorization = api.begin_login().unwrap();
    assert_eq!(
        api.complete_login(&authorization, &mut FakeClock::default())
            .unwrap_err()
            .code,
        "INVALID_RESPONSE"
    );
    assert!(store.read().unwrap().is_none());
    server.finish();
}

#[test]
fn save_failure_revokes_the_new_session() {
    let server = Server::start(vec![
        begin(600),
        approved(),
        Step::json(
            "POST",
            "/api/auth/sign-out",
            Some(TOKEN),
            200,
            json!({"success":true}),
        )
        .body(json!({})),
    ]);
    let store = MemoryStore::default();
    store.0.borrow_mut().fail_save = true;
    let mut api = Api::new(server.origin.clone(), store.clone()).unwrap();
    let authorization = api.begin_login().unwrap();
    let error = api
        .complete_login(&authorization, &mut FakeClock::default())
        .unwrap_err();
    assert_eq!(error.code, "CREDENTIAL_SAVE_FAILED");
    assert!(error.message.contains("was revoked"));
    assert!(!api.has_session());
    server.finish();
}

#[test]
fn discovery_follows_empty_pages_and_renews_credentials_between_requests() {
    let server = Server::start(vec![
        Step::json(
            "GET",
            "/data/v1/projects?limit=100",
            Some(TOKEN),
            200,
            json!({"data":[project()],"pagination":{"next_cursor":"a+b/="}}),
        )
        .header("set-auth-token", NEXT),
        Step::json(
            "GET",
            "/data/v1/projects?limit=100&cursor=a%2Bb%2F%3D",
            Some(NEXT),
            200,
            json!({"data":[],"pagination":{"next_cursor":"last"}}),
        )
        .header("set-auth-token", NEXT),
        Step::json(
            "GET",
            "/data/v1/projects?limit=100&cursor=last",
            Some(NEXT),
            200,
            json!({"data":[],"pagination":{"next_cursor":null}}),
        ),
        Step::json(
            "GET",
            &format!("/api/v1/projects/{PROJECT_ID}"),
            Some(NEXT),
            200,
            json!({"data":project()}),
        ),
    ]);
    let store = MemoryStore::with_token(TOKEN);
    let mut api = Api::new(server.origin.clone(), store.clone()).unwrap();
    assert_eq!(api.projects().unwrap().len(), 1);
    assert_eq!(
        api.project(PROJECT_ID).unwrap().description.as_deref(),
        Some("Request durations in milliseconds")
    );
    assert_eq!(store.read().unwrap().as_deref(), Some(NEXT));
    assert_eq!(store.0.borrow().saves, 1);
    server.finish();
}

#[test]
fn repeated_pagination_cursor_fails_instead_of_looping() {
    let server = Server::start(vec![
        Step::json(
            "GET",
            "/data/v1/projects?limit=100",
            Some(TOKEN),
            200,
            json!({"data":[],"pagination":{"next_cursor":"same"}}),
        ),
        Step::json(
            "GET",
            "/data/v1/projects?limit=100&cursor=same",
            Some(TOKEN),
            200,
            json!({"data":[],"pagination":{"next_cursor":"same"}}),
        ),
    ]);
    let mut api = Api::new(server.origin.clone(), MemoryStore::with_token(TOKEN)).unwrap();
    assert_eq!(api.projects().err().unwrap().code, "INVALID_PAGINATION");
    server.finish();
}

#[test]
fn unauthorized_clears_session_and_does_not_trust_renewal_header() {
    let server = Server::start(vec![Step::json(
        "GET",
        "/api/v1/me",
        Some(TOKEN),
        401,
        json!({"error":{"code":"UNAUTHENTICATED"}}),
    )
    .header("set-auth-token", NEXT)]);
    let store = MemoryStore::with_token(TOKEN);
    let mut api = Api::new(server.origin.clone(), store.clone()).unwrap();
    assert_eq!(api.whoami().err().unwrap().code, "UNAUTHENTICATED");
    assert!(store.read().unwrap().is_none());
    assert_eq!(api.whoami().err().unwrap().code, "UNAUTHENTICATED");
    server.finish();
}

#[test]
fn stale_unauthorized_response_keeps_a_new_login_in_the_store() {
    let server = Server::start(vec![Step::json(
        "GET",
        "/api/v1/me",
        Some(TOKEN),
        401,
        json!({}),
    )]);
    let store = MemoryStore::with_token(TOKEN);
    let mut old_process = Api::new(server.origin.clone(), store.clone()).unwrap();
    store.save(NEXT).unwrap();
    assert_eq!(old_process.whoami().err().unwrap().code, "UNAUTHENTICATED");
    assert!(!old_process.has_session());
    assert_eq!(store.read().unwrap().as_deref(), Some(NEXT));
    server.finish();
}

#[test]
fn logout_of_an_old_session_keeps_a_new_login_in_the_store() {
    let server = Server::start(vec![Step::json(
        "POST",
        "/api/auth/sign-out",
        Some(TOKEN),
        200,
        json!({"success":true}),
    )
    .body(json!({}))]);
    let store = MemoryStore::with_token(TOKEN);
    let mut old_process = Api::new(server.origin.clone(), store.clone()).unwrap();
    store.save(NEXT).unwrap();
    old_process.logout().unwrap();
    assert!(!old_process.has_session());
    assert_eq!(store.read().unwrap().as_deref(), Some(NEXT));
    server.finish();
}

#[test]
fn permission_and_server_errors_keep_credentials_and_redact_response_body() {
    for (status, code) in [
        (403, "ACCESS_DENIED"),
        (404, "NOT_FOUND"),
        (500, "SERVER_ERROR"),
    ] {
        let server = Server::start(vec![Step::json(
            "GET",
            "/api/v1/me",
            Some(TOKEN),
            status,
            json!({"secret":TOKEN}),
        )
        .header("x-request-id", "request-test")]);
        let store = MemoryStore::with_token(TOKEN);
        let mut api = Api::new(server.origin.clone(), store.clone()).unwrap();
        let error = api.whoami().err().unwrap();
        assert_eq!(error.code, code);
        assert_eq!(error.request_id.as_deref(), Some("request-test"));
        assert!(!serde_json::to_string(&error).unwrap().contains(TOKEN));
        assert!(store.read().unwrap().is_some());
        server.finish();
    }
}

#[test]
fn redirects_are_not_followed_or_exposed() {
    let server = Server::start(vec![Step::json(
        "GET",
        "/api/v1/me",
        Some(TOKEN),
        302,
        json!({}),
    )
    .header("Location", "http://127.0.0.1:1/private?token=secret")]);
    let mut api = Api::new(server.origin.clone(), MemoryStore::with_token(TOKEN)).unwrap();
    let error = api.whoami().err().unwrap();
    assert_eq!(error.code, "REDIRECT_REFUSED");
    assert!(!error.message.contains("secret"));
    server.finish();
}

#[test]
fn logout_failure_retains_retryable_session_then_success_clears_it() {
    let server = Server::start(vec![
        Step::json("POST", "/api/auth/sign-out", Some(TOKEN), 500, json!({})).body(json!({})),
        Step::json(
            "POST",
            "/api/auth/sign-out",
            Some(TOKEN),
            200,
            json!({"success":true}),
        )
        .body(json!({})),
    ]);
    let store = MemoryStore::with_token(TOKEN);
    let mut api = Api::new(server.origin.clone(), store.clone()).unwrap();
    assert_eq!(api.logout().unwrap_err().code, "SERVER_ERROR");
    assert!(store.read().unwrap().is_some());
    api.logout().unwrap();
    assert!(store.read().unwrap().is_none());
    api.logout().unwrap();
    server.finish();
}

#[test]
fn logout_already_expired_session_clears_local_entry() {
    let server = Server::start(vec![Step::json(
        "POST",
        "/api/auth/sign-out",
        Some(TOKEN),
        401,
        json!({}),
    )]);
    let store = MemoryStore::with_token(TOKEN);
    let mut api = Api::new(server.origin.clone(), store.clone()).unwrap();
    api.logout().unwrap();
    assert!(store.read().unwrap().is_none());
    server.finish();
}

#[test]
fn logout_local_delete_failure_does_not_claim_full_success() {
    let server = Server::start(vec![Step::json(
        "POST",
        "/api/auth/sign-out",
        Some(TOKEN),
        200,
        json!({"success":true}),
    )]);
    let store = MemoryStore::with_token(TOKEN);
    store.0.borrow_mut().fail_delete = true;
    let mut api = Api::new(server.origin.clone(), store.clone()).unwrap();
    assert_eq!(api.logout().unwrap_err().code, "CREDENTIAL_DELETE_FAILED");
    assert!(store.read().unwrap().is_some());
    server.finish();
}

#[test]
fn renewal_save_failure_keeps_success_and_uses_new_token_in_memory() {
    let server = Server::start(vec![
        Step::json(
            "GET",
            "/api/v1/me",
            Some(TOKEN),
            200,
            json!({"data":user()}),
        )
        .header("set-auth-token", NEXT),
        Step::json("GET", "/api/v1/me", Some(NEXT), 200, json!({"data":user()})),
    ]);
    let store = MemoryStore::with_token(TOKEN);
    store.0.borrow_mut().fail_save = true;
    let mut api = Api::new(server.origin.clone(), store).unwrap();
    api.whoami().unwrap();
    api.whoami().unwrap();
    server.finish();
}

#[test]
fn missing_session_and_path_injection_are_rejected_before_network() {
    let origin = reqwest::Url::parse("http://127.0.0.1:1").unwrap();
    let mut api = Api::new(origin, MemoryStore::default()).unwrap();
    assert_eq!(api.whoami().err().unwrap().code, "UNAUTHENTICATED");
    for id in [
        "../keys",
        "prj_../../keys",
        "prj_00000000-0000-4000-8000-000000000001?x=1",
    ] {
        assert_eq!(api.project(id).err().unwrap().code, "INVALID_PROJECT_ID");
    }
}

#[test]
fn network_failure_keeps_session() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let origin =
        reqwest::Url::parse(&format!("http://{}", listener.local_addr().unwrap())).unwrap();
    drop(listener);
    let store = MemoryStore::with_token(TOKEN);
    let mut api = Api::new(origin, store.clone()).unwrap();
    let error = api.whoami().err().unwrap();
    assert_eq!(error.code, "NETWORK_ERROR");
    assert!(store.read().unwrap().is_some());
    assert!(!error.message.contains(TOKEN));
}

#[test]
fn invalid_browser_authorization_links_are_rejected_before_opening() {
    for url in [
        "javascript:alert(1)",
        "http://remote.example.test/cli/login?user_code=TESTCODE",
        "https://user:password@example.test/cli/login?user_code=TESTCODE",
        "https://example.test/cli/login?user_code=WRONGCODE",
    ] {
        let mut step = begin(600);
        let mut body: serde_json::Value = serde_json::from_str(&step.response).unwrap();
        body["verification_uri_complete"] = json!(url);
        step.response = body.to_string();
        let server = Server::start(vec![step]);
        let api = Api::new(server.origin.clone(), MemoryStore::default()).unwrap();
        assert_eq!(api.begin_login().err().unwrap().code, "INVALID_RESPONSE");
        server.finish();
    }
}

#[test]
fn authorization_expiry_between_pages_fails_without_returning_partial_projects() {
    let server = Server::start(vec![
        Step::json(
            "GET",
            "/data/v1/projects?limit=100",
            Some(TOKEN),
            200,
            json!({"data":[project()],"pagination":{"next_cursor":"next"}}),
        ),
        Step::json(
            "GET",
            "/data/v1/projects?limit=100&cursor=next",
            Some(TOKEN),
            401,
            json!({"error":{"code":"UNAUTHENTICATED"}}),
        ),
    ]);
    let store = MemoryStore::with_token(TOKEN);
    let mut api = Api::new(server.origin.clone(), store.clone()).unwrap();
    assert_eq!(api.projects().err().unwrap().code, "UNAUTHENTICATED");
    assert!(store.read().unwrap().is_none());
    server.finish();
}
