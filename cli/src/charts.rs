use crate::{
    client::{valid_id, valid_project_id, Api},
    credentials::CredentialStore,
    error::{Error, Result},
    files,
    models::{Data, Page},
};
use clap::{Args, Subcommand};
use reqwest::Method;
use serde_json::{json, Map, Value};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};
use time::{format_description::well_known::Rfc3339, OffsetDateTime, UtcOffset};

const SQL_LIMIT: usize = 64 * 1024;
const SPEC_LIMIT: usize = 128 * 1024;
const RESULT_LIMIT: usize = 5 * 1024 * 1024;
const REQUEST_LIMIT: usize = 6 * 1024 * 1024;

#[derive(Subcommand)]
pub enum ChartCommand {
    /// List chart summaries, optionally filtered by a Project metadata tag
    List {
        #[arg(long)]
        project_id: Option<String>,
    },
    /// Get the definition and latest result
    Get { id: String },
    /// Save a definition, optionally with its first computed result
    Create(CreateArgs),
    /// Update a definition using its last observed revision
    Update(UpdateArgs),
    /// Delete a chart and its latest result
    Delete { id: String },
    Result {
        #[command(subcommand)]
        command: ResultCommand,
    },
}

#[derive(Subcommand)]
pub enum ResultCommand {
    Put {
        id: String,
        #[arg(long)]
        file: PathBuf,
    },
}

#[derive(Args)]
pub struct CreateArgs {
    #[arg(long)]
    pub name: String,
    #[arg(long)]
    pub description: Option<String>,
    #[arg(long = "project-id")]
    pub project_ids: Vec<String>,
    #[arg(long)]
    pub sql_file: PathBuf,
    #[arg(long)]
    pub spec_file: PathBuf,
    /// JSON object containing computed_at and data
    #[arg(long)]
    pub result_file: Option<PathBuf>,
}

#[derive(Args)]
pub struct UpdateArgs {
    pub id: String,
    #[arg(long)]
    pub revision: u64,
    #[arg(long)]
    pub name: Option<String>,
    #[arg(long, conflicts_with = "clear_description")]
    pub description: Option<String>,
    #[arg(long)]
    pub clear_description: bool,
    #[arg(long = "project-id", conflicts_with = "clear_projects")]
    pub project_ids: Vec<String>,
    #[arg(long)]
    pub clear_projects: bool,
    #[arg(long)]
    pub sql_file: Option<PathBuf>,
    #[arg(long)]
    pub spec_file: Option<PathBuf>,
}

fn invalid(message: &str) -> Error {
    Error::new("INVALID_INPUT", message)
}

fn validate_project_tags(ids: &[String]) -> Result<()> {
    let mut seen = HashSet::new();
    if ids.len() > 100
        || ids
            .iter()
            .any(|id| !valid_project_id(id) || !seen.insert(id))
    {
        return Err(invalid(
            "Project tags must be at most 100 unique Project IDs.",
        ));
    }
    Ok(())
}

fn name(value: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > 100 {
        return Err(invalid("Chart name must contain 1 to 100 characters."));
    }
    Ok(value.into())
}

fn description(value: &str) -> Result<Value> {
    if value.chars().count() > 2000 {
        return Err(invalid("Description exceeds 2000 characters."));
    }
    Ok(json!(value))
}

fn sql(path: &Path) -> Result<Value> {
    let value = files::text(path, SQL_LIMIT)?;
    if value.trim().is_empty() {
        return Err(invalid("SQL must not be empty."));
    }
    Ok(json!(value))
}

fn spec(path: &Path) -> Result<Value> {
    let value = files::json(path, SPEC_LIMIT)?;
    if !value.is_object() || value.get("data") != Some(&json!({"name":"result"})) {
        return Err(invalid(
            "Vega-Lite must be an object with root data set to {\"name\":\"result\"}.",
        ));
    }
    // The API owns complete Vega-Lite data-source validation, including nested charts.
    Ok(value)
}

pub fn result_file(path: &Path) -> Result<Value> {
    let mut value = files::json(path, REQUEST_LIMIT)?;
    let object = value
        .as_object()
        .ok_or_else(|| invalid("Result must be an object containing computed_at and data."))?;
    if object.len() != 2 {
        return Err(invalid("Result accepts only computed_at and data."));
    }
    let computed = object
        .get("computed_at")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("Result requires computed_at."))?;
    let timestamp = OffsetDateTime::parse(computed, &Rfc3339)
        .map_err(|_| invalid("computed_at must be an ISO timestamp with a timezone."))?;
    if timestamp > OffsetDateTime::now_utc() + time::Duration::minutes(5) {
        return Err(invalid(
            "computed_at is more than five minutes in the future.",
        ));
    }
    let rows = object
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("Result data must be an array of JSON objects."))?;
    if rows.len() > 10000 || rows.iter().any(|row| !row.is_object()) {
        return Err(invalid(
            "Result data must contain at most 10000 JSON objects.",
        ));
    }
    if serde_json::to_vec(rows).unwrap().len() > RESULT_LIMIT {
        return Err(invalid("Result data exceeds 5 MiB."));
    }
    // The API accepts millisecond precision; Python commonly emits microseconds.
    value["computed_at"] = json!(timestamp
        .to_offset(UtcOffset::UTC)
        .replace_nanosecond(u32::from(timestamp.millisecond()) * 1_000_000)
        .unwrap()
        .format(&Rfc3339)
        .map_err(|_| invalid("computed_at is outside the supported timestamp range."))?);
    Ok(value)
}

pub fn create_body(args: &CreateArgs) -> Result<Value> {
    validate_project_tags(&args.project_ids)?;
    let mut body = json!({"name":name(&args.name)?, "sql":sql(&args.sql_file)?,
        "vega_lite_spec":spec(&args.spec_file)?, "project_ids":args.project_ids});
    if let Some(value) = &args.description {
        body["description"] = description(value)?;
    }
    if let Some(file) = &args.result_file {
        body["result"] = result_file(file)?;
    }
    if serde_json::to_vec(&body).unwrap().len() > REQUEST_LIMIT {
        return Err(invalid("Chart creation exceeds 6 MiB."));
    }
    Ok(body)
}

pub fn update_body(args: &UpdateArgs) -> Result<Value> {
    if args.revision == 0 || args.revision > 9_007_199_254_740_991 {
        return Err(invalid(
            "revision must be a positive JavaScript-safe integer.",
        ));
    }
    let mut body = Map::new();
    body.insert("revision".into(), json!(args.revision));
    if let Some(value) = &args.name {
        body.insert("name".into(), json!(name(value)?));
    }
    if let Some(value) = &args.description {
        body.insert("description".into(), description(value)?);
    }
    if args.clear_description {
        body.insert("description".into(), Value::Null);
    }
    if !args.project_ids.is_empty() || args.clear_projects {
        validate_project_tags(&args.project_ids)?;
        body.insert("project_ids".into(), json!(args.project_ids));
    }
    if let Some(path) = &args.sql_file {
        body.insert("sql".into(), sql(path)?);
    }
    if let Some(path) = &args.spec_file {
        body.insert("vega_lite_spec".into(), spec(path)?);
    }
    if body.len() == 1 {
        return Err(invalid("Supply at least one definition field to update."));
    }
    let value = Value::Object(body);
    if serde_json::to_vec(&value).unwrap().len() > 256 * 1024 {
        return Err(invalid("Chart update exceeds 256 KiB."));
    }
    Ok(value)
}

impl<S: CredentialStore> Api<S> {
    pub fn charts(&mut self, project: Option<&str>) -> Result<Vec<Value>> {
        if project.is_some_and(|id| !valid_project_id(id)) {
            return Err(invalid("Expected a Project ID for the chart tag filter."));
        }
        let mut result = Vec::new();
        let mut cursor = None;
        let mut seen = HashSet::new();
        loop {
            let mut query = vec![("limit".into(), "100".into())];
            if let Some(project) = project {
                query.push(("project_id".into(), project.into()));
            }
            if let Some(cursor) = cursor {
                query.push(("cursor".into(), cursor));
            }
            let page = self
                .authenticated(Method::GET, "/api/v1/charts", &query, None)?
                .decode::<Page<Value>>()?;
            result.extend(page.data);
            cursor = page.pagination.next_cursor;
            match &cursor {
                Some(value) if !seen.insert(value.clone()) => {
                    return Err(Error::new(
                        "INVALID_PAGINATION",
                        "The Chart API repeated a cursor.",
                    ))
                }
                Some(_) => {}
                None => return Ok(result),
            }
        }
    }

    pub fn chart(&mut self, id: &str) -> Result<Value> {
        self.authenticated(Method::GET, &chart_path(id)?, &[], None)?
            .decode::<Data<Value>>()
            .map(|r| r.data)
    }

    pub fn create_chart(&mut self, args: &CreateArgs) -> Result<Value> {
        self.authenticated(
            Method::POST,
            "/api/v1/charts",
            &[],
            Some(&create_body(args)?),
        )?
        .decode::<Data<Value>>()
        .map(|r| r.data)
    }

    pub fn update_chart(&mut self, args: &UpdateArgs) -> Result<Value> {
        self.authenticated(
            Method::PATCH,
            &chart_path(&args.id)?,
            &[],
            Some(&update_body(args)?),
        )?
        .decode::<Data<Value>>()
        .map(|r| r.data)
    }

    pub fn delete_chart(&mut self, id: &str) -> Result<()> {
        let reply = self.authenticated(Method::DELETE, &chart_path(id)?, &[], None)?;
        if !reply.success() {
            return Err(reply.error());
        }
        if reply.status != 204 {
            return Err(Error::invalid_response());
        }
        Ok(())
    }

    pub fn put_chart_result(&mut self, id: &str, path: &Path) -> Result<Value> {
        self.authenticated(
            Method::PUT,
            &format!("{}/result", chart_path(id)?),
            &[],
            Some(&result_file(path)?),
        )?
        .decode::<Data<Value>>()
        .map(|r| r.data)
    }

    pub fn chart_command(&mut self, command: ChartCommand) -> Result<Value> {
        let data = match command {
            ChartCommand::List { project_id } => json!(self.charts(project_id.as_deref())?),
            ChartCommand::Get { id } => self.chart(&id)?,
            ChartCommand::Create(args) => self.create_chart(&args)?,
            ChartCommand::Update(args) => self.update_chart(&args)?,
            ChartCommand::Delete { id } => {
                self.delete_chart(&id)?;
                json!({"id": id, "deleted":true})
            }
            ChartCommand::Result {
                command: ResultCommand::Put { id, file },
            } => self.put_chart_result(&id, &file)?,
        };
        Ok(json!({"data": data}))
    }
}

fn chart_path(id: &str) -> Result<String> {
    if !valid_id(id, "chart_") {
        return Err(invalid("Expected chart_ followed by a UUID."));
    }
    Ok(format!("/api/v1/charts/{id}"))
}
