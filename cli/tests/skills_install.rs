use logcove::skills::{install, Agent, FILES};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

static SEQUENCE: AtomicUsize = AtomicUsize::new(0);

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "logcove-skill-test-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn installs_complete_skill_for_both_agents_without_service_configuration() {
    let home = Directory::new();
    for (agent, directory) in [(Agent::Codex, ".agents"), (Agent::Claude, ".claude")] {
        let result = install(&home.0, agent, false).unwrap();
        assert_eq!(result["data"]["status"], "installed");
        let path = home.0.join(directory).join("skills/logcove");
        assert_eq!(result["data"]["path"], path.to_str().unwrap());
        assert_eq!(result["data"]["version"], env!("CARGO_PKG_VERSION"));
        for &(name, contents) in FILES {
            assert_eq!(fs::read(path.join(name)).unwrap(), contents);
        }
        assert_eq!(
            install(&home.0, agent, false).unwrap()["data"]["status"],
            "unchanged"
        );
    }
}

#[test]
fn conflicts_do_not_write_other_files_and_force_preserves_extra_files() {
    let home = Directory::new();
    install(&home.0, Agent::Codex, false).unwrap();
    let path = home.0.join(".agents/skills/logcove");
    fs::remove_file(path.join("SKILL.md")).unwrap();
    fs::write(path.join("references/charts.md"), "local customization").unwrap();
    fs::write(path.join("notes.md"), "keep this").unwrap();
    assert_eq!(
        install(&home.0, Agent::Codex, false).unwrap_err().code,
        "SKILL_INSTALL_CONFLICT"
    );
    assert!(!path.join("SKILL.md").exists());
    assert_eq!(
        fs::read_to_string(path.join("references/charts.md")).unwrap(),
        "local customization"
    );
    install(&home.0, Agent::Codex, true).unwrap();
    assert_eq!(
        fs::read_to_string(path.join("notes.md")).unwrap(),
        "keep this"
    );
    for &(name, contents) in FILES {
        assert_eq!(fs::read(path.join(name)).unwrap(), contents);
    }
}

#[test]
fn fills_missing_files_in_a_matching_manual_installation() {
    let home = Directory::new();
    let path = home.0.join(".claude/skills/logcove");
    fs::create_dir_all(&path).unwrap();
    fs::write(path.join("SKILL.md"), FILES[0].1).unwrap();
    install(&home.0, Agent::Claude, false).unwrap();
    for &(name, contents) in FILES {
        assert_eq!(fs::read(path.join(name)).unwrap(), contents);
    }
}

#[test]
fn old_version_requires_explicit_overwrite() {
    let home = Directory::new();
    install(&home.0, Agent::Claude, false).unwrap();
    let version = home.0.join(".claude/skills/logcove/VERSION");
    fs::write(&version, "0.0.1\n").unwrap();
    assert_eq!(
        install(&home.0, Agent::Claude, false).unwrap_err().code,
        "SKILL_INSTALL_CONFLICT"
    );
    install(&home.0, Agent::Claude, true).unwrap();
    assert_eq!(
        fs::read_to_string(version).unwrap().trim(),
        env!("CARGO_PKG_VERSION")
    );
}

#[test]
fn rejects_directory_in_place_of_managed_file_even_with_force() {
    let home = Directory::new();
    let path = home.0.join(".agents/skills/logcove/references/cli.md");
    fs::create_dir_all(&path).unwrap();
    assert_eq!(
        install(&home.0, Agent::Codex, true).unwrap_err().code,
        "SKILL_INSTALL_CONFLICT"
    );
    assert!(path.is_dir());
}

#[cfg(unix)]
#[test]
fn does_not_overwrite_a_symlinked_source_checkout() {
    use std::os::unix::fs::symlink;
    let home = Directory::new();
    let source = home.0.join("source");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(home.0.join(".agents/skills")).unwrap();
    symlink(&source, home.0.join(".agents/skills/logcove")).unwrap();
    assert_eq!(
        install(&home.0, Agent::Codex, true).unwrap_err().code,
        "SKILL_INSTALL_CONFLICT"
    );
    assert_eq!(fs::read_dir(&source).unwrap().count(), 0);
}

#[test]
fn cli_help_and_agent_validation_work_without_an_api() {
    for arguments in [
        vec!["skills", "--help"],
        vec!["skills", "install", "--help"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_logcove"))
            .args(arguments)
            .output()
            .unwrap();
        assert!(output.status.success());
    }
    for arguments in [
        vec!["skills", "install"],
        vec!["skills", "install", "--agent", "unknown"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_logcove"))
            .args(arguments)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
    }
}
