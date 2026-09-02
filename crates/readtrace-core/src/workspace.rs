use crate::{ProjectStore, SCHEMA_VERSION};
use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultRecord {
    pub vault_id: String,
    pub name: String,
    pub relative_path: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceManifest {
    pub schema_version: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub vaults: Vec<VaultRecord>,
}

/// Owns discovery and creation of independent vaults below one workspace root.
/// Existing `ProjectStore` paths remain valid, so this adds organization
/// without coupling a vault to the CLI or server.
#[derive(Debug, Clone)]
pub struct WorkspaceStore {
    pub root: PathBuf,
}

impl WorkspaceStore {
    pub fn init(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(root.join("vaults"))?;
        let store = Self { root };
        if !store.manifest_path().exists() {
            let timestamp = Utc::now();
            store.write_manifest(&WorkspaceManifest {
                schema_version: SCHEMA_VERSION,
                created_at: timestamp,
                updated_at: timestamp,
                vaults: Vec::new(),
            })?;
        }
        Ok(store)
    }

    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        let store = Self {
            root: root.as_ref().to_path_buf(),
        };
        if !store.manifest_path().is_file() {
            return Err(anyhow!(
                "not a ReadTrace workspace; run workspace-init first: {}",
                store.root.display()
            ));
        }
        let _ = store.manifest()?;
        Ok(store)
    }

    pub fn manifest(&self) -> Result<WorkspaceManifest> {
        let bytes = fs::read(self.manifest_path())
            .with_context(|| format!("read {}", self.manifest_path().display()))?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    pub fn create_vault(&self, name: &str) -> Result<VaultRecord> {
        let name = name.trim();
        if name.is_empty() {
            return Err(anyhow!("vault name cannot be empty"));
        }
        let mut manifest = self.manifest()?;
        if manifest.vaults.iter().any(|vault| vault.name == name) {
            return Err(anyhow!("vault name already exists: {name}"));
        }
        let slug = safe_slug(name);
        let mut relative = format!("vaults/{slug}");
        let mut suffix = 2;
        while self.root.join(&relative).exists() {
            relative = format!("vaults/{slug}-{suffix}");
            suffix += 1;
        }
        let vault = VaultRecord {
            vault_id: format!("vault-{}", Uuid::new_v4()),
            name: name.into(),
            relative_path: relative.replace('\\', "/"),
            created_at: Utc::now(),
        };
        ProjectStore::init(self.root.join(&vault.relative_path))?;
        manifest.vaults.push(vault.clone());
        manifest.updated_at = Utc::now();
        self.write_manifest(&manifest)?;
        Ok(vault)
    }

    pub fn list_vaults(&self) -> Result<Vec<VaultRecord>> {
        let mut vaults = self.manifest()?.vaults;
        vaults.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(vaults)
    }

    pub fn vault_path(&self, name_or_id: &str) -> Result<PathBuf> {
        let manifest = self.manifest()?;
        let vault = manifest
            .vaults
            .iter()
            .find(|vault| vault.name == name_or_id || vault.vault_id == name_or_id)
            .ok_or_else(|| anyhow!("vault not found: {name_or_id}"))?;
        let relative = safe_manifest_path(&vault.relative_path)?;
        Ok(self.root.join(relative))
    }

    pub fn open_vault(&self, name_or_id: &str) -> Result<ProjectStore> {
        ProjectStore::open(self.vault_path(name_or_id)?)
    }

    fn manifest_path(&self) -> PathBuf {
        self.root.join("workspace.json")
    }

    fn write_manifest(&self, manifest: &WorkspaceManifest) -> Result<()> {
        let path = self.manifest_path();
        let temporary = path.with_extension("tmp");
        fs::write(&temporary, serde_json::to_vec_pretty(manifest)?)?;
        fs::rename(temporary, path)?;
        Ok(())
    }
}

fn safe_slug(value: &str) -> String {
    let slug = value
        .chars()
        .map(|character| {
            if character.is_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        format!("vault-{}", &Uuid::new_v4().to_string()[..8])
    } else {
        slug.into()
    }
}

fn safe_manifest_path(value: &str) -> Result<PathBuf> {
    let path = Path::new(value);
    if path.is_absolute()
        || path
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
        || !value.replace('\\', "/").starts_with("vaults/")
    {
        return Err(anyhow!(
            "vault path must stay below workspace/vaults: {value}"
        ));
    }
    Ok(path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_and_opens_separate_vaults() {
        let root = std::env::temp_dir().join(format!("readtrace-workspace-{}", Uuid::new_v4()));
        let workspace = WorkspaceStore::init(&root).unwrap();
        let first = workspace.create_vault("课程资料").unwrap();
        let second = workspace.create_vault("game notes").unwrap();
        assert_ne!(first.relative_path, second.relative_path);
        assert_eq!(workspace.list_vaults().unwrap().len(), 2);
        assert!(workspace.open_vault(&first.vault_id).is_ok());
        assert!(workspace.open_vault("课程资料").is_ok());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_edited_manifest_path_traversal() {
        assert!(safe_manifest_path("../outside").is_err());
        assert!(safe_manifest_path("vaults/../../outside").is_err());
        assert!(safe_manifest_path("vaults/safe").is_ok());
    }
}
