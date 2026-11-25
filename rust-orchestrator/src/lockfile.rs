use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use sha2::Digest;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Lockfile {
    pub upstream_url: String,
    pub upstream_tarball_base: String,
    pub pinned_ref: String,
    pub dwm_version: String,
    pub patches: BTreeMap<String, String>,
}

impl Lockfile {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let lockfile: Lockfile = toml::from_str(&content)?;
        Ok(lockfile)
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    pub fn sha256_file(path: &Path) -> anyhow::Result<String> {
        let mut file = File::open(path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;

        let result = sha2::Sha256::digest(&buffer);
        Ok(hex::encode(result).to_lowercase())
    }

    pub fn verify_patches(&self, patches_dir: &Path) -> anyhow::Result<()> {
        for (patch_name, expected_hash) in &self.patches {
            let patch_path = patches_dir.join(patch_name);
            if !patch_path.exists() {
                return Err(anyhow::anyhow!(
                    "Patch file {} does not exist",
                    patch_name
                ));
            }
            
            let actual_hash = Self::sha256_file(&patch_path)?;
            if actual_hash != *expected_hash {
                return Err(anyhow::anyhow!(
                    "Patch hash mismatch for {}: expected {}, got {}",
                    patch_name,
                    expected_hash,
                    actual_hash
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_round_trip_save_load() {
        let dir = tempdir().unwrap();
        let lockfile_path = dir.path().join("lockfile.toml");

        let original_lockfile = Lockfile {
            upstream_url: "https://git.suckless.org/dwm".to_string(),
            upstream_tarball_base: "dwm-6.4".to_string(),
            pinned_ref: "v6.4".to_string(),
            dwm_version: "6.4".to_string(),
            patches: BTreeMap::from([
                ("01-alpha.diff".to_string(), "abc123def456".to_string()),
                ("02-cfacts.diff".to_string(), "def456abc123".to_string()),
            ]),
        };

        // Save
        original_lockfile.save(&lockfile_path).unwrap();

        // Load
        let loaded_lockfile = Lockfile::load(&lockfile_path).unwrap();

        // Verify they're equal
        assert_eq!(original_lockfile, loaded_lockfile);
    }

    #[test]
    fn test_patch_hash_mismatch() {
        let dir = tempdir().unwrap();
        let patches_dir = dir.path().join("patches");
        std::fs::create_dir(&patches_dir).unwrap();

        // Create a patch file with one content
        let patch_path = patches_dir.join("01-alpha.diff");
        std::fs::write(&patch_path, "some content").unwrap();

        let lockfile = Lockfile {
            upstream_url: "https://git.suckless.org/dwm".to_string(),
            upstream_tarball_base: "dwm-6.4".to_string(),
            pinned_ref: "v6.4".to_string(),
            dwm_version: "6.4".to_string(),
            patches: BTreeMap::from([
                ("01-alpha.diff".to_string(), "wrong_hash".to_string()),
            ]),
        };

        // This should fail due to hash mismatch
        let result = lockfile.verify_patches(&patches_dir);
        assert!(result.is_err());
    }
}