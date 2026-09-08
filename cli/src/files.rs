use crate::error::{Error, Result};
use serde_json::Value;
use std::{fs::File, io::Read, path::Path};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

pub fn text(path: &Path, limit: usize) -> Result<String> {
    let file = File::open(path)
        .map_err(|_| Error::new("FILE_ERROR", format!("Could not read {}.", path.display())))?;
    let mut bytes = Vec::new();
    file.take(limit as u64 + 4)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::new("FILE_ERROR", "Could not finish reading the input file."))?;
    if bytes.starts_with(b"\xef\xbb\xbf") {
        bytes.drain(..3);
    }
    if bytes.len() > limit {
        return Err(Error::new(
            "PAYLOAD_TOO_LARGE",
            format!("Input file exceeds {limit} bytes."),
        ));
    }
    String::from_utf8(bytes).map_err(|_| Error::new("INVALID_INPUT", "Input files must be UTF-8."))
}

pub fn json(path: &Path, limit: usize) -> Result<Value> {
    serde_json::from_str(&text(path, limit)?)
        .map_err(|_| Error::new("INVALID_INPUT", "Input file is not valid JSON."))
}

pub fn now() -> String {
    let now = OffsetDateTime::now_utc().replace_nanosecond(0).unwrap();
    now.format(&Rfc3339).unwrap()
}
