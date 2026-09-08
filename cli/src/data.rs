use crate::{
    client::{valid_project_id, Api},
    config,
    credentials::CredentialStore,
    error::{Error, Result},
    files,
    models::{Data, Page},
};
use reqwest::{blocking::Client, redirect::Policy, Method, Url};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::mpsc,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use time::{
    format_description::well_known::Rfc3339, macros::format_description, Date, OffsetDateTime,
};

#[derive(Clone, Deserialize, Serialize)]
pub struct LogFile {
    pub key: String,
    pub size: u64,
    pub etag: String,
    pub uploaded_at: String,
    pub ingest_date: String,
}

#[derive(Deserialize)]
struct DownloadLink {
    key: String,
    url: String,
    expires_at: String,
}

#[derive(Serialize)]
pub struct ManifestFile {
    #[serde(flatten)]
    pub object: LogFile,
    /// Relative to the directory containing manifest.json.
    pub path: String,
}

#[derive(Serialize)]
pub struct Manifest {
    pub schema_version: u8,
    pub api_url: String,
    pub project_id: String,
    pub start_date: String,
    pub end_date: String,
    pub completed_at: String,
    pub files: Vec<ManifestFile>,
}

#[derive(Serialize)]
pub struct PullSummary {
    pub project_id: String,
    pub manifest_path: PathBuf,
    pub file_count: usize,
    pub total_bytes: u64,
}

struct DownloadClients {
    direct: Client,
    proxied: Client,
}

impl DownloadClients {
    fn new() -> Result<Self> {
        Ok(Self {
            direct: download_client(true)?,
            proxied: download_client(false)?,
        })
    }
}

pub fn validate_dates(from: &str, to: &str) -> Result<()> {
    let format = format_description!("[year]-[month]-[day]");
    let parse = |value: &str| {
        Date::parse(value, format)
            .ok()
            .filter(|date| date.to_string() == value)
    };
    let (Some(start), Some(end)) = (parse(from), parse(to)) else {
        return Err(Error::new(
            "INVALID_DATE_RANGE",
            "Use valid UTC ingestion dates in YYYY-MM-DD format.",
        ));
    };
    if end < start || (end - start).whole_days() >= 93 {
        return Err(Error::new(
            "INVALID_DATE_RANGE",
            "The inclusive date range must contain 1 to 93 days.",
        ));
    }
    Ok(())
}

impl<S: CredentialStore> Api<S> {
    pub fn pull(
        &mut self,
        project: &str,
        from: &str,
        to: &str,
        output: &Path,
        concurrency: u8,
    ) -> Result<PullSummary> {
        if !(1..=8).contains(&concurrency) {
            return Err(Error::new(
                "INVALID_INPUT",
                "Concurrency must be between 1 and 8.",
            ));
        }
        if !valid_project_id(project) {
            return Err(Error::new(
                "INVALID_PROJECT_ID",
                "Expected prj_ followed by a UUID.",
            ));
        }
        validate_dates(from, to)?;
        let objects = self.log_files(project, from, to)?;
        fs::create_dir_all(output)
            .map_err(|_| file_error("Could not create the download directory."))?;
        let output = output
            .canonicalize()
            .map_err(|_| file_error("Could not resolve the download directory."))?;
        let run = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| file_error("Invalid system clock."))?
            .as_nanos();
        let directory = output.join(format!("pull-{run}-{}", std::process::id()));
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder
            .create(&directory)
            .map_err(|_| file_error("Could not create a unique pull directory."))?;
        let result = self.pull_into(project, (from, to), &objects, &directory, concurrency);
        result.map_err(|mut error| {
            error.message.push_str(&format!(
                " Incomplete pull: {}. No completed manifest was published.",
                directory.display()
            ));
            error
        })
    }

    fn log_files(&mut self, project: &str, from: &str, to: &str) -> Result<Vec<LogFile>> {
        let mut result = Vec::new();
        let mut cursor = None;
        let mut cursors = HashSet::new();
        let mut keys = HashSet::new();
        loop {
            let mut query = vec![
                ("start_date".into(), from.into()),
                ("end_date".into(), to.into()),
                ("limit".into(), "1000".into()),
            ];
            if let Some(cursor) = cursor {
                query.push(("cursor".into(), cursor));
            }
            let page = self
                .authenticated(
                    Method::GET,
                    &format!("/data/v1/projects/{project}/files"),
                    &query,
                    None,
                )?
                .decode::<Page<LogFile>>()?;
            for file in page.data {
                if file.ingest_date.as_str() < from
                    || file.ingest_date.as_str() > to
                    || !file.key.starts_with(&format!(
                        "logs/project={project}/date={}/",
                        file.ingest_date
                    ))
                    || !file.key.ends_with(".parquet")
                    || file.key.len() > 1024
                    || !keys.insert(file.key.clone())
                {
                    return Err(Error::invalid_response());
                }
                result.push(file);
            }
            cursor = page.pagination.next_cursor;
            match &cursor {
                Some(value) if !cursors.insert(value.clone()) => {
                    return Err(Error::new(
                        "INVALID_PAGINATION",
                        "The file API repeated a cursor.",
                    ))
                }
                Some(_) => {}
                None => return Ok(result),
            }
        }
    }

    fn download_links(
        &mut self,
        project: &str,
        keys: &[&str],
    ) -> Result<HashMap<String, DownloadLink>> {
        let links = self
            .authenticated(
                Method::POST,
                &format!("/data/v1/projects/{project}/download-urls"),
                &[],
                Some(&json!({"keys": keys})),
            )?
            .decode::<Data<Vec<DownloadLink>>>()?
            .data;
        let mut result = HashMap::new();
        for link in links {
            if !keys.contains(&link.key.as_str()) || result.contains_key(&link.key) {
                return Err(Error::invalid_response());
            }
            result.insert(link.key.clone(), link);
        }
        if result.len() != keys.len() {
            return Err(Error::invalid_response());
        }
        Ok(result)
    }

    fn pull_into(
        &mut self,
        project: &str,
        (from, to): (&str, &str),
        objects: &[LogFile],
        directory: &Path,
        concurrency: u8,
    ) -> Result<PullSummary> {
        let clients = DownloadClients::new()?;
        let mut manifest = Manifest {
            schema_version: 1,
            api_url: self.origin.origin().ascii_serialization(),
            project_id: project.into(),
            start_date: from.into(),
            end_date: to.into(),
            completed_at: String::new(),
            files: vec![],
        };
        let mut offset = 0;
        let mut total_bytes = 0u64;
        while offset < objects.len() {
            let mut keys = Vec::new();
            for file in objects.iter().skip(offset).take(20) {
                keys.push(file.key.as_str());
                if serde_json::to_vec(&json!({"keys": keys})).unwrap().len() > 16 * 1024 {
                    keys.pop();
                    break;
                }
            }
            if keys.is_empty() {
                return Err(Error::invalid_response());
            }
            let mut links = self.download_links(project, &keys)?;
            let batch = &objects[offset..offset + keys.len()];
            let local_api = config::is_loopback(&self.origin);
            // Keep API/keyring operations on this thread; workers only fetch storage bytes.
            thread::scope(|scope| -> Result<()> {
                let (sender, receiver) = mpsc::channel();
                let mut pending: VecDeque<_> = (0..batch.len()).map(|i| (i, false)).collect();
                let mut active = 0;
                let mut completed = 0;
                while !pending.is_empty() || active > 0 {
                    while active < concurrency && !pending.is_empty() {
                        let (index, retry) = pending.pop_front().unwrap();
                        let file = &batch[index];
                        let mut link = links
                            .remove(&file.key)
                            .ok_or_else(Error::invalid_response)?;
                        let expires = OffsetDateTime::parse(&link.expires_at, &Rfc3339)
                            .map_err(|_| Error::invalid_response())?;
                        if retry
                            || expires <= OffsetDateTime::now_utc() + time::Duration::seconds(30)
                        {
                            link = self
                                .download_links(project, &[&file.key])?
                                .remove(&file.key)
                                .ok_or_else(Error::invalid_response)?;
                        }
                        let path = directory.join(format!("{:06}.parquet", offset + index));
                        let url = link.url.clone();
                        links.insert(file.key.clone(), link);
                        let sender = sender.clone();
                        let clients = &clients;
                        thread::Builder::new()
                            .spawn_scoped(scope, move || {
                                // Report even a worker panic so the coordinator cannot wait forever.
                                let result =
                                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                        download(clients, &url, file, &path, local_api)
                                    }))
                                    .unwrap_or_else(|_| {
                                        Err(Error::new(
                                            "DOWNLOAD_FAILED",
                                            "The download worker panicked.",
                                        ))
                                    });
                                let _ = sender.send((index, retry, result));
                            })
                            .map_err(|_| {
                                Error::new("DOWNLOAD_FAILED", "Could not start a download worker.")
                            })?;
                        active += 1;
                    }
                    let first = receiver.recv().map_err(|_| {
                        Error::new(
                            "DOWNLOAD_FAILED",
                            "The download worker stopped unexpectedly.",
                        )
                    })?;
                    // Observe queued failures before filling newly available slots.
                    for (index, retry, result) in std::iter::once(first).chain(receiver.try_iter())
                    {
                        active -= 1;
                        if !retry && result.as_ref().is_err_and(|e| e.code == "DOWNLOAD_DENIED") {
                            pending.push_front((index, true));
                        } else {
                            result?;
                            completed += 1;
                            eprintln!(
                                "Downloaded {} / {} files",
                                offset + completed,
                                objects.len()
                            );
                        }
                    }
                }
                Ok(())
            })?;
            for (index, file) in batch.iter().enumerate() {
                total_bytes = total_bytes
                    .checked_add(file.size)
                    .ok_or_else(Error::invalid_response)?;
                manifest.files.push(ManifestFile {
                    object: file.clone(),
                    path: format!("{:06}.parquet", offset + index),
                });
            }
            offset += keys.len();
        }
        manifest.completed_at = files::now();
        let temporary = directory.join("manifest.json.part");
        let mut file = private_file(&temporary)?;
        serde_json::to_writer_pretty(&mut file, &manifest)
            .map_err(|_| file_error("Could not write the manifest."))?;
        file.write_all(b"\n")
            .and_then(|_| file.sync_all())
            .map_err(|_| file_error("Could not finish the manifest."))?;
        drop(file);
        let manifest_path = directory.join("manifest.json");
        fs::rename(temporary, &manifest_path)
            .map_err(|_| file_error("Could not publish the manifest."))?;
        Ok(PullSummary {
            project_id: project.into(),
            manifest_path,
            file_count: objects.len(),
            total_bytes,
        })
    }
}

fn private_file(path: &Path) -> Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
        .open(path)
        .map_err(|_| file_error("Could not create a new download file."))
}

fn file_error(message: &str) -> Error {
    Error::new("FILE_ERROR", message)
}

fn download(
    clients: &DownloadClients,
    signed_url: &str,
    object: &LogFile,
    path: &Path,
    local_api: bool,
) -> Result<()> {
    let url = Url::parse(signed_url).map_err(|_| Error::invalid_response())?;
    if !config::safe_transport(&url)
        || url.fragment().is_some()
        || (url.scheme() != "https" && !(local_api && config::is_loopback(&url)))
    {
        return Err(Error::new(
            "INVALID_DOWNLOAD_URL",
            "The API returned an unsupported download destination.",
        ));
    }
    // These shared pools never receive the user's Session or API headers.
    let client = if config::is_loopback(&url) {
        &clients.direct
    } else {
        &clients.proxied
    };
    let response = client
        .get(url)
        .send()
        .map_err(|_| Error::new("DOWNLOAD_FAILED", "The file download request failed."))?;
    if response.status().as_u16() == 403 {
        return Err(Error::new(
            "DOWNLOAD_DENIED",
            "The download link expired or access was denied.",
        ));
    }
    if response.status().as_u16() != 200 {
        return Err(Error::new(
            "DOWNLOAD_FAILED",
            format!(
                "File download returned HTTP {}. No file was accepted.",
                response.status().as_u16()
            ),
        ));
    }
    let etag = response
        .headers()
        .get("etag")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.trim_matches('"'));
    if etag != Some(object.etag.as_str())
        || response.content_length().is_some_and(|n| n != object.size)
    {
        return Err(Error::new(
            "OBJECT_CHANGED",
            "The object no longer matches the file listing. Run data pull again.",
        ));
    }
    let temporary = path.with_extension("parquet.part");
    let result = (|| {
        let mut file = private_file(&temporary)?;
        let size = std::io::copy(&mut response.take(object.size.saturating_add(1)), &mut file)
            .map_err(|_| Error::new("DOWNLOAD_FAILED", "The file download was interrupted."))?;
        if size != object.size || size < 12 {
            return Err(Error::new(
                "DOWNLOAD_FAILED",
                "The downloaded file has an unexpected size.",
            ));
        }
        file.sync_all()
            .map_err(|_| file_error("Could not flush the downloaded file."))?;
        drop(file);
        let mut file = File::open(&temporary)
            .map_err(|_| file_error("Could not inspect the downloaded file."))?;
        let mut magic = [0; 4];
        file.read_exact(&mut magic)
            .map_err(|_| file_error("Could not inspect the file header."))?;
        if &magic != b"PAR1" {
            return Err(Error::new(
                "INVALID_PARQUET",
                "The downloaded file does not have a Parquet header.",
            ));
        }
        file.seek(SeekFrom::End(-4))
            .and_then(|_| file.read_exact(&mut magic))
            .map_err(|_| file_error("Could not inspect the file footer."))?;
        if &magic != b"PAR1" {
            return Err(Error::new(
                "INVALID_PARQUET",
                "The downloaded file does not have a Parquet footer.",
            ));
        }
        drop(file);
        fs::rename(&temporary, path)
            .map_err(|_| file_error("Could not finish the downloaded file."))
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

fn download_client(direct: bool) -> Result<Client> {
    let mut builder = Client::builder()
        .redirect(Policy::none())
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(300));
    if direct {
        builder = builder.no_proxy();
    }
    builder.build().map_err(|_| {
        Error::new(
            "HTTP_CLIENT_ERROR",
            "Could not initialize the download client.",
        )
    })
}
