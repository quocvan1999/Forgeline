use std::path::Path;
use std::process::Command;

use crate::validate::validate_git_url;
use crate::CoreError;

/// Clone into a directory Forgeline owns. This never uses a shell.
/// A checkout that already has `.git` is left untouched: later indexes do not
/// fetch, so analysis does not open the network again by surprise.
pub fn ensure_managed_checkout(url: &str, dest: &Path) -> Result<(), CoreError> {
    validate_git_url(url)?;
    if dest.file_name().and_then(|name| name.to_str()) != Some("checkout") {
        return Err(CoreError::GitClone(
            "refusing to clone outside a managed checkout directory".into(),
        ));
    }
    if dest.join(".git").is_dir() {
        return Ok(());
    }
    if dest.exists() {
        std::fs::remove_dir_all(dest)?;
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let output = Command::new("git")
        .args(["clone", "--depth", "1", "--"])
        .arg(url)
        .arg(dest)
        .output()
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                CoreError::GitClone(
                    "git is not installed; Forgeline does not install it for you".into(),
                )
            } else {
                CoreError::GitClone(error.to_string())
            }
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let detail = if stderr.trim().is_empty() {
            stdout.trim()
        } else {
            stderr.trim()
        };
        let _ = std::fs::remove_dir_all(dest);
        return Err(CoreError::GitClone(if detail.is_empty() {
            "git clone failed".into()
        } else {
            detail.to_string()
        }));
    }
    Ok(())
}
