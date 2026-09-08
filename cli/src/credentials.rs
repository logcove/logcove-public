use crate::error::{Error, Result};
use keyring::{Entry, Error as KeyringError};
use std::fs::{self, File, OpenOptions};

pub const SERVICE: &str = "com.logcove.cli.session";

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

// All CLI writers share this per-user lock, including across config directories.
fn write_lock() -> Result<File> {
    let directory = directories::BaseDirs::new()
        .ok_or_else(|| storage_error("lock"))?
        .data_local_dir()
        .join("logcove");
    fs::create_dir_all(&directory).map_err(|_| storage_error("lock"))?;
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options
        .open(directory.join("credentials.lock"))
        .map_err(|_| storage_error("lock"))?;
    file.lock().map_err(|_| storage_error("lock"))?;
    Ok(file)
}

pub struct OsCredentials(Entry);

impl OsCredentials {
    pub fn new(origin: &str) -> Result<Self> {
        Entry::new(SERVICE, origin)
            .map(Self)
            .map_err(|_| storage_error("open"))
    }
}

fn storage_error(operation: &str) -> Error {
    Error::new("CREDENTIAL_STORE_ERROR", format!(
        "Could not {operation} the CLI credential in the system store. Allow access and unlock the store. Linux requires a running Secret Service (such as GNOME Keyring or KWallet). No plaintext fallback is used."))
}

impl CredentialStore for OsCredentials {
    fn read(&self) -> Result<Option<String>> {
        match self.0.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(KeyringError::NoEntry) => Ok(None),
            Err(_) => Err(storage_error("read")),
        }
    }

    fn save(&self, token: &str) -> Result<()> {
        let _lock = write_lock()?;
        self.0
            .set_password(token)
            .map_err(|_| storage_error("save"))
    }

    fn delete(&self) -> Result<()> {
        let _lock = write_lock()?;
        self.delete_entry()
    }

    fn delete_if_matches(&self, expected: &str) -> Result<()> {
        let _lock = write_lock()?;
        if self.read()?.as_deref() == Some(expected) {
            self.delete_entry()?;
        }
        Ok(())
    }
}

impl OsCredentials {
    fn delete_entry(&self) -> Result<()> {
        match self.0.delete_credential() {
            Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
            Err(_) => Err(storage_error("delete")),
        }
    }
}
