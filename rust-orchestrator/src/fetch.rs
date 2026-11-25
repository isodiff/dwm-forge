use crate::cli::{FetchArgs, FetchSource};
use crate::lockfile::Lockfile;
use anyhow::{Context, Result};
use sha2::Digest;
use std::io::Read;
use std::path::Path;
use std::process::Command;

pub fn run_fetch(repo_root: &Path, args: &FetchArgs) -> Result<()> {
    let lock_path = repo_root.join("config/dwm-lock.toml");
    let mut lockfile = Lockfile::load(&lock_path)
        .with_context(|| format!("loading lockfile {}", lock_path.display()))?;

    let resolved_ref = match args.source.clone() {
        FetchSource::Git => fetch_git(repo_root, &lockfile)?,
        FetchSource::Tarball => fetch_tarball(repo_root, &lockfile)?,
    };

    if lockfile.pinned_ref == resolved_ref {
        println!("UP_TO_DATE");
    } else {
        println!(
            "UPDATE_AVAILABLE: {} -> {}",
            if lockfile.pinned_ref.is_empty() {
                "none"
            } else {
                lockfile.pinned_ref.as_str()
            },
            resolved_ref
        );
        if args.update_lock {
            lockfile.pinned_ref = resolved_ref;
            lockfile
                .save(&lock_path)
                .with_context(|| format!("saving lockfile {}", lock_path.display()))?;
        }
    }

    Ok(())
}

fn fetch_git(repo_root: &Path, lockfile: &Lockfile) -> Result<String> {
    let cache_dir = repo_root.join(".cache/dwm-upstream");
    std::fs::create_dir_all(cache_dir.parent().expect("cache directory has a parent"))?;

    if !cache_dir.exists() {
        let output = Command::new("git")
            .arg("clone")
            .arg(&lockfile.upstream_url)
            .arg(&cache_dir)
            .output()
            .context("starting git clone")?;
        ensure_success(output.status.success(), "git clone", &output.stderr)?;
    } else {
        let output = Command::new("git")
            .args(["-C"])
            .arg(&cache_dir)
            .args(["fetch", "origin"])
            .output()
            .context("starting git fetch")?;
        ensure_success(output.status.success(), "git fetch origin", &output.stderr)?;
    }

    let mut resolved_ref = None;
    for reference in ["origin/master", "origin/main", "HEAD"] {
        let output = Command::new("git")
            .args(["-C"])
            .arg(&cache_dir)
            .args(["rev-parse", "--verify", reference])
            .output()
            .with_context(|| format!("resolving upstream reference {reference}"))?;
        if output.status.success() {
            let resolved = String::from_utf8(output.stdout)?.trim().to_owned();
            if !resolved.is_empty() {
                resolved_ref = Some(resolved);
                break;
            }
        }
    }

    let resolved_ref = resolved_ref.ok_or_else(|| {
        anyhow::anyhow!(
            "could not resolve origin/master, origin/main, or HEAD in {}",
            cache_dir.display()
        )
    })?;
    let pre_checkout_reset = Command::new("git")
        .args(["-C"])
        .arg(&cache_dir)
        .args(["reset", "--hard"])
        .output()
        .context("cleaning upstream cache before checkout")?;
    ensure_success(
        pre_checkout_reset.status.success(),
        "git reset --hard",
        &pre_checkout_reset.stderr,
    )?;
    let pre_checkout_clean = Command::new("git")
        .args(["-C"])
        .arg(&cache_dir)
        .args(["clean", "-fdx"])
        .output()
        .context("removing untracked files from upstream cache")?;
    ensure_success(
        pre_checkout_clean.status.success(),
        "git clean -fdx",
        &pre_checkout_clean.stderr,
    )?;

    let checkout = Command::new("git")
        .args(["-C"])
        .arg(&cache_dir)
        .args(["checkout", "--detach"])
        .arg(&resolved_ref)
        .output()
        .context("checking out resolved upstream commit")?;
    ensure_success(
        checkout.status.success(),
        "git checkout --detach",
        &checkout.stderr,
    )?;

    let reset = Command::new("git")
        .args(["-C"])
        .arg(&cache_dir)
        .args(["reset", "--hard"])
        .arg(&resolved_ref)
        .output()
        .context("resetting upstream cache to resolved commit")?;
    ensure_success(reset.status.success(), "git reset --hard", &reset.stderr)?;

    let clean = Command::new("git")
        .args(["-C"])
        .arg(&cache_dir)
        .args(["clean", "-fdx"])
        .output()
        .context("removing untracked files from resolved upstream cache")?;
    ensure_success(clean.status.success(), "git clean -fdx", &clean.stderr)?;

    Ok(resolved_ref)
}

fn fetch_tarball(repo_root: &Path, lockfile: &Lockfile) -> Result<String> {
    let url = format!(
        "{}/dwm-{}.tar.gz",
        lockfile.upstream_tarball_base.trim_end_matches('/'),
        lockfile.dwm_version
    );
    let response = ureq::get(&url)
        .call()
        .with_context(|| format!("downloading {url}"))?;
    let mut bytes = Vec::new();
    response
        .into_reader()
        .read_to_end(&mut bytes)
        .with_context(|| format!("reading tarball from {url}"))?;

    let digest = hex::encode(sha2::Sha256::digest(&bytes));
    unpack_tarball(repo_root, &bytes, &url)?;
    std::fs::write(repo_root.join(".cache/dwm-tarball.tar.gz"), &bytes)
        .context("caching downloaded upstream tarball")?;
    Ok(digest)
}

pub fn materialize_cached_tarball(repo_root: &Path, pinned_ref: &str) -> Result<()> {
    let archive_path = repo_root.join(".cache/dwm-tarball.tar.gz");
    let bytes = std::fs::read(&archive_path)
        .with_context(|| format!("reading cached tarball {}", archive_path.display()))?;
    let digest = hex::encode(sha2::Sha256::digest(&bytes));
    if digest != pinned_ref {
        anyhow::bail!(
            "cached tarball SHA-256 mismatch: pinned {}, got {}",
            pinned_ref,
            digest
        );
    }
    unpack_tarball(repo_root, &bytes, "cached tarball")
}

fn unpack_tarball(repo_root: &Path, bytes: &[u8], source: &str) -> Result<()> {
    let cache_parent = repo_root.join(".cache");
    std::fs::create_dir_all(&cache_parent)?;
    let cache_dir = cache_parent.join("dwm-tarball");
    let unpack_dir = tempfile::tempdir_in(&cache_parent)?;
    let decoder = flate2::read::GzDecoder::new(bytes);
    let mut archive = tar::Archive::new(decoder);
    archive
        .unpack(unpack_dir.path())
        .with_context(|| format!("unpacking {source}"))?;
    replace_cache(&cache_dir, unpack_dir.path())
}

fn replace_cache(cache_dir: &Path, unpacked_dir: &Path) -> Result<()> {
    let backup_dir = cache_dir.with_extension("old");
    if backup_dir.exists() {
        std::fs::remove_dir_all(&backup_dir)?;
    }
    if cache_dir.exists() {
        std::fs::rename(cache_dir, &backup_dir)
            .with_context(|| format!("moving old cache {}", cache_dir.display()))?;
    }

    match copy_directory(unpacked_dir, cache_dir) {
        Ok(()) => {
            if backup_dir.exists() {
                std::fs::remove_dir_all(backup_dir)?;
            }
            Ok(())
        }
        Err(error) => {
            if cache_dir.exists() {
                std::fs::remove_dir_all(cache_dir)?;
            }
            if backup_dir.exists() {
                std::fs::rename(backup_dir, cache_dir)?;
            }
            Err(error)
        }
    }
}

fn copy_directory(source: &Path, destination: &Path) -> Result<()> {
    std::fs::create_dir_all(destination)?;
    for entry in walkdir::WalkDir::new(source) {
        let entry = entry?;
        let relative = entry.path().strip_prefix(source)?;
        if relative.as_os_str().is_empty() {
            continue;
        }
        let target = destination.join(relative);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target)?;
        } else {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

fn ensure_success(success: bool, action: &str, stderr: &[u8]) -> Result<()> {
    if success {
        Ok(())
    } else {
        anyhow::bail!(
            "{action} failed: {}",
            String::from_utf8_lossy(stderr).trim()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use tempfile::tempdir;

    #[test]
    fn fetch_git_updates_lockfile_for_local_remote() -> Result<()> {
        let temp = tempdir()?;
        let repo_root = temp.path();
        let remote = repo_root.join("remote.git");
        std::fs::create_dir_all(repo_root.join("config"))?;
        let output = Command::new("git")
            .args(["init", "--bare"])
            .arg(&remote)
            .output()?;
        ensure_success(output.status.success(), "git init", &output.stderr)?;

        let seed = repo_root.join("seed");
        std::fs::create_dir(&seed)?;
        let output = Command::new("git")
            .arg("-C")
            .arg(&seed)
            .args(["init"])
            .output()?;
        ensure_success(output.status.success(), "git init", &output.stderr)?;
        for (key, value) in [("user.email", "test@example.com"), ("user.name", "Test")] {
            let output = Command::new("git")
                .arg("-C")
                .arg(&seed)
                .args(["config", key, value])
                .output()?;
            ensure_success(output.status.success(), "git config", &output.stderr)?;
        }
        std::fs::write(seed.join("source.txt"), "source")?;
        let output = Command::new("git")
            .arg("-C")
            .arg(&seed)
            .args(["add", "."])
            .output()?;
        ensure_success(output.status.success(), "git add", &output.stderr)?;
        let output = Command::new("git")
            .arg("-C")
            .arg(&seed)
            .args(["commit", "-m", "initial"])
            .output()?;
        ensure_success(output.status.success(), "git commit", &output.stderr)?;
        let output = Command::new("git")
            .arg("-C")
            .arg(&seed)
            .arg("remote")
            .arg("add")
            .arg("origin")
            .arg(&remote)
            .output()?;
        ensure_success(output.status.success(), "git remote add", &output.stderr)?;
        let output = Command::new("git")
            .arg("-C")
            .arg(&seed)
            .args(["push", "origin", "HEAD:master"])
            .output()?;
        ensure_success(output.status.success(), "git push", &output.stderr)?;

        let lockfile = Lockfile {
            upstream_url: remote.to_string_lossy().into_owned(),
            upstream_tarball_base: String::new(),
            pinned_ref: String::new(),
            dwm_version: "6.5".into(),
            patches: BTreeMap::new(),
        };
        lockfile.save(&repo_root.join("config/dwm-lock.toml"))?;
        let args = FetchArgs {
            source: FetchSource::Git,
            update_lock: true,
        };
        run_fetch(repo_root, &args)?;
        let updated = Lockfile::load(&repo_root.join("config/dwm-lock.toml"))?;
        assert_eq!(updated.pinned_ref.len(), 40);
        assert_eq!(fetch_git(repo_root, &updated)?, updated.pinned_ref);
        let cache_dir = repo_root.join(".cache/dwm-upstream");
        let head = Command::new("git")
            .args(["-C"])
            .arg(cache_dir)
            .args(["rev-parse", "HEAD"])
            .output()?;
        ensure_success(head.status.success(), "git rev-parse HEAD", &head.stderr)?;
        assert_eq!(String::from_utf8(head.stdout)?.trim(), updated.pinned_ref);
        Ok(())
    }

    #[test]
    fn tarball_cache_replacement_removes_stale_files() -> Result<()> {
        let temp = tempdir()?;
        let cache = temp.path().join("dwm-tarball");
        std::fs::create_dir_all(&cache)?;
        std::fs::write(cache.join("stale"), "stale")?;
        let unpacked = temp.path().join("unpacked");
        std::fs::create_dir(&unpacked)?;
        std::fs::write(unpacked.join("fresh"), "fresh")?;
        replace_cache(&cache, &unpacked)?;
        assert!(!cache.join("stale").exists());
        assert!(cache.join("fresh").exists());
        Ok(())
    }
}
