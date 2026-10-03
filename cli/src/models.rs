use clap::ValueEnum;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Default, Deserialize, Serialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
#[value(rename_all = "snake_case")]
pub enum IngestionProtocol {
    #[default]
    HttpJson,
    OtlpHttp,
}

#[derive(Deserialize, Serialize)]
pub struct Data<T> {
    pub data: T,
}

#[derive(Deserialize, Serialize)]
pub struct User {
    pub id: String,
    pub email: String,
    pub email_verified: bool,
    pub name: String,
    pub image: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Deserialize, Serialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub status: String,
    // Older API responses predate OTLP support and describe JSON-only Projects.
    #[serde(default)]
    pub ingestion_protocol: IngestionProtocol,
    pub data_prefix: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<ProjectUsage>,
}

#[derive(Deserialize, Serialize)]
pub struct ProjectUsage {
    pub raw_bytes: u64,
    pub start_date: String,
    pub end_date: String,
    pub timezone: String,
    pub granularity: String,
    pub tracking_started_at: String,
    pub updated_at: Option<String>,
}

#[derive(Deserialize)]
pub struct Page<T> {
    pub data: Vec<T>,
    pub pagination: Pagination,
}

#[derive(Deserialize)]
pub struct Pagination {
    pub next_cursor: Option<String>,
}
