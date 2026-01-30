//! Internal API token management
//!
//! Provides automatic generation and file-based persistence of the internal API token.
//! The token is stored in `~/.llmlb/internal_token` with permissions 600.

use crate::config::get_env_with_fallback;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::PathBuf;
use uuid::Uuid;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

/// Default internal token file name
const INTERNAL_TOKEN_FILE: &str = "internal_token";
/// Default data directory name
const DATA_DIR: &str = ".llmlb";

/// Get or create the internal API token
///
/// Priority:
/// 1. Environment variable `LLMLB_INTERNAL_API_TOKEN` (or deprecated `INTERNAL_API_TOKEN`)
/// 2. Read from file `~/.llmlb/internal_token`
/// 3. Generate new UUIDv4 and save to file
pub fn get_or_create_internal_token() -> io::Result<String> {
    if let Some(token) = get_env_with_fallback("LLMLB_INTERNAL_API_TOKEN", "INTERNAL_API_TOKEN") {
        if !token.is_empty() {
            tracing::info!("Using internal API token from environment variable");
            return Ok(token);
        }
    }

    let token_path = get_internal_token_path()?;
    if token_path.exists() {
        let token = read_secret_file(&token_path)?;
        if !token.is_empty() {
            tracing::info!(
                "Using internal API token from file: {}",
                token_path.display()
            );
            return Ok(token);
        }
    }

    let token = generate_token();
    write_secret_file(&token_path, &token)?;
    tracing::info!(
        "Generated new internal API token and saved to: {}",
        token_path.display()
    );

    Ok(token)
}

fn get_internal_token_path() -> io::Result<PathBuf> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| io::Error::new(io::ErrorKind::NotFound, "Failed to resolve home directory"))?;

    Ok(PathBuf::from(home).join(DATA_DIR).join(INTERNAL_TOKEN_FILE))
}

fn generate_token() -> String {
    Uuid::new_v4().to_string()
}

fn read_secret_file(path: &PathBuf) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut secret = String::new();
    file.read_to_string(&mut secret)?;
    Ok(secret.trim().to_string())
}

fn write_secret_file(path: &PathBuf, secret: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)?;
    file.write_all(secret.as_bytes())?;

    #[cfg(unix)]
    {
        let metadata = file.metadata()?;
        let mut permissions = metadata.permissions();
        permissions.set_mode(0o600);
        fs::set_permissions(path, permissions)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use tempfile::tempdir;

    fn set_home(path: &PathBuf) {
        let value = path.to_string_lossy().to_string();
        std::env::set_var("HOME", &value);
        std::env::set_var("USERPROFILE", &value);
    }

    fn clear_env() {
        std::env::remove_var("LLMLB_INTERNAL_API_TOKEN");
        std::env::remove_var("INTERNAL_API_TOKEN");
    }

    #[test]
    #[serial]
    fn internal_token_uses_env_var() {
        let temp = tempdir().expect("temp dir");
        set_home(&temp.path().to_path_buf());
        clear_env();
        std::env::set_var("LLMLB_INTERNAL_API_TOKEN", "env-token");

        let token = get_or_create_internal_token().expect("token");
        assert_eq!(token, "env-token");

        std::env::remove_var("LLMLB_INTERNAL_API_TOKEN");
    }

    #[test]
    #[serial]
    fn internal_token_reads_from_file() {
        let temp = tempdir().expect("temp dir");
        set_home(&temp.path().to_path_buf());
        clear_env();

        let token_path = get_internal_token_path().expect("token path");
        write_secret_file(&token_path, "file-token").expect("write token");

        let token = get_or_create_internal_token().expect("token");
        assert_eq!(token, "file-token");
    }

    #[test]
    #[serial]
    fn internal_token_generates_and_persists() {
        let temp = tempdir().expect("temp dir");
        set_home(&temp.path().to_path_buf());
        clear_env();

        let token = get_or_create_internal_token().expect("token");
        assert!(!token.is_empty());

        let token_path = get_internal_token_path().expect("token path");
        let stored = read_secret_file(&token_path).expect("read token");
        assert_eq!(token, stored);
    }
}
