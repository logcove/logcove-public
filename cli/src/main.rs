use clap::{Parser, Subcommand};
use logcove::{
    auth::SystemClock,
    charts::ChartCommand,
    client::Api,
    config,
    credentials::OsCredentials,
    error::Result,
    management::{KeyCommand, ProjectCommand},
    skills::{self, SkillCommand},
};
use serde_json::{json, Value};
use std::{
    io::{self, Write},
    path::PathBuf,
};

#[derive(Parser)]
#[command(
    name = "logcove",
    version,
    about = "Access Logcove from your local analysis workflow"
)]
struct Cli {
    /// API origin; overrides the environment and saved configuration
    #[arg(long, env = "LOGCOVE_API_URL", global = true)]
    api_url: Option<String>,
    /// Directory for non-secret configuration (credentials remain in the OS store)
    #[arg(long, env = "LOGCOVE_CONFIG_DIR", global = true)]
    config_dir: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Install the bundled analysis Skill for a coding agent
    Skills {
        #[command(subcommand)]
        command: SkillCommand,
    },
    /// Inspect or configure the API environment
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    /// Authorize this CLI in your browser
    Login {
        #[arg(long, help = "Print the authorization link without opening a browser")]
        no_browser: bool,
    },
    /// Show the current account
    Whoami,
    /// Revoke this CLI session and remove its saved credential
    Logout,
    /// Create and manage Project data sources
    Projects {
        #[command(subcommand)]
        command: ProjectCommand,
    },
    /// Manage write keys; created secrets are saved to a private local file
    Keys {
        #[command(subcommand)]
        command: KeyCommand,
    },
    /// Download log datasets
    Data {
        #[command(subcommand)]
        command: DataCommand,
    },
    /// Manage chart definitions for local computation
    Charts {
        #[command(subcommand)]
        command: ChartCommand,
    },
}

#[derive(Subcommand)]
enum DataCommand {
    /// Download Parquet for an inclusive UTC ingestion-date range
    Pull {
        project_id: String,
        #[arg(long)]
        from: String,
        #[arg(long)]
        to: String,
        #[arg(long)]
        output: PathBuf,
        /// Maximum simultaneous file downloads
        #[arg(long, default_value_t = 4, value_parser = clap::value_parser!(u8).range(1..=8))]
        concurrency: u8,
        /// Reuse matching local files from a completed manifest (repeatable)
        #[arg(long = "reuse-manifest")]
        reuse_manifests: Vec<PathBuf>,
    },
}

#[derive(Subcommand)]
enum ConfigCommand {
    Show,
    SetApiUrl { origin: String },
}

fn run(cli: Cli) -> Result<Value> {
    if let Command::Skills { command } = &cli.command {
        return skills::run(command);
    }
    let directory = config::config_dir(cli.config_dir)?;
    if let Command::Config {
        command: ConfigCommand::SetApiUrl { origin },
    } = &cli.command
    {
        let url = config::origin(origin)?;
        config::save(&directory, &url)?;
        return Ok(json!({"data": {"api_url": url.origin().ascii_serialization()}}));
    }
    let saved = config::load(&directory)?;
    if matches!(
        cli.command,
        Command::Config {
            command: ConfigCommand::Show
        }
    ) {
        let effective = config::resolve(cli.api_url.as_deref(), &saved)?
            .origin()
            .ascii_serialization();
        return Ok(
            json!({"data": {"api_url": effective, "config_file": directory.join("config.json")}}),
        );
    }
    let origin = config::resolve(cli.api_url.as_deref(), &saved)?;
    let credentials = OsCredentials::new(&origin.origin().ascii_serialization())?;
    let mut api = Api::new(origin, credentials)?;
    match cli.command {
        Command::Login { no_browser } => {
            if api.has_session() {
                match api.whoami() {
                    Ok(user) => {
                        eprintln!(
                            "Already signed in. Use logout before authorizing another account."
                        );
                        return Ok(json!({"data": user}));
                    }
                    Err(e) if e.code == "UNAUTHENTICATED" => {}
                    Err(e) => return Err(e),
                }
            }
            let authorization = api.begin_login()?;
            let mut clock = SystemClock::default();
            eprintln!("Open this link in your browser:\n{}\nVerification code: {}\nWaiting for browser approval...",
                authorization.verification_uri_complete, authorization.user_code);
            if !no_browser && open::that(&authorization.verification_uri_complete).is_err() {
                eprintln!("Could not open the browser automatically. Open the link manually.");
            }
            api.complete_login(&authorization, &mut clock)?;
            Ok(json!({"data": api.whoami()?}))
        }
        Command::Whoami => Ok(json!({"data": api.whoami()?})),
        Command::Logout => {
            api.logout()?;
            Ok(json!({"data": {"signed_out": true}}))
        }
        Command::Projects { command } => api.project_command(command),
        Command::Keys { command } => api.key_command(command),
        Command::Data {
            command:
                DataCommand::Pull {
                    project_id,
                    from,
                    to,
                    output,
                    concurrency,
                    reuse_manifests,
                },
        } => Ok(
            json!({"data":api.pull_with_reuse(&project_id, (&from, &to), &output, concurrency, &reuse_manifests)?}),
        ),
        Command::Charts { command } => api.chart_command(command),
        Command::Config { .. } | Command::Skills { .. } => unreachable!(),
    }
}

fn main() {
    match run(Cli::parse()) {
        Ok(value) => {
            let mut stdout = io::stdout().lock();
            if serde_json::to_writer(&mut stdout, &value).is_err() || writeln!(stdout).is_err() {
                std::process::exit(1);
            }
        }
        Err(error) => {
            eprintln!("{}", json!({"error": error}));
            std::process::exit(1);
        }
    }
}
