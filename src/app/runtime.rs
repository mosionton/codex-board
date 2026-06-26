use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, anyhow};

use crate::{
    provider_config::{self, ProviderRegistry},
    session_store::load_sessions,
    ui,
};

use super::{App, AppAction};

pub fn run() -> Result<()> {
    let current_dir = env::current_dir().context("failed to read current directory")?;
    let codex_home = codex_home()?;
    let provider_config_path = provider_config::config_path(&codex_home);
    let codex_config_path = provider_config::codex_config_path(&codex_home);
    let codex_auth_path = provider_config::codex_auth_path(&codex_home);
    let mut provider_registry = ProviderRegistry::load(&provider_config_path)?;
    provider_registry.merge_defaults(provider_config::load_codex_config_providers(
        &codex_config_path,
        &codex_auth_path,
    )?);
    let applied_provider_id = provider_config::load_applied_model_provider(&codex_config_path)?;

    let sessions_dir = codex_home.join("sessions");
    let sessions = load_sessions(&sessions_dir)?;
    let mut app = App::new(
        sessions,
        current_dir,
        provider_registry,
        provider_config_path,
        codex_config_path,
        sessions_dir,
    );
    app.providers.applied_provider_id = applied_provider_id;
    let action = ui::run_tui(&mut app)?;

    match action {
        AppAction::Quit => Ok(()),
        AppAction::Resume(session) => exec_codex_resume(&session.id, &session.cwd),
    }
}

fn codex_home() -> Result<PathBuf> {
    if let Some(path) = env::var_os("CODEX_HOME") {
        return Ok(PathBuf::from(path));
    }
    let home = env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .or_else(
            || match (env::var_os("HOMEDRIVE"), env::var_os("HOMEPATH")) {
                (Some(drive), Some(path)) => {
                    let mut home = PathBuf::from(drive);
                    home.push(path);
                    Some(home.into_os_string())
                }
                _ => None,
            },
        )
        .ok_or_else(|| {
            anyhow!("CODEX_HOME is not set and no home directory variables are available")
        })?;
    Ok(PathBuf::from(home).join(".codex"))
}

pub(super) fn exec_codex_resume(session_id: &str, cwd: &Path) -> Result<()> {
    ensure_session_cwd_exists(cwd)?;

    let status = Command::new("codex")
        .current_dir(cwd)
        .arg("resume")
        .arg(session_id)
        .status()
        .with_context(|| {
            format!(
                "failed to start `codex resume {session_id}` in {}",
                cwd.display()
            )
        })?;

    if status.success() {
        Ok(())
    } else {
        anyhow::bail!("codex exited with status {status}");
    }
}

pub(super) fn ensure_session_cwd_exists(cwd: &Path) -> Result<()> {
    if !cwd.exists() {
        anyhow::bail!("session directory does not exist: {}", cwd.display());
    }
    if !cwd.is_dir() {
        anyhow::bail!("session path is not a directory: {}", cwd.display());
    }
    Ok(())
}
