use crate::error::{Error, Result};
use clap::{Subcommand, ValueEnum};
use directories::BaseDirs;
use serde::Serialize;
use serde_json::{json, Value};
use std::{fs, io, path::Path};

pub const FILES: &[(&str, &[u8])] = &[
    ("SKILL.md", include_bytes!("../../skills/logcove/SKILL.md")),
    (
        "references/cli.md",
        include_bytes!("../../skills/logcove/references/cli.md"),
    ),
    (
        "references/duckdb.md",
        include_bytes!("../../skills/logcove/references/duckdb.md"),
    ),
    (
        "references/charts.md",
        include_bytes!("../../skills/logcove/references/charts.md"),
    ),
    ("LICENSE", include_bytes!("../../LICENSE")),
    (
        "VERSION",
        concat!(env!("CARGO_PKG_VERSION"), "\n").as_bytes(),
    ),
];

#[derive(Clone, Copy, Debug, Serialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Agent {
    Codex,
    Claude,
}

#[derive(Subcommand)]
pub enum SkillCommand {
    /// Install this CLI version's Skill into the agent's user directory
    Install {
        #[arg(long, value_enum)]
        agent: Agent,
        /// Replace bundled files when the destination contains different content
        #[arg(long)]
        force: bool,
    },
}

pub fn run(command: &SkillCommand) -> Result<Value> {
    let home = BaseDirs::new().ok_or_else(|| {
        Error::new(
            "SKILL_INSTALL_ERROR",
            "Cannot locate the user home directory.",
        )
    })?;
    match command {
        SkillCommand::Install { agent, force } => install(home.home_dir(), *agent, *force),
    }
}

fn inspect(path: &Path, directory: bool) -> Result<bool> {
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
        Ok(_) => Err(Error::new(
            "SKILL_INSTALL_CONFLICT",
            format!(
                "{} must be a regular {} (not a symlink).",
                path.display(),
                if directory { "directory" } else { "file" }
            ),
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(Error::new(
            "SKILL_INSTALL_ERROR",
            format!("Could not inspect {}.", path.display()),
        )),
    }
}

pub fn install(home: &Path, agent: Agent, force: bool) -> Result<Value> {
    let agent_directory = match agent {
        Agent::Codex => ".agents",
        Agent::Claude => ".claude",
    };
    let destination = home.join(agent_directory).join("skills/logcove");
    inspect(&destination, true)?;
    inspect(&destination.join("references"), true)?;
    let mut changed = Vec::new();
    // Check every managed file before writing, so a conflict preserves the installation.
    for &(name, contents) in FILES {
        let path = destination.join(name);
        if inspect(&path, false)? {
            let existing = fs::read(&path).map_err(|_| {
                Error::new(
                    "SKILL_INSTALL_ERROR",
                    format!("Could not read {}.", path.display()),
                )
            })?;
            if existing == contents {
                continue;
            }
            if !force {
                return Err(Error::new(
                    "SKILL_INSTALL_CONFLICT",
                    format!("{} differs from the bundled Skill. Review local changes, then use --force to replace bundled files.", path.display()),
                ));
            }
        }
        changed.push((name, contents));
    }
    if !changed.is_empty() {
        fs::create_dir_all(destination.join("references")).map_err(|_| {
            Error::new(
                "SKILL_INSTALL_ERROR",
                "Could not create the Skill directory.",
            )
        })?;
        for &(name, contents) in &changed {
            fs::write(destination.join(name), contents).map_err(|_| {
                Error::new(
                    "SKILL_INSTALL_ERROR",
                    format!(
                        "Could not write Skill file {name}. Retry the installation to complete it."
                    ),
                )
            })?;
        }
    }
    Ok(json!({"data": {
        "agent": agent,
        "path": destination,
        "version": env!("CARGO_PKG_VERSION"),
        "status": if changed.is_empty() { "unchanged" } else { "installed" },
    }}))
}
