use crate::error::{Error, Result};
use directories::BaseDirs;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub api_url: Option<String>,
}

pub fn config_dir(explicit: Option<PathBuf>) -> Result<PathBuf> {
    explicit
        .or_else(|| BaseDirs::new().map(|dirs| dirs.config_dir().join("logcove")))
        .ok_or_else(|| {
            Error::new(
                "CONFIG_ERROR",
                "Cannot locate the OS configuration directory. Set --config-dir.",
            )
        })
}

pub fn load(directory: &Path) -> Result<Config> {
    let raw = match fs::read(directory.join("config.json")) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Config::default()),
        Err(_) => return Err(Error::new("CONFIG_ERROR", "Could not read config.json.")),
    };
    serde_json::from_slice(&raw).map_err(|_| {
        Error::new(
            "CONFIG_ERROR",
            "config.json must contain a valid API configuration.",
        )
    })
}

pub fn save(directory: &Path, origin: &Url) -> Result<()> {
    fs::create_dir_all(directory).map_err(|_| {
        Error::new(
            "CONFIG_ERROR",
            "Could not create the configuration directory.",
        )
    })?;
    let config = Config {
        api_url: Some(origin.origin().ascii_serialization()),
    };
    fs::write(
        directory.join("config.json"),
        serde_json::to_string_pretty(&config).unwrap() + "\n",
    )
    .map_err(|_| Error::new("CONFIG_ERROR", "Could not save config.json."))
}

pub fn origin(value: &str) -> Result<Url> {
    let invalid = || {
        Error::new("INVALID_API_URL", "Use an HTTPS origin (or loopback HTTP), without credentials, a path, query, or fragment.")
    };
    let url = Url::parse(value).map_err(|_| invalid())?;
    if !safe_transport(&url)
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(invalid());
    }
    Ok(url)
}

pub fn safe_transport(url: &Url) -> bool {
    url.host().is_some()
        && url.username().is_empty()
        && url.password().is_none()
        && (url.scheme() == "https" || (url.scheme() == "http" && is_loopback(url)))
}

pub fn is_loopback(url: &Url) -> bool {
    url.host_str().is_some_and(|host| {
        host == "localhost"
            || host
                .trim_matches(['[', ']'])
                .parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_loopback())
    })
}

pub fn resolve(override_url: Option<&str>, saved: &Config) -> Result<Url> {
    origin(override_url.or(saved.api_url.as_deref()).ok_or_else(|| {
        Error::new(
            "API_NOT_CONFIGURED",
            "Run logcove config set-api-url <origin>, or set --api-url / LOGCOVE_API_URL.",
        )
    })?)
}
