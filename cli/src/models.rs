use serde::{Deserialize, Serialize};

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
    pub data_prefix: String,
    pub created_at: String,
    pub updated_at: String,
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
