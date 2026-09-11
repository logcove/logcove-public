use crate::{
    client::{valid_id, Api},
    credentials::CredentialStore,
    error::{Error, Result},
    models::{Data, IngestionProtocol, Page, Project},
    secret_file::SecretFile,
};
use clap::{Args, Subcommand, ValueEnum};
use reqwest::Method;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::{collections::HashSet, path::PathBuf};

#[derive(Clone, Copy, ValueEnum, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ProjectStatus {
    Active,
    Archived,
    All,
}
#[derive(Clone, Copy, ValueEnum, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum KeyStatus {
    Active,
    Revoked,
    All,
}

#[derive(Subcommand)]
pub enum ProjectCommand {
    /// List Projects, following every page; defaults to active sources
    List {
        #[arg(long, value_enum, default_value = "active")]
        status: ProjectStatus,
    },
    /// Show metadata, write-key binding and desired ingestion revision
    Get { id: String },
    /// Create an active Project without a write key
    Create {
        #[arg(long)]
        name: String,
        #[arg(long)]
        description: Option<String>,
        /// Immutable ingestion format for this Project
        #[arg(long, value_enum, default_value = "http_json")]
        ingestion_protocol: IngestionProtocol,
    },
    /// Edit metadata or atomically replace/clear the write-key binding
    Update(UpdateProject),
    /// Archive a Project; data is retained but access is disabled
    Archive { id: String },
    /// Restore an archived Project
    Restore { id: String },
}

#[derive(Args)]
pub struct UpdateProject {
    pub id: String,
    #[arg(long)]
    pub name: Option<String>,
    #[arg(long, conflicts_with = "clear_description")]
    pub description: Option<String>,
    #[arg(long)]
    pub clear_description: bool,
    /// Key resource ID, not the plaintext write credential
    #[arg(long, conflicts_with = "clear_write_key")]
    pub write_key_id: Option<String>,
    #[arg(long)]
    pub clear_write_key: bool,
}

#[derive(Subcommand)]
pub enum KeyCommand {
    /// List masked Key metadata, including revoked keys by default
    List {
        #[arg(long, value_enum, default_value = "all")]
        status: KeyStatus,
        #[arg(long)]
        project_id: Option<String>,
    },
    /// Show masked Key metadata and bindings; cannot recover the secret
    Get { id: String },
    /// Create a Key and save its secret to a new private file, never stdout
    Create(CreateKey),
    /// Rename a Key without changing its secret or bindings
    Update {
        id: String,
        #[arg(long)]
        name: String,
    },
    /// Replace the complete binding set; does not steal another Key's Projects
    SetProjects(SetKeyProjects),
    /// Irreversibly revoke a Key and remove all its bindings
    #[command(visible_alias = "delete")]
    Revoke { id: String },
}

#[derive(Args)]
pub struct CreateKey {
    #[arg(long)]
    pub name: String,
    #[arg(long = "project-id")]
    pub project_ids: Vec<String>,
    /// New file for the plaintext Key; parent must exist, never overwritten
    #[arg(long)]
    pub output: PathBuf,
}

#[derive(Args)]
#[group(skip)]
pub struct SetKeyProjects {
    pub id: String,
    #[arg(
        long = "project-id",
        required_unless_present = "clear_projects",
        conflicts_with = "clear_projects"
    )]
    pub project_ids: Vec<String>,
    #[arg(long)]
    pub clear_projects: bool,
}

#[derive(Deserialize, Serialize)]
pub struct ManagedProject {
    #[serde(flatten)]
    pub project: Project,
    pub write_key_id: Option<String>,
    pub ingestion: Ingestion,
}
#[derive(Deserialize, Serialize)]
pub struct Ingestion {
    pub desired_revision: u64,
}

// Only allow public metadata into command output, even on the create path.
#[derive(Deserialize, Serialize)]
pub struct KeyMetadata {
    pub id: String,
    pub name: String,
    pub key_prefix: String,
    pub masked_key: String,
    pub project_ids: Vec<String>,
    pub revoked_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Deserialize)]
struct CreatedKey {
    #[serde(flatten)]
    metadata: KeyMetadata,
    key: String,
}

fn invalid(message: &str) -> Error {
    Error::new("INVALID_INPUT", message)
}
fn path(id: &str, prefix: &str, resource: &str) -> Result<String> {
    if !valid_id(id, prefix) {
        return Err(invalid("Expected a resource ID followed by a UUID."));
    }
    Ok(format!("/api/v1/{resource}/{id}"))
}
fn name(value: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() || value.encode_utf16().count() > 100 {
        return Err(invalid("Name must contain 1 to 100 characters."));
    }
    Ok(value.into())
}
fn description(value: &str) -> Result<Value> {
    if value.encode_utf16().count() > 2000 {
        return Err(invalid("Description exceeds 2000 characters."));
    }
    Ok(json!(value))
}
fn project_ids(ids: &[String]) -> Result<()> {
    let mut seen = HashSet::new();
    if ids.len() > 100
        || ids
            .iter()
            .any(|id| !valid_id(id, "prj_") || !seen.insert(id))
    {
        return Err(invalid("Provide at most 100 unique Project IDs."));
    }
    Ok(())
}
fn payload(value: Value) -> Result<Value> {
    if serde_json::to_vec(&value).unwrap().len() > 16 * 1024 {
        return Err(invalid("Management payload exceeds 16 KiB."));
    }
    Ok(value)
}

impl<S: CredentialStore> Api<S> {
    fn management<T: DeserializeOwned>(
        &mut self,
        method: Method,
        path: &str,
        body: Option<Value>,
    ) -> Result<T> {
        let body = body.map(payload).transpose()?;
        Ok(self
            .authenticated(method, path, &[], body.as_ref())?
            .decode::<Data<T>>()?
            .data)
    }

    fn management_list<T: DeserializeOwned>(
        &mut self,
        path: &str,
        filters: Vec<(String, String)>,
    ) -> Result<Vec<T>> {
        let mut rows = Vec::new();
        let mut cursor = None;
        let mut seen = HashSet::new();
        loop {
            let mut query = filters.clone();
            query.push(("limit".into(), "100".into()));
            if let Some(cursor) = cursor {
                query.push(("cursor".into(), cursor));
            }
            let page = self
                .authenticated(Method::GET, path, &query, None)?
                .decode::<Page<T>>()?;
            rows.extend(page.data);
            cursor = page.pagination.next_cursor;
            match &cursor {
                Some(value) if !seen.insert(value.clone()) => {
                    return Err(Error::new(
                        "INVALID_PAGINATION",
                        "The management API repeated a cursor.",
                    ))
                }
                Some(_) => {}
                None => return Ok(rows),
            }
        }
    }

    pub fn project_command(&mut self, command: ProjectCommand) -> Result<Value> {
        let project: ManagedProject = match command {
            ProjectCommand::List { status } => {
                let filters = match status {
                    ProjectStatus::All => vec![],
                    _ => vec![("status".into(), json!(status).as_str().unwrap().into())],
                };
                return Ok(
                    json!({"data": self.management_list::<ManagedProject>("/api/v1/projects", filters)?}),
                );
            }
            ProjectCommand::Get { id } => {
                self.management(Method::GET, &path(&id, "prj_", "projects")?, None)?
            }
            ProjectCommand::Create {
                name: value,
                description: desc,
                ingestion_protocol,
            } => {
                let mut body =
                    json!({"name": name(&value)?, "ingestion_protocol": ingestion_protocol});
                if let Some(desc) = desc {
                    body["description"] = description(&desc)?;
                }
                self.management(Method::POST, "/api/v1/projects", Some(body))?
            }
            ProjectCommand::Update(args) => {
                let mut body = Map::new();
                if let Some(value) = args.name {
                    body.insert("name".into(), json!(name(&value)?));
                }
                if let Some(value) = args.description {
                    body.insert("description".into(), description(&value)?);
                }
                if args.clear_description {
                    body.insert("description".into(), Value::Null);
                }
                if let Some(id) = args.write_key_id {
                    path(&id, "key_", "keys")?;
                    body.insert("write_key_id".into(), json!(id));
                }
                if args.clear_write_key {
                    body.insert("write_key_id".into(), Value::Null);
                }
                if body.is_empty() {
                    return Err(invalid("Provide at least one Project field to update."));
                }
                self.management(
                    Method::PATCH,
                    &path(&args.id, "prj_", "projects")?,
                    Some(Value::Object(body)),
                )?
            }
            ProjectCommand::Archive { id } => self.management(
                Method::PATCH,
                &path(&id, "prj_", "projects")?,
                Some(json!({"status":"archived"})),
            )?,
            ProjectCommand::Restore { id } => self.management(
                Method::PATCH,
                &path(&id, "prj_", "projects")?,
                Some(json!({"status":"active"})),
            )?,
        };
        Ok(json!({"data": project}))
    }

    pub fn create_key(&mut self, args: CreateKey) -> Result<Value> {
        project_ids(&args.project_ids)?;
        let body = payload(json!({"name":name(&args.name)?, "project_ids":args.project_ids}))?;
        // Reserve the destination before creating a once-only server credential.
        let mut file = SecretFile::create(&args.output)?;
        let created: CreatedKey = self.management(Method::POST, "/api/v1/keys", Some(body)).map_err(|mut error| {
            if !matches!(error.code, "UNAUTHENTICATED" | "INVALID_INPUT" | "ACCESS_DENIED" | "NOT_FOUND" | "CONFLICT" | "PAYLOAD_TOO_LARGE") {
                error.message.push_str(" Key creation may have reached the server. Inspect logcove keys list before retrying; the secret cannot be retrieved again.");
            }
            error
        })?;
        if !valid_id(&created.metadata.id, "key_") {
            return Err(Error::invalid_response());
        }
        let id = &created.metadata.id;
        if created.key.len() != 67
            || !created.key.starts_with("lc_")
            || !created.key[3..].bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(Error::new("INVALID_RESPONSE", format!("Key {id} was created but its credential response was invalid. Run logcove keys revoke {id} before creating another key.")));
        }
        let key_file = file.save(&created.key).map_err(|_| Error::new("KEY_FILE_WRITE_FAILED", format!("Key {id} was created, but its secret could not be saved. Run logcove keys revoke {id} before creating another key. No automatic retry was attempted.")))?;
        let mut metadata = serde_json::to_value(created.metadata).unwrap();
        metadata["key_file"] = json!(key_file);
        Ok(json!({"data":metadata}))
    }

    pub fn key_command(&mut self, command: KeyCommand) -> Result<Value> {
        let key: KeyMetadata = match command {
            KeyCommand::List { status, project_id } => {
                let mut filters = match status {
                    KeyStatus::All => vec![],
                    _ => vec![("status".into(), json!(status).as_str().unwrap().into())],
                };
                if let Some(id) = project_id {
                    path(&id, "prj_", "projects")?;
                    filters.push(("project_id".into(), id));
                }
                return Ok(
                    json!({"data":self.management_list::<KeyMetadata>("/api/v1/keys", filters)?}),
                );
            }
            KeyCommand::Get { id } => {
                self.management(Method::GET, &path(&id, "key_", "keys")?, None)?
            }
            KeyCommand::Create(args) => return self.create_key(args),
            KeyCommand::Update { id, name: value } => self.management(
                Method::PATCH,
                &path(&id, "key_", "keys")?,
                Some(json!({"name":name(&value)?})),
            )?,
            KeyCommand::SetProjects(args) => {
                if args.project_ids.is_empty() && !args.clear_projects {
                    return Err(invalid("Supply --project-id or --clear-projects."));
                }
                if args.clear_projects && !args.project_ids.is_empty() {
                    return Err(invalid("Cannot combine --project-id and --clear-projects."));
                }
                project_ids(&args.project_ids)?;
                self.management(
                    Method::PUT,
                    &format!("{}/projects", path(&args.id, "key_", "keys")?),
                    Some(json!({"project_ids":args.project_ids})),
                )?
            }
            KeyCommand::Revoke { id } => {
                self.management(Method::DELETE, &path(&id, "key_", "keys")?, None)?
            }
        };
        Ok(json!({"data":key}))
    }
}
