use crate::error::{Error, Result};
use std::{
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
};

pub(crate) struct SecretFile {
    file: Option<File>,
    path: PathBuf,
    saved: bool,
}

impl SecretFile {
    pub fn create(path: &Path) -> Result<Self> {
        let absolute = std::path::absolute(path).map_err(|_| file_error())?;
        let parent = absolute
            .parent()
            .ok_or_else(file_error)?
            .canonicalize()
            .map_err(|_| file_error())?;
        let path = parent.join(absolute.file_name().ok_or_else(file_error)?);
        if path.to_str().is_none() {
            return Err(file_error());
        }
        #[cfg(windows)]
        if path.file_name().unwrap().to_string_lossy().contains(':') {
            return Err(file_error());
        }
        let file = create_private(&path).map_err(|_| file_error())?;
        Ok(Self {
            file: Some(file),
            path,
            saved: false,
        })
    }

    pub fn save(&mut self, secret: &str) -> io::Result<PathBuf> {
        let file = self.file.as_mut().unwrap();
        file.write_all(secret.as_bytes())?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        self.saved = true;
        self.file.take();
        Ok(self.path.clone())
    }
}

impl Drop for SecretFile {
    fn drop(&mut self) {
        self.file.take();
        if !self.saved {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn file_error() -> Error {
    Error::new("KEY_FILE_ERROR", "Could not create the private Key file. Use --output with a new file in an existing writable directory; existing files and symlinks are never overwritten.")
}

#[cfg(unix)]
fn create_private(path: &Path) -> io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
}

#[cfg(windows)]
fn create_private(path: &Path) -> io::Result<File> {
    use std::{
        os::windows::{ffi::OsStrExt, io::FromRawHandle},
        ptr,
    };
    use windows_sys::Win32::{
        Foundation::{LocalFree, GENERIC_WRITE, INVALID_HANDLE_VALUE},
        Security::{
            Authorization::{
                ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
            },
            SECURITY_ATTRIBUTES,
        },
        Storage::FileSystem::{CreateFileW, CREATE_NEW, FILE_ATTRIBUTE_NORMAL},
    };
    let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    if path[..path.len() - 1].contains(&0) {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let mut descriptor = ptr::null_mut();
    // A protected DACL grants access only to the file owner from creation onward.
    unsafe {
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            windows_sys::core::w!("D:P(A;;FA;;;OW)"),
            SDDL_REVISION_1,
            &mut descriptor,
            ptr::null_mut(),
        ) == 0
        {
            return Err(io::Error::last_os_error());
        }
        let attributes = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor,
            bInheritHandle: 0,
        };
        let handle = CreateFileW(
            path.as_ptr(),
            GENERIC_WRITE,
            0,
            &attributes,
            CREATE_NEW,
            FILE_ATTRIBUTE_NORMAL,
            ptr::null_mut(),
        );
        let error = io::Error::last_os_error();
        LocalFree(descriptor);
        if handle == INVALID_HANDLE_VALUE {
            return Err(error);
        }
        Ok(File::from_raw_handle(handle))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn save_failure_is_reported_without_exposing_the_secret() {
        let path = std::env::temp_dir().join(format!(
            "logcove-key-io-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut file = SecretFile::create(&path).unwrap();
        file.file.take();
        file.file = Some(File::open(&path).unwrap());
        assert!(file.save("fake-credential").is_err());
        drop(file);
        assert!(!path.exists());
    }

    #[cfg(windows)]
    #[test]
    fn windows_secret_has_a_protected_owner_only_dacl() {
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
        let path = std::env::temp_dir().join(format!(
            "logcove-key-acl-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut file = SecretFile::create(&path).unwrap();
        file.save("fake-credential").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "fake-credential\n");
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
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
                    &mut descriptor
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
        fs::remove_file(path).unwrap();
    }
}
