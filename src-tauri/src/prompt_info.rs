use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DirMeta {
    pub cwd: PathBuf,
    pub git_branch: Option<String>,
    pub git_dirty: bool,
    pub node_version: Option<String>,
    pub python_version: Option<String>,
    pub rust_project: bool,
    pub package_json: bool,
}

#[derive(Clone, Default)]
pub struct DirectoryMetaCache {
    inner: Arc<RwLock<HashMap<PathBuf, DirMeta>>>,
}

impl DirectoryMetaCache {
    pub async fn get(&self, path: PathBuf) -> DirMeta {
        if let Some(meta) = self.inner.read().await.get(&path).cloned() {
            return meta;
        }
        let meta = compute_meta(&path).await;
        self.inner.write().await.insert(path, meta.clone());
        meta
    }

    pub async fn invalidate(&self, path: &Path) {
        self.inner.write().await.remove(path);
    }
}

pub async fn compute_meta(path: &Path) -> DirMeta {
    let git_branch = timed_git(path, ["rev-parse", "--abbrev-ref", "HEAD"]).await;
    let git_dirty = timed_git(path, ["status", "--porcelain"])
        .await
        .is_some_and(|output| !output.trim().is_empty());
    DirMeta {
        cwd: path.to_path_buf(),
        git_branch: git_branch.filter(|branch| branch != "HEAD"),
        git_dirty,
        node_version: read_trim(path.join(".nvmrc"))
            .or_else(|| read_trim(path.join(".node-version"))),
        python_version: read_trim(path.join(".python-version")),
        rust_project: path.join("Cargo.toml").exists(),
        package_json: path.join("package.json").exists(),
    }
}

async fn timed_git<const N: usize>(path: &Path, args: [&str; N]) -> Option<String> {
    let mut command = Command::new("git");
    command.arg("-C").arg(path).args(args).stdout(Stdio::piped()).stderr(Stdio::null());
    let output =
        tokio::time::timeout(Duration::from_millis(500), command.output()).await.ok()?.ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

fn read_trim(path: PathBuf) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub fn truncate_path(path: &Path) -> String {
    let parts = path.iter().map(|part| part.to_string_lossy().to_string()).collect::<Vec<_>>();
    if parts.len() <= 3 {
        path.display().to_string()
    } else {
        format!("…/{}", parts[parts.len().saturating_sub(3)..].join("/"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deep_path_truncates() {
        assert_eq!(truncate_path(Path::new("/a/b/c/d")), "…/b/c/d");
    }
}
