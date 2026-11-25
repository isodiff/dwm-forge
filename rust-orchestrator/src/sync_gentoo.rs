use crate::cli::SyncGentooArgs;
use crate::lockfile::sha256_file;
use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Deserialize)]
struct Release {
    assets: Vec<ReleaseAsset>,
}

#[derive(Debug, Deserialize)]
struct ReleaseAsset {
    name: String,
    browser_download_url: String,
}

pub fn run_sync_gentoo(args: &SyncGentooArgs) -> Result<()> {
    let api_url = format!(
        "{}/repos/{}/releases/latest",
        args.api_base.trim_end_matches('/'),
        args.repo.trim_matches('/')
    );
    let mut request = ureq::get(&api_url).set("User-Agent", "dwm-forge");
    let token = std::env::var("GITHUB_TOKEN").unwrap_or_default();
    if !token.is_empty() {
        request = request.set("Authorization", &format!("Bearer {token}"));
    }

    let response = request
        .call()
        .with_context(|| format!("querying latest release at {api_url}"))?;
    let release: Release = response
        .into_json()
        .context("parsing latest GitHub release response")?;
    let archive_asset = release
        .assets
        .iter()
        .find(|asset| asset.name.ends_with(".tar.gz"))
        .context("latest release has no .tar.gz asset")?;
    let config_asset = release
        .assets
        .iter()
        .find(|asset| asset.name == "config.h")
        .context("latest release has no config.h asset")?;
    let sums_asset = release
        .assets
        .iter()
        .find(|asset| asset.name == "SHA256SUMS")
        .context("latest release has no SHA256SUMS asset")?;
    validate_asset_name(&archive_asset.name)?;

    let temporary_directory = tempfile::tempdir().context("creating download directory")?;
    let archive_path = temporary_directory.path().join(&archive_asset.name);
    let config_path = temporary_directory.path().join("config.h");
    let sums_path = temporary_directory.path().join("SHA256SUMS");

    download_asset(&archive_asset.browser_download_url, &archive_path)?;
    download_asset(&config_asset.browser_download_url, &config_path)?;
    download_asset(&sums_asset.browser_download_url, &sums_path)?;
    verify_checksums(&sums_path, &archive_asset.name, &archive_path, &config_path)?;

    if args.dry_run {
        println!(
            "DRY_RUN: verified {} and config.h; would sync savedconfig to {} and distfile to {}",
            archive_asset.name,
            args.savedconfig_path.display(),
            args.distdir.display()
        );
        return Ok(());
    }

    if let Some(parent) = args.savedconfig_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("creating savedconfig directory {}", parent.display()))?;
    }
    fs::copy(&config_path, &args.savedconfig_path)
        .with_context(|| format!("copying config.h to {}", args.savedconfig_path.display()))?;

    fs::create_dir_all(&args.distdir)
        .with_context(|| format!("creating distdir {}", args.distdir.display()))?;
    let distfile_path = args.distdir.join(&archive_asset.name);
    fs::copy(&archive_path, &distfile_path)
        .with_context(|| format!("copying distfile to {}", distfile_path.display()))?;

    let mut ebuild = Command::new("ebuild");
    ebuild.arg(&args.ebuild_path).arg("manifest");
    run_command(&mut ebuild, "ebuild")?;
    let mut emerge = Command::new("emerge");
    emerge.args(["-1", "x11-wm/dwm"]);
    run_command(&mut emerge, "emerge")?;
    Ok(())
}

fn download_asset(url: &str, destination: &Path) -> Result<()> {
    let response = ureq::get(url)
        .set("User-Agent", "dwm-forge")
        .call()
        .with_context(|| format!("downloading {url}"))?;
    let mut reader = response.into_reader();
    let mut file = fs::File::create(destination)
        .with_context(|| format!("creating download {}", destination.display()))?;
    std::io::copy(&mut reader, &mut file)
        .with_context(|| format!("writing download {}", destination.display()))?;
    Ok(())
}

fn verify_checksums(
    sums_path: &Path,
    archive_name: &str,
    archive_path: &Path,
    config_path: &Path,
) -> Result<()> {
    let checksums = parse_checksums(&fs::read_to_string(sums_path)?)?;
    verify_one(&checksums, archive_name, archive_path)?;
    verify_one(&checksums, "config.h", config_path)?;
    Ok(())
}

fn parse_checksums(contents: &str) -> Result<BTreeMap<String, String>> {
    let mut checksums = BTreeMap::new();
    for (line_number, line) in contents.lines().enumerate() {
        let line = line.trim_end();
        if line.is_empty() {
            continue;
        }
        let (digest, filename) = line
            .split_once(char::is_whitespace)
            .with_context(|| format!("invalid SHA256SUMS line {}", line_number + 1))?;
        let filename = filename
            .trim_start()
            .strip_prefix('*')
            .unwrap_or(filename.trim_start());
        if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            anyhow::bail!(
                "invalid SHA-256 digest on SHA256SUMS line {}",
                line_number + 1
            );
        }
        validate_asset_name(filename)?;
        if checksums
            .insert(filename.to_owned(), digest.to_ascii_lowercase())
            .is_some()
        {
            anyhow::bail!("duplicate checksum entry for {filename}");
        }
    }
    Ok(checksums)
}

fn validate_asset_name(name: &str) -> Result<()> {
    let path = Path::new(name);
    if name.is_empty()
        || path.file_name().and_then(|component| component.to_str()) != Some(name)
        || name == "."
        || name == ".."
    {
        anyhow::bail!("unsafe release asset filename: {name:?}");
    }
    Ok(())
}

fn verify_one(checksums: &BTreeMap<String, String>, name: &str, path: &Path) -> Result<()> {
    let expected = checksums
        .get(name)
        .with_context(|| format!("SHA256SUMS has no entry for {name}"))?;
    let actual = sha256_file(path)?;
    if actual != *expected {
        anyhow::bail!("SHA-256 mismatch for {name}: expected {expected}, got {actual}");
    }
    Ok(())
}

fn run_command(command: &mut Command, name: &str) -> Result<()> {
    let output = command
        .output()
        .with_context(|| format!("starting {name}"))?;
    if !output.status.success() {
        anyhow::bail!(
            "{name} failed ({}): stdout: {}; stderr: {}",
            output.status,
            String::from_utf8_lossy(&output.stdout).trim(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sha256sum_lines() {
        let parsed = parse_checksums(
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef  dwm.tar.gz\n\
             abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789  config.h\n",
        )
        .unwrap();
        assert_eq!(parsed.len(), 2);
        assert!(parsed.contains_key("dwm.tar.gz"));
        assert!(parsed.contains_key("config.h"));
    }

    #[test]
    fn rejects_path_traversal_checksum_names() {
        let result = parse_checksums(
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef  ../config.h",
        );
        assert!(result.is_err());
    }
}
