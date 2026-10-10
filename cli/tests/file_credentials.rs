mod support;

use logcove::credentials::{CredentialStore, FileCredentials};
use std::{fs, path::Path, process::Command};
use support::Directory;

const ORIGIN: &str = "https://api.example.test";

fn store(directory: &Path, origin: &str) -> FileCredentials {
    FileCredentials::in_directory(directory.to_path_buf(), origin).unwrap()
}

#[test]
fn missing_store_is_logged_out_without_creating_files() {
    let root = Directory::default();
    let directory = root.0.join("credentials");
    assert!(store(&directory, ORIGIN).read().unwrap().is_none());
    assert!(!directory.exists());
}

#[test]
fn origins_are_isolated_and_stale_deletion_preserves_new_login() {
    let root = Directory::default();
    let first = store(&root.0, "https://API.example.test:443/");
    let same = store(&root.0, ORIGIN);
    let other = store(&root.0, "https://api.example.test:8443");
    first.save("initial.signature").unwrap();
    other.save("other.signature").unwrap();
    assert_eq!(same.read().unwrap().as_deref(), Some("initial.signature"));
    same.save("new.signature").unwrap();
    first.delete_if_matches("initial.signature").unwrap();
    assert_eq!(same.read().unwrap().as_deref(), Some("new.signature"));
    first.delete_if_matches("new.signature").unwrap();
    assert!(same.read().unwrap().is_none());
    assert_eq!(other.read().unwrap().as_deref(), Some("other.signature"));
    other.delete().unwrap();
    other.delete().unwrap();
    assert!(other.read().unwrap().is_none());
}

#[test]
fn invalid_store_is_not_silently_overwritten_or_exposed() {
    for invalid in ["{private-broken-token", "{}", r#"{"sessions":null}"#] {
        let root = Directory::default();
        let path = root.0.join("auth.json");
        fs::write(&path, invalid).unwrap();
        let credentials = store(&root.0, ORIGIN);
        for error in [
            credentials.read().unwrap_err(),
            credentials.save("replacement.signature").unwrap_err(),
            credentials.delete().unwrap_err(),
            credentials.delete_if_matches("old.signature").unwrap_err(),
        ] {
            assert_eq!(error.code, "CREDENTIAL_STORE_ERROR");
            assert!(!error.message.contains(invalid));
            assert!(!error.message.contains("replacement.signature"));
        }
        assert_eq!(fs::read_to_string(path).unwrap(), invalid);
    }
}

#[test]
fn interrupted_or_failed_writes_preserve_the_last_complete_store() {
    let root = Directory::default();
    let credentials = store(&root.0, ORIGIN);
    credentials.save("initial.signature").unwrap();
    let path = root.0.join("auth.json");
    let previous = fs::read(&path).unwrap();
    let temporary = root.0.join("auth.json.tmp");
    fs::create_dir(&temporary).unwrap();
    assert!(credentials.save("failed.signature").is_err());
    assert_eq!(fs::read(&path).unwrap(), previous);
    fs::remove_dir(&temporary).unwrap();
    fs::write(&temporary, "interrupted-write").unwrap();
    credentials.save("updated.signature").unwrap();
    assert_eq!(
        credentials.read().unwrap().as_deref(),
        Some("updated.signature")
    );
    assert!(!temporary.exists());
}

#[test]
fn file_credential_persistence_across_processes() {
    const DIRECTORY: &str = "LOGCOVE_TEST_CREDENTIAL_DIR";
    const PHASE: &str = "LOGCOVE_TEST_CREDENTIAL_PHASE";
    if let Some(directory) = std::env::var_os(DIRECTORY) {
        let credentials = store(Path::new(&directory), ORIGIN);
        match std::env::var(PHASE).unwrap().as_str() {
            "restore" => {
                assert_eq!(
                    credentials.read().unwrap().as_deref(),
                    Some("initial.signature")
                );
                credentials.save("updated.signature").unwrap();
            }
            "delete" => {
                credentials.delete_if_matches("initial.signature").unwrap();
                assert_eq!(
                    credentials.read().unwrap().as_deref(),
                    Some("updated.signature")
                );
                credentials.delete_if_matches("updated.signature").unwrap();
            }
            phase => {
                let index: usize = phase.parse().unwrap();
                let own = store(
                    Path::new(&directory),
                    &format!("https://writer-{index}.example.test"),
                );
                for revision in 0..20 {
                    let token = format!("writer-{index}-{revision}.signature");
                    own.save(&token).unwrap();
                    assert_eq!(own.read().unwrap().as_deref(), Some(token.as_str()));
                }
            }
        }
        return;
    }
    let root = Directory::default();
    let credentials = store(&root.0, ORIGIN);
    credentials.save("initial.signature").unwrap();
    let command = |phase: &str| {
        let mut child = Command::new(std::env::current_exe().unwrap());
        child
            .args(["--exact", "file_credential_persistence_across_processes"])
            .env(DIRECTORY, &root.0)
            .env(PHASE, phase);
        child
    };
    for phase in ["restore", "delete"] {
        assert!(command(phase).output().unwrap().status.success());
    }
    assert!(credentials.read().unwrap().is_none());
    let children: Vec<_> = (0..8)
        .map(|index| {
            command(&index.to_string())
                .stdout(std::process::Stdio::null())
                .spawn()
                .unwrap()
        })
        .collect();
    for mut child in children {
        assert!(child.wait().unwrap().success());
    }
    for index in 0..8 {
        let own = store(&root.0, &format!("https://writer-{index}.example.test"));
        assert_eq!(
            own.read().unwrap(),
            Some(format!("writer-{index}-19.signature"))
        );
    }
    assert!(!fs::read_to_string(root.0.join("credentials.lock"))
        .unwrap()
        .contains("signature"));
}

#[cfg(unix)]
#[test]
fn unix_permissions_are_private_after_creation_and_replacement() {
    use std::os::unix::fs::PermissionsExt;
    let root = Directory::default();
    let directory = root.0.join("credentials");
    let credentials = store(&directory, ORIGIN);
    for token in ["initial.signature", "updated.signature"] {
        credentials.save(token).unwrap();
        for (path, expected) in [
            (directory.clone(), 0o700),
            (directory.join("auth.json"), 0o600),
            (directory.join("credentials.lock"), 0o600),
        ] {
            assert_eq!(
                fs::metadata(path).unwrap().permissions().mode() & 0o777,
                expected
            );
        }
    }
}

#[cfg(unix)]
#[test]
fn credential_symlinks_are_rejected_without_touching_the_target() {
    use std::os::unix::fs::symlink;
    let root = Directory::default();
    let target = root.write("target", "private-target");
    for name in ["auth.json", "credentials.lock"] {
        let directory = root.0.join(name.replace('.', "-"));
        fs::create_dir(&directory).unwrap();
        symlink(&target, directory.join(name)).unwrap();
        let credentials = store(&directory, ORIGIN);
        assert!(credentials.read().is_err());
        assert!(credentials.save("new.signature").is_err());
        assert_eq!(fs::read_to_string(&target).unwrap(), "private-target");
    }
    let alias = root.0.join("alias");
    symlink(&root.0, &alias).unwrap();
    assert!(store(&alias, ORIGIN).read().is_err());
    assert!(store(&alias, ORIGIN).save("new.signature").is_err());
}

#[cfg(windows)]
#[test]
fn windows_credentials_keep_owner_only_acl_after_replacement() {
    use std::{os::windows::ffi::OsStrExt, ptr};
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::{
            Authorization::{
                ConvertSecurityDescriptorToStringSecurityDescriptorW, GetNamedSecurityInfoW,
                SDDL_REVISION_1, SE_FILE_OBJECT,
            },
            DACL_SECURITY_INFORMATION,
        },
    };
    let root = Directory::default();
    let credentials = store(&root.0, ORIGIN);
    for token in ["initial.signature", "updated.signature"] {
        credentials.save(token).unwrap();
        assert_eq!(credentials.read().unwrap().as_deref(), Some(token));
        let wide: Vec<_> = root
            .0
            .join("auth.json")
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect();
        let mut descriptor = ptr::null_mut();
        let mut text = ptr::null_mut();
        let mut length = 0;
        unsafe {
            assert_eq!(
                GetNamedSecurityInfoW(
                    wide.as_ptr(),
                    SE_FILE_OBJECT,
                    DACL_SECURITY_INFORMATION,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    &mut descriptor,
                ),
                0
            );
            let converted = ConvertSecurityDescriptorToStringSecurityDescriptorW(
                descriptor,
                SDDL_REVISION_1,
                DACL_SECURITY_INFORMATION,
                &mut text,
                &mut length,
            );
            LocalFree(descriptor);
            assert_ne!(converted, 0);
            let sddl =
                String::from_utf16_lossy(std::slice::from_raw_parts(text, length as usize - 1));
            LocalFree(text.cast());
            assert!(sddl.starts_with("D:P"), "{sddl}");
            assert!(sddl.contains(";;;OW)"), "{sddl}");
            assert_eq!(sddl.matches("(A;").count(), 1, "{sddl}");
        }
    }
}
