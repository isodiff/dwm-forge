use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;

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
        Ok(toml::from_str(&content)?)
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    pub fn sha256_file(path: &Path) -> anyhow::Result<String> {
        sha256_file(path)
    }

    pub fn verify_patches(&self, patches_dir: &Path) -> anyhow::Result<()> {
        let mut patch_paths = std::fs::read_dir(patches_dir)?
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| path.extension().is_some_and(|ext| ext == "diff"))
            .collect::<Vec<_>>();
        patch_paths.sort();

        for path in patch_paths {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| anyhow::anyhow!("Invalid patch filename: {}", path.display()))?;
            let expected = self
                .patches
                .get(name)
                .ok_or_else(|| anyhow::anyhow!("Patch {} is missing from [patches]", name))?;
            let actual = sha256_file(&path)?;
            if actual != *expected {
                anyhow::bail!(
                    "Patch hash mismatch for {}: expected {}, got {}",
                    name,
                    expected,
                    actual
                );
            }
        }

        for name in self.patches.keys() {
            if !patches_dir.join(name).is_file() {
                anyhow::bail!("Patch file {} does not exist", name);
            }
        }

        Ok(())
    }
}

pub fn sha256_file(path: &Path) -> anyhow::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = sha2::Sha256::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn save_load_and_verify_patch_hashes() {
        let dir = tempdir().unwrap();
        let patches_dir = dir.path().join("patches");
        std::fs::create_dir(&patches_dir).unwrap();
        let patch = patches_dir.join("01-test.diff");
        std::fs::write(&patch, "patch content").unwrap();

        let lockfile = Lockfile {
            upstream_url: "https://git.suckless.org/dwm".into(),
            upstream_tarball_base: "https://dl.suckless.org/dwm".into(),
            pinned_ref: String::new(),
            dwm_version: "6.5".into(),
            patches: BTreeMap::from([("01-test.diff".into(), sha256_file(&patch).unwrap())]),
        };
        let lock_path = dir.path().join("lock.toml");
        lockfile.save(&lock_path).unwrap();
        let loaded = Lockfile::load(&lock_path).unwrap();
        assert_eq!(lockfile, loaded);
        loaded.verify_patches(&patches_dir).unwrap();

        std::fs::write(&patch, "corrupted").unwrap();
        assert!(loaded.verify_patches(&patches_dir).is_err());
    }

    #[test]
    fn rejects_unmapped_diff_file() {
        let dir = tempdir().unwrap();
        let patches_dir = dir.path().join("patches");
        std::fs::create_dir(&patches_dir).unwrap();
        std::fs::write(patches_dir.join("01-extra.diff"), "content").unwrap();
        let lockfile = Lockfile {
            upstream_url: String::new(),
            upstream_tarball_base: String::new(),
            pinned_ref: String::new(),
            dwm_version: String::new(),
            patches: BTreeMap::new(),
        };
        assert!(lockfile.verify_patches(&patches_dir).is_err());
    }
}
