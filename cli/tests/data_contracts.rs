#[path = "support/downloads.rs"]
mod downloads;
mod support;

use logcove::{client::Api, credentials::CredentialStore, data::validate_dates};
use serde_json::{json, Value};
use support::{Directory, MemoryStore, Server, Step};

const PROJECT: &str = "prj_00000000-0000-4000-8000-000000000001";
const TOKEN: &str = "session.signature";
const BYTES: &str = "PAR1test-dataPAR1";

fn key(index: usize) -> String {
    format!("logs/project={PROJECT}/date=2026-09-08/{index}.parquet")
}
fn object(index: usize) -> Value {
    json!({"key":key(index),"size":BYTES.len(),"etag":"file-etag","uploaded_at":"2026-09-08T00:00:00Z","ingest_date":"2026-09-08"})
}
fn list(objects: Vec<Value>, cursor: Option<&str>, request_cursor: Option<&str>) -> Step {
    let mut path = format!(
        "/data/v1/projects/{PROJECT}/files?start_date=2026-09-08&end_date=2026-09-08&limit=1000"
    );
    if let Some(cursor) = request_cursor {
        path.push_str(&format!("&cursor={cursor}"));
    }
    Step::json(
        "GET",
        &path,
        Some(TOKEN),
        200,
        json!({"data":objects,"pagination":{"next_cursor":cursor}}),
    )
}
fn links(keys: &[usize], storage: &reqwest::Url) -> Step {
    Step::json("POST", &format!("/data/v1/projects/{PROJECT}/download-urls"), Some(TOKEN), 200,
        json!({"data":keys.iter().map(|i|json!({"key":key(*i),"url":storage.join(&format!("/{i}?signature=private-signature")).unwrap().as_str(),"expires_at":"2100-01-01T00:00:00Z"})).collect::<Vec<_>>()}))
        .body(json!({"keys":keys.iter().map(|i|key(*i)).collect::<Vec<_>>()}))
}
fn download(index: usize) -> Step {
    let mut step = Step::json(
        "GET",
        &format!("/{index}?signature=private-signature"),
        None,
        200,
        Value::Null,
    )
    .header("ETag", "\"file-etag\"");
    step.response = BYTES.into();
    step
}

fn cache_manifest(
    folder: &Directory,
    origin: &reqwest::Url,
    files: Vec<Value>,
) -> std::path::PathBuf {
    let path = folder.0.join("manifest.json");
    std::fs::write(
        &path,
        serde_json::to_vec(&json!({
            "schema_version":1,"api_url":origin.origin().ascii_serialization(),"project_id":PROJECT,
            "start_date":"2026-09-08","end_date":"2026-09-08","completed_at":"2026-09-08T12:00:00Z",
            "files":files,
        }))
        .unwrap(),
    )
    .unwrap();
    path
}

fn cached_object(index: usize) -> Value {
    let mut file = object(index);
    file["path"] = json!(format!("{index}.parquet"));
    file
}

#[test]
fn reuse_downloads_only_missing_files_and_keeps_complete_portable_manifest() {
    let storage = Server::start(vec![download(1)]);
    let server = Server::start(vec![
        list(vec![object(0), object(1), object(2)], None, None),
        links(&[1], &storage.origin),
    ]);
    let cache = Directory::default();
    for index in [0, 2] {
        std::fs::write(cache.0.join(format!("{index}.parquet")), BYTES).unwrap();
    }
    let manifest = cache_manifest(
        &cache,
        &server.origin,
        vec![cached_object(0), cached_object(2)],
    );
    let mut api = Api::new(server.origin.clone(), MemoryStore::with_token(TOKEN)).unwrap();
    let output = Directory::default();
    let report = api
        .pull_with_reuse(
            PROJECT,
            ("2026-09-08", "2026-09-08"),
            &output.0,
            4,
            &[manifest],
        )
        .unwrap();
    assert_eq!(
        (
            report.file_count,
            report.downloaded_file_count,
            report.reused_file_count
        ),
        (3, 1, 2)
    );
    assert_eq!(report.total_bytes, 3 * BYTES.len() as u64);
    drop(cache);
    let manifest: Value =
        serde_json::from_slice(&std::fs::read(&report.manifest_path).unwrap()).unwrap();
    for (index, file) in manifest["files"].as_array().unwrap().iter().enumerate() {
        assert_eq!(file["key"], key(index));
        let path = report
            .manifest_path
            .parent()
            .unwrap()
            .join(file["path"].as_str().unwrap());
        assert_eq!(std::fs::read_to_string(path).unwrap(), BYTES);
    }
}

#[test]
fn reuse_all_files_still_lists_but_never_signs_or_downloads() {
    let server = Server::start(vec![list(vec![object(0)], None, None)]);
    let cache = Directory::default();
    std::fs::write(cache.0.join("0.parquet"), BYTES).unwrap();
    let manifest = cache_manifest(&cache, &server.origin, vec![cached_object(0)]);
    let output = Directory::default();
    let mut api = Api::new(server.origin.clone(), MemoryStore::with_token(TOKEN)).unwrap();
    let report = api
        .pull_with_reuse(
            PROJECT,
            ("2026-09-08", "2026-09-08"),
            &output.0,
            4,
            &[manifest.clone(), manifest],
        )
        .unwrap();
    assert_eq!(
        (
            report.file_count,
            report.downloaded_file_count,
            report.reused_file_count
        ),
        (1, 0, 1)
    );
}

#[test]
fn reuse_redownloads_changed_missing_truncated_and_invalid_cached_files() {
    for problem in [
        "etag",
        "size",
        "missing",
        "truncated",
        "header",
        "footer",
        "escape",
    ] {
        let storage = Server::start(vec![download(0)]);
        let server = Server::start(vec![
            list(vec![object(0)], None, None),
            links(&[0], &storage.origin),
        ]);
        let cache = Directory::default();
        let mut file = cached_object(0);
        let bytes = match problem {
            "truncated" => "PAR1",
            "header" => "FAILtest-dataPAR1",
            "footer" => "PAR1test-dataFAIL",
            _ => BYTES,
        };
        if problem != "missing" {
            std::fs::write(cache.0.join("0.parquet"), bytes).unwrap();
        }
        match problem {
            "etag" => file["etag"] = json!("old-etag"),
            "size" => file["size"] = json!(999),
            "escape" => file["path"] = json!("../0.parquet"),
            _ => {}
        }
        let manifest = cache_manifest(&cache, &server.origin, vec![file]);
        let output = Directory::default();
        let mut api = Api::new(server.origin.clone(), MemoryStore::with_token(TOKEN)).unwrap();
        let report = api
            .pull_with_reuse(
                PROJECT,
                ("2026-09-08", "2026-09-08"),
                &output.0,
                4,
                &[manifest],
            )
            .unwrap();
        assert_eq!(
            (report.downloaded_file_count, report.reused_file_count),
            (1, 0),
            "{problem}"
        );
    }
}

#[test]
fn reuse_excludes_objects_no_longer_in_current_listing() {
    let server = Server::start(vec![list(vec![], None, None)]);
    let cache = Directory::default();
    std::fs::write(cache.0.join("0.parquet"), BYTES).unwrap();
    let manifest = cache_manifest(&cache, &server.origin, vec![cached_object(0)]);
    let output = Directory::default();
    let mut api = Api::new(server.origin.clone(), MemoryStore::with_token(TOKEN)).unwrap();
    let report = api
        .pull_with_reuse(
            PROJECT,
            ("2026-09-08", "2026-09-08"),
            &output.0,
            4,
            &[manifest],
        )
        .unwrap();
    assert_eq!((report.file_count, report.reused_file_count), (0, 0));
}

#[test]
fn reuse_rejects_other_origins_projects_and_incomplete_manifests() {
    for field in ["api_url", "project_id", "completed_at", "schema_version"] {
        let server = Server::start(vec![list(vec![object(0)], None, None)]);
        let cache = Directory::default();
        let path = cache_manifest(&cache, &server.origin, vec![cached_object(0)]);
        let mut manifest: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        manifest[field] = if field == "schema_version" {
            json!(999)
        } else {
            json!("other")
        };
        std::fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        let mut api = Api::new(server.origin.clone(), MemoryStore::with_token(TOKEN)).unwrap();
        let output = Directory::default();
        assert_eq!(
            api.pull_with_reuse(PROJECT, ("2026-09-08", "2026-09-08"), &output.0, 4, &[path])
                .err()
                .unwrap()
                .code,
            "INVALID_CACHE"
        );
    }
}

#[test]
fn cached_data_does_not_bypass_current_read_authorization() {
    let mut denied = list(vec![], None, None);
    denied.status = 403;
    let server = Server::start(vec![denied]);
    let cache = Directory::default();
    let path = cache_manifest(&cache, &server.origin, vec![cached_object(0)]);
    let mut api = Api::new(server.origin.clone(), MemoryStore::with_token(TOKEN)).unwrap();
    let output = Directory::default();
    assert_eq!(
        api.pull_with_reuse(PROJECT, ("2026-09-08", "2026-09-08"), &output.0, 4, &[path])
            .err()
            .unwrap()
            .code,
        "ACCESS_DENIED"
    );
}

#[test]
fn inclusive_utc_dates_check_calendar_and_93_day_boundary() {
    for (from, to) in [("2026-01-01", "2026-04-03"), ("2024-02-29", "2024-02-29")] {
        validate_dates(from, to).unwrap();
    }
    for (from, to) in [
        ("2026-01-01", "2026-04-04"),
        ("2026-02-29", "2026-03-01"),
        ("2026-1-01", "2026-01-01"),
        ("2026-09-09", "2026-09-08"),
        ("2026-09-08T00:00:00Z", "2026-09-08"),
    ] {
        assert_eq!(
            validate_dates(from, to).unwrap_err().code,
            "INVALID_DATE_RANGE"
        );
    }
}

#[test]
fn concurrent_downloads_fill_free_slots_reuse_connections_and_keep_manifest_order() {
    let storage = downloads::Storage::start(|index, _, monitor| {
        if index < 3 {
            monitor.wait_for(|s| s.started.len() >= 3);
        }
        if index == 0 {
            monitor.wait_for(|s| s.finished.contains(&3));
        }
        (200, format!("PAR1{index:09}PAR1"))
    });
    let server = Server::start(vec![
        list((0..7).map(object).collect(), None, None),
        links(&(0..7).collect::<Vec<_>>(), &storage.origin),
    ]);
    let mut api = Api::new(server.origin.clone(), MemoryStore::with_token(TOKEN)).unwrap();
    let output = Directory::default();
    let report = api
        .pull(PROJECT, "2026-09-08", "2026-09-08", &output.0, 3)
        .unwrap();
    let manifest: Value =
        serde_json::from_slice(&std::fs::read(&report.manifest_path).unwrap()).unwrap();
    for (index, file) in manifest["files"].as_array().unwrap().iter().enumerate() {
        assert_eq!(file["key"], key(index));
        assert_eq!(file["path"], format!("{index:06}.parquet"));
        let path = report
            .manifest_path
            .parent()
            .unwrap()
            .join(file["path"].as_str().unwrap());
        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            format!("PAR1{index:09}PAR1")
        );
    }
    assert_eq!(report.file_count, 7);
    assert_eq!(report.total_bytes, 7 * BYTES.len() as u64);
    let stats = storage.finish();
    assert_eq!(stats.peak, 3);
    assert!(
        stats.connections <= 3,
        "Completed requests should reuse pooled connections"
    );
    assert_eq!(stats.started.len(), 7);
    assert!(
        stats.finished.iter().position(|i| *i == 3) < stats.finished.iter().position(|i| *i == 0)
    );
    server.finish();
}

#[test]
fn concurrent_failure_joins_active_downloads_and_leaves_no_manifest() {
    let storage = downloads::Storage::start(|index, _, monitor| {
        monitor.wait_for(|s| s.started.len() >= 3);
        if index == 0 {
            return (200, "this-is-not-par1!".into());
        }
        monitor.wait_for(|s| s.finished.contains(&0));
        std::thread::sleep(std::time::Duration::from_millis(100));
        (200, BYTES.into())
    });
    let server = Server::start(vec![
        list((0..6).map(object).collect(), None, None),
        links(&(0..6).collect::<Vec<_>>(), &storage.origin),
    ]);
    let mut api = Api::new(server.origin.clone(), MemoryStore::with_token(TOKEN)).unwrap();
    let output = Directory::default();
    let error = api
        .pull(PROJECT, "2026-09-08", "2026-09-08", &output.0, 3)
        .err()
        .unwrap();
    assert_eq!(error.code, "INVALID_PARQUET");
    assert!(!error.message.contains("private-signature"));
    let stats = storage.monitor.snapshot();
    assert_eq!(stats.active, 0);
    assert_eq!(stats.finished.len(), 3);
    let run = std::fs::read_dir(&output.0)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let mut names: Vec<_> = std::fs::read_dir(run)
        .unwrap()
        .map(|p| p.unwrap().file_name())
        .collect();
    names.sort();
    assert_eq!(names, ["000001.parquet", "000002.parquet"]);
    let stats = storage.finish();
    assert_eq!(stats.started.len(), 3);
    server.finish();
}

#[test]
fn eight_downloads_can_overlap_across_bounded_signing_batches() {
    let storage = downloads::Storage::start(|index, _, monitor| {
        if index < 8 {
            monitor.wait_for(|s| s.started.len() >= 8);
        }
        (200, BYTES.into())
    });
    let server = Server::start(vec![
        list((0..22).map(object).collect(), None, None),
        links(&(0..20).collect::<Vec<_>>(), &storage.origin),
        links(&[20, 21], &storage.origin),
    ]);
    let mut api = Api::new(server.origin.clone(), MemoryStore::with_token(TOKEN)).unwrap();
    let report = api
        .pull(
            PROJECT,
            "2026-09-08",
            "2026-09-08",
            &Directory::default().0,
            8,
        )
        .unwrap();
    assert_eq!(report.file_count, 22);
    let stats = storage.finish();
    assert_eq!(stats.peak, 8);
    assert_eq!(stats.started.len(), 22);
    server.finish();
}

#[test]
fn concurrent_denied_download_is_resigned_only_once() {
    for always_denied in [false, true] {
        let storage = downloads::Storage::start(move |index, attempt, monitor| {
            monitor.wait_for(|s| s.started.len() >= 3);
            if index == 0 && (attempt == 1 || always_denied) {
                (403, String::new())
            } else {
                (200, BYTES.into())
            }
        });
        let server = Server::start(vec![
            list((0..3).map(object).collect(), None, None),
            links(&[0, 1, 2], &storage.origin),
            links(&[0], &storage.origin),
        ]);
        let store = MemoryStore::with_token(TOKEN);
        let mut api = Api::new(server.origin.clone(), store.clone()).unwrap();
        let output = Directory::default();
        let result = api.pull(PROJECT, "2026-09-08", "2026-09-08", &output.0, 3);
        if always_denied {
            assert_eq!(result.err().unwrap().code, "DOWNLOAD_DENIED");
            let run = std::fs::read_dir(&output.0)
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path();
            assert!(!run.join("manifest.json").exists());
        } else {
            assert_eq!(result.unwrap().file_count, 3);
        }
        assert!(store.read().unwrap().is_some());
        let stats = storage.finish();
        assert_eq!(stats.started.iter().filter(|i| **i == 0).count(), 2);
        assert!(stats.peak <= 3);
        server.finish();
    }
}

#[test]
fn signing_failure_during_concurrent_download_waits_for_started_file() {
    let storage = downloads::Storage::start(|_, _, _| (200, BYTES.into()));
    let mut signed = links(&[0, 1], &storage.origin);
    let mut body: Value = serde_json::from_str(&signed.response).unwrap();
    body["data"][1]["expires_at"] = json!("2000-01-01T00:00:00Z");
    signed.response = body.to_string();
    let server = Server::start(vec![
        list(vec![object(0), object(1)], None, None),
        signed,
        Step::json(
            "POST",
            &format!("/data/v1/projects/{PROJECT}/download-urls"),
            Some(TOKEN),
            500,
            json!({}),
        )
        .body(json!({"keys":[key(1)]})),
    ]);
    let mut api = Api::new(server.origin.clone(), MemoryStore::with_token(TOKEN)).unwrap();
    let output = Directory::default();
    let error = api
        .pull(PROJECT, "2026-09-08", "2026-09-08", &output.0, 3)
        .err()
        .unwrap();
    assert_eq!(error.code, "SERVER_ERROR");
    let run = std::fs::read_dir(&output.0)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert!(run.join("000000.parquet").exists());
    assert!(!run.join("manifest.json").exists());
    let stats = storage.finish();
    assert_eq!(stats.started, vec![0]);
    assert_eq!(stats.finished, vec![0]);
    server.finish();
}

#[test]
fn paginated_downloads_use_separate_auth_free_client_and_publish_portable_manifest() {
    let storage = Server::start(vec![download(0), download(1)]);
    let api_server = Server::start(vec![
        list(vec![object(0)], Some("next"), None),
        list(vec![], Some("last"), Some("next")),
        list(vec![object(1)], None, Some("last")),
        links(&[0, 1], &storage.origin),
    ]);
    let mut api = Api::new(api_server.origin.clone(), MemoryStore::with_token(TOKEN)).unwrap();
    let output = Directory::default();
    output.write("old.parquet", "old-data-not-in-this-pull");
    let report = api
        .pull(PROJECT, "2026-09-08", "2026-09-08", &output.0, 1)
        .unwrap();
    assert_eq!(report.file_count, 2);
    assert_eq!(report.total_bytes, 2 * BYTES.len() as u64);
    let text = std::fs::read_to_string(&report.manifest_path).unwrap();
    assert!(
        !text.contains("private-signature")
            && !text.contains(TOKEN)
            && !text.contains("old.parquet")
    );
    let manifest: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(manifest["project_id"], PROJECT);
    assert_eq!(manifest["schema_version"], 1);
    for file in manifest["files"].as_array().unwrap() {
        let path = report
            .manifest_path
            .parent()
            .unwrap()
            .join(file["path"].as_str().unwrap());
        assert_eq!(std::fs::read_to_string(path).unwrap(), BYTES);
    }
    storage.finish();
    api_server.finish();
}

#[test]
fn downloads_reuse_one_keep_alive_connection_without_api_credentials() {
    use std::{
        io::{BufRead, BufReader, Write},
        net::TcpListener,
        thread,
        time::Duration,
    };

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin =
        reqwest::Url::parse(&format!("http://{}", listener.local_addr().unwrap())).unwrap();
    let handle = thread::spawn(move || {
        // Accept exactly one connection: subsequent downloads must reuse it.
        let (socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut reader = BufReader::new(socket);
        for index in 0..3 {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            assert_eq!(
                line.trim(),
                format!("GET /{index}?signature=private-signature HTTP/1.1")
            );
            loop {
                line.clear();
                assert!(reader.read_line(&mut line).unwrap() > 0);
                if line == "\r\n" {
                    break;
                }
                let (name, _) = line.split_once(':').unwrap();
                assert!(!["authorization", "cookie", "origin"]
                    .contains(&name.to_ascii_lowercase().as_str()));
            }
            write!(reader.get_mut(), "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nETag: \"file-etag\"\r\nConnection: keep-alive\r\n\r\n{BYTES}", BYTES.len()).unwrap();
            reader.get_mut().flush().unwrap();
        }
    });
    let server = Server::start(vec![
        list((0..3).map(object).collect(), None, None),
        links(&[0, 1, 2], &origin),
    ]);
    let mut api = Api::new(server.origin.clone(), MemoryStore::with_token(TOKEN)).unwrap();
    let result = api
        .pull(
            PROJECT,
            "2026-09-08",
            "2026-09-08",
            &Directory::default().0,
            1,
        )
        .unwrap();
    assert_eq!(result.file_count, 3);
    handle.join().unwrap();
    server.finish();
}

#[test]
fn signing_is_batched_at_twenty_keys() {
    let storage = Server::start((0..21).map(download).collect());
    let first: Vec<_> = (0..20).collect();
    let api_server = Server::start(vec![
        list((0..21).map(object).collect(), None, None),
        links(&first, &storage.origin),
        links(&[20], &storage.origin),
    ]);
    let mut api = Api::new(api_server.origin.clone(), MemoryStore::with_token(TOKEN)).unwrap();
    assert_eq!(
        api.pull(
            PROJECT,
            "2026-09-08",
            "2026-09-08",
            &Directory::default().0,
            1
        )
        .unwrap()
        .file_count,
        21
    );
    storage.finish();
    api_server.finish();
}

#[test]
fn long_object_keys_split_signing_requests_at_the_body_limit() {
    let objects: Vec<_> = (0..20)
        .map(|i| {
            let mut value = object(i);
            value["key"] = json!(format!(
                "logs/project={PROJECT}/date=2026-09-08/{}-{i}.parquet",
                "x".repeat(900)
            ));
            value
        })
        .collect();
    let storage = Server::start((0..20).map(download).collect());
    let mut steps = vec![list(objects.clone(), None, None)];
    // These keys fit sixteen, but not seventeen, in the API's 16 KiB body.
    for range in [0..16, 16..20] {
        let offset = range.start;
        let keys: Vec<_> = objects[range]
            .iter()
            .map(|object| object["key"].clone())
            .collect();
        let replies: Vec<_> = keys.iter().enumerate().map(|(i, key)| json!({
            "key": key,
            "url": storage.origin.join(&format!("/{}?signature=private-signature", offset + i)).unwrap().as_str(),
            "expires_at": "2100-01-01T00:00:00Z"
        })).collect();
        steps.push(
            Step::json(
                "POST",
                &format!("/data/v1/projects/{PROJECT}/download-urls"),
                Some(TOKEN),
                200,
                json!({"data": replies}),
            )
            .body(json!({"keys": keys})),
        );
    }
    assert_eq!(steps.len(), 3);
    let server = Server::start(steps);
    let mut api = Api::new(server.origin.clone(), MemoryStore::with_token(TOKEN)).unwrap();
    assert_eq!(
        api.pull(
            PROJECT,
            "2026-09-08",
            "2026-09-08",
            &Directory::default().0,
            1
        )
        .unwrap()
        .file_count,
        20
    );
    storage.finish();
    server.finish();
}

#[test]
fn expired_link_is_resigned_before_any_storage_request() {
    let storage = Server::start(vec![download(0)]);
    let stale = Step::json(
        "POST",
        &format!("/data/v1/projects/{PROJECT}/download-urls"),
        Some(TOKEN),
        200,
        json!({"data": [{
            "key": key(0), "url": "https://expired.example.invalid/private-link",
            "expires_at": "2000-01-01T00:00:00Z"
        }]}),
    )
    .body(json!({"keys": [key(0)]}));
    let server = Server::start(vec![
        list(vec![object(0)], None, None),
        stale,
        links(&[0], &storage.origin),
    ]);
    let mut api = Api::new(server.origin.clone(), MemoryStore::with_token(TOKEN)).unwrap();
    assert_eq!(
        api.pull(
            PROJECT,
            "2026-09-08",
            "2026-09-08",
            &Directory::default().0,
            1
        )
        .unwrap()
        .file_count,
        1
    );
    storage.finish();
    server.finish();
}

#[test]
fn empty_result_creates_empty_manifest_without_signing_requests() {
    let server = Server::start(vec![list(vec![], None, None)]);
    let mut api = Api::new(server.origin.clone(), MemoryStore::with_token(TOKEN)).unwrap();
    let output = Directory::default();
    let result = api
        .pull(PROJECT, "2026-09-08", "2026-09-08", &output.0, 1)
        .unwrap();
    assert_eq!(result.file_count, 0);
    assert_eq!(result.total_bytes, 0);
    assert!(result.manifest_path.exists());
    server.finish();
}

#[test]
fn expired_download_access_is_refreshed_once_without_losing_session() {
    let storage = Server::start(vec![
        Step::json(
            "GET",
            "/0?signature=private-signature",
            None,
            403,
            json!({"error":"private-signature"}),
        ),
        download(0),
    ]);
    let server = Server::start(vec![
        list(vec![object(0)], None, None),
        links(&[0], &storage.origin),
        links(&[0], &storage.origin),
    ]);
    let store = MemoryStore::with_token(TOKEN);
    let mut api = Api::new(server.origin.clone(), store.clone()).unwrap();
    assert_eq!(
        api.pull(
            PROJECT,
            "2026-09-08",
            "2026-09-08",
            &Directory::default().0,
            1
        )
        .unwrap()
        .file_count,
        1
    );
    assert!(store.read().unwrap().is_some());
    storage.finish();
    server.finish();
}

#[test]
fn failed_second_file_does_not_publish_manifest_or_leave_partial_file() {
    let mut bad = download(1);
    bad.response = "this-is-not-par1!".into();
    let storage = Server::start(vec![download(0), bad]);
    let server = Server::start(vec![
        list(vec![object(0), object(1)], None, None),
        links(&[0, 1], &storage.origin),
    ]);
    let mut api = Api::new(server.origin.clone(), MemoryStore::with_token(TOKEN)).unwrap();
    let output = Directory::default();
    let error = api
        .pull(PROJECT, "2026-09-08", "2026-09-08", &output.0, 1)
        .err()
        .unwrap();
    assert_eq!(error.code, "INVALID_PARQUET");
    let run = std::fs::read_dir(&output.0)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert!(run.join("000000.parquet").exists());
    assert!(!run.join("manifest.json").exists());
    assert!(!run.join("000001.parquet.part").exists());
    assert!(!run.join("000001.parquet").exists());
    storage.finish();
    server.finish();
}

#[test]
fn object_change_and_redirect_are_not_accepted_as_downloads() {
    for redirect in [false, true] {
        let mut object_reply = download(0);
        if redirect {
            object_reply.status = 302;
            object_reply.headers.push((
                "Location".into(),
                "https://example.test/?private-signature".into(),
            ));
        } else {
            object_reply.headers = vec![("ETag".into(), "different".into())];
        }
        let storage = Server::start(vec![object_reply]);
        let server = Server::start(vec![
            list(vec![object(0)], None, None),
            links(&[0], &storage.origin),
        ]);
        let store = MemoryStore::with_token(TOKEN);
        let mut api = Api::new(server.origin.clone(), store.clone()).unwrap();
        let error = api
            .pull(
                PROJECT,
                "2026-09-08",
                "2026-09-08",
                &Directory::default().0,
                1,
            )
            .err()
            .unwrap();
        assert_eq!(
            error.code,
            if redirect {
                "DOWNLOAD_FAILED"
            } else {
                "OBJECT_CHANGED"
            }
        );
        assert!(!error.message.contains("private-signature"));
        assert!(store.read().unwrap().is_some());
        storage.finish();
        server.finish();
    }
}

#[test]
fn foreign_project_listing_and_duplicate_cursor_are_rejected() {
    let mut other = object(0);
    other["key"] = json!("logs/project=prj_other/date=2026-09-08/a.parquet");
    let server = Server::start(vec![list(vec![other], None, None)]);
    let mut api = Api::new(server.origin.clone(), MemoryStore::with_token(TOKEN)).unwrap();
    assert_eq!(
        api.pull(
            PROJECT,
            "2026-09-08",
            "2026-09-08",
            &Directory::default().0,
            1
        )
        .err()
        .unwrap()
        .code,
        "INVALID_RESPONSE"
    );
    server.finish();
    let server = Server::start(vec![
        list(vec![], Some("same"), None),
        list(vec![], Some("same"), Some("same")),
    ]);
    let mut api = Api::new(server.origin.clone(), MemoryStore::with_token(TOKEN)).unwrap();
    assert_eq!(
        api.pull(
            PROJECT,
            "2026-09-08",
            "2026-09-08",
            &Directory::default().0,
            1
        )
        .err()
        .unwrap()
        .code,
        "INVALID_PAGINATION"
    );
    server.finish();
}
