use std::{
    ffi::OsString,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, anyhow};

pub(super) fn write_file_atomic(path: &Path, contents: &[u8]) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create config directory {}", parent.display()))?;
    }

    let mut last_error = None;
    for attempt in 0..16 {
        let temp_path = temp_path_for(path, attempt)?;
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
        {
            Ok(mut file) => {
                let result = (|| {
                    file.write_all(contents).with_context(|| {
                        format!("failed to write temporary config {}", temp_path.display())
                    })?;
                    file.sync_all().with_context(|| {
                        format!("failed to sync temporary config {}", temp_path.display())
                    })?;
                    drop(file);
                    fs::rename(&temp_path, path).with_context(|| {
                        format!(
                            "failed to replace config {} with {}",
                            path.display(),
                            temp_path.display()
                        )
                    })
                })();
                if result.is_err() {
                    let _ = fs::remove_file(&temp_path);
                }
                return result;
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                last_error = Some(err);
            }
            Err(err) => {
                return Err(err).with_context(|| {
                    format!("failed to create temporary config {}", temp_path.display())
                });
            }
        }
    }

    Err(anyhow!(
        "failed to allocate temporary config path for {}{}",
        path.display(),
        last_error.map_or_else(String::new, |err| format!(": {err}"))
    ))
}

fn temp_path_for(path: &Path, attempt: u8) -> Result<PathBuf> {
    let file_name = path
        .file_name()
        .ok_or_else(|| anyhow!("config path has no file name: {}", path.display()))?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();

    let mut temp_name = OsString::from(".");
    temp_name.push(file_name);
    temp_name.push(format!(".{}.{}.{}.tmp", process::id(), nonce, attempt));
    Ok(path.with_file_name(temp_name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn atomic_write_creates_parent_directory_and_writes_contents() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nested").join("config.toml");

        write_file_atomic(&path, b"model_provider = \"openai\"\n").unwrap();

        assert_eq!(
            fs::read_to_string(path).unwrap(),
            "model_provider = \"openai\"\n"
        );
    }

    #[test]
    fn atomic_write_replaces_existing_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "old = true\n").unwrap();

        write_file_atomic(&path, b"new = true\n").unwrap();

        assert_eq!(fs::read_to_string(path).unwrap(), "new = true\n");
    }
}
