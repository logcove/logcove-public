use crate::{
    config,
    error::{Error, Result},
    secret_file::create_private,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

pub trait CredentialStore {
    fn read(&self) -> Result<Option<String>>;
    fn save(&self, token: &str) -> Result<()>;
    fn delete(&self) -> Result<()>;
    fn delete_if_matches(&self, expected: &str) -> Result<()> {
        if self.read()?.as_deref() == Some(expected) {
            self.delete()?;
        }
        Ok(())
    }
}

#[derive(Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Credentials {
    sessions: BTreeMap<String, String>,
}

pub struct FileCredentials {
    directory: PathBuf,
    origin: String,
}

impl FileCredentials {
    // Construction performs no I/O; environment PATs never access saved Sessions.
    pub fn new(origin: &str) -> Result<Self> {
        let directory = directories::BaseDirs::new()
            .ok_or_else(|| storage_error("locate"))?
            .home_dir()
            .join(".logcove");
        Self::in_directory(directory, origin)
    }

    pub fn in_directory(directory: PathBuf, origin: &str) -> Result<Self> {
        Ok(Self {
            directory,
            origin: config::origin(origin)?.origin().ascii_serialization(),
        })
    }

    fn lock(&self) -> Result<File> {
        let operation = || -> io::Result<File> {
            check_path(&self.directory, true)?;
            let builder = fs::DirBuilder::new();
            #[cfg(unix)]
            let builder = {
                use std::os::unix::fs::DirBuilderExt;
                let mut builder = builder;
                builder.mode(0o700);
                builder
            };
            match builder.create(&self.directory) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
            check_path(&self.directory, true)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&self.directory, fs::Permissions::from_mode(0o700))?;
            }
            let path = self.directory.join("credentials.lock");
            check_path(&path, false)?;
            let mut options = OpenOptions::new();
            options.read(true).write(true).create(true).truncate(false);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let file = options.open(path)?;
            file.lock()?;
            Ok(file)
        };
        operation().map_err(|_| storage_error("lock"))
    }

    fn load(&self) -> Result<Credentials> {
        let path = self.directory.join("auth.json");
        check_path(&path, false).map_err(|_| storage_error("read"))?;
        match fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|_| storage_error("parse")),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Credentials::default()),
            Err(_) => Err(storage_error("read")),
        }
    }

    // The caller holds the lock across loading, changing and replacing the map.
    fn persist(&self, credentials: &Credentials) -> Result<()> {
        let path = self.directory.join("auth.json");
        let temporary = self.directory.join("auth.json.tmp");
        let operation = || -> io::Result<()> {
            check_path(&path, false)?;
            // A previous process may have exited before renaming its private file.
            match fs::remove_file(&temporary) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
            let mut file = create_private(&temporary)?;
            serde_json::to_writer(&mut file, credentials)?;
            file.write_all(b"\n")?;
            file.sync_all()?;
            drop(file);
            fs::rename(&temporary, &path)
        };
        let result = operation();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result.map_err(|_| storage_error("save"))
    }
}

impl CredentialStore for FileCredentials {
    fn read(&self) -> Result<Option<String>> {
        if !check_path(&self.directory, true).map_err(|_| storage_error("read"))? {
            return Ok(None);
        }
        let _lock = self.lock()?;
        Ok(self.load()?.sessions.remove(&self.origin))
    }

    fn save(&self, token: &str) -> Result<()> {
        let _lock = self.lock()?;
        let mut credentials = self.load()?;
        credentials
            .sessions
            .insert(self.origin.clone(), token.into());
        self.persist(&credentials)
    }

    fn delete(&self) -> Result<()> {
        let _lock = self.lock()?;
        let mut credentials = self.load()?;
        if credentials.sessions.remove(&self.origin).is_some() {
            self.persist(&credentials)?;
        }
        Ok(())
    }

    fn delete_if_matches(&self, expected: &str) -> Result<()> {
        let _lock = self.lock()?;
        let mut credentials = self.load()?;
        if credentials.sessions.get(&self.origin).map(String::as_str) == Some(expected) {
            credentials.sessions.remove(&self.origin);
            self.persist(&credentials)?;
        }
        Ok(())
    }
}

fn check_path(path: &Path, directory: bool) -> io::Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(metadata)
            if !metadata.file_type().is_symlink()
                && (if directory {
                    metadata.is_dir()
                } else {
                    metadata.is_file()
                }) =>
        {
            Ok(true)
        }
        Ok(_) => Err(io::Error::other("Expected a regular credential path")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

fn storage_error(operation: &str) -> Error {
    Error::new("CREDENTIAL_STORE_ERROR", format!(
        "Could not {operation} the CLI session file at ~/.logcove/auth.json. Check directory permissions and file integrity. Credential contents are never included in this error."))
}
