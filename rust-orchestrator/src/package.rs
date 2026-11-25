use crate::cli::PackageArgs;
use crate::lockfile::{sha256_file, Lockfile};
use anyhow::{Context, Result};
use flate2::write::GzEncoder;
use flate2::{Compression, GzBuilder};
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use tar::{Builder, EntryType, Header};
use walkdir::WalkDir;

pub fn run_package(repo_root: &Path, args: &PackageArgs) -> Result<()> {
    let lockfile_path = repo_root.join("config/dwm-lock.toml");
    let lockfile = Lockfile::load(&lockfile_path)
        .with_context(|| format!("loading lockfile {}", lockfile_path.display()))?;
    let staging_dir = resolve_path(repo_root, &args.staging_dir);
    if !staging_dir.is_dir() {
        anyhow::bail!(
            "staging directory does not exist: {}",
            staging_dir.display()
        );
    }
    let staging_dir = fs::canonicalize(&staging_dir)
        .with_context(|| format!("canonicalizing staging directory {}", staging_dir.display()))?;

    let out_dir = resolve_path(repo_root, &args.out_dir);
    fs::create_dir_all(&out_dir)
        .with_context(|| format!("creating output directory {}", out_dir.display()))?;
    let out_dir = fs::canonicalize(&out_dir)
        .with_context(|| format!("canonicalizing output directory {}", out_dir.display()))?;
    if out_dir.starts_with(&staging_dir) {
        anyhow::bail!(
            "output directory {} must not be inside staging directory {}",
            out_dir.display(),
            staging_dir.display()
        );
    }

    let short_sha: String = args.sha.chars().take(12).collect();
    if short_sha.is_empty()
        || !short_sha
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    {
        anyhow::bail!("--sha must begin with filename-safe characters");
    }

    let archive_name = format!("dwm-patched-{}-{short_sha}.tar.gz", lockfile.dwm_version);
    let top_level = format!("dwm-patched-{}-{short_sha}", lockfile.dwm_version);
    let archive_path = out_dir.join(&archive_name);
    let config_source = repo_root.join("config/config.h");
    if !config_source.is_file() {
        anyhow::bail!(
            "configuration header does not exist: {}",
            config_source.display()
        );
    }

    // Collect and sort before opening output files so an output directory nested under the
    // staging tree cannot cause the archive to include its own in-progress output.
    let mut entries = WalkDir::new(&staging_dir)
        .follow_links(false)
        .into_iter()
        .map(|entry| entry.map_err(anyhow::Error::from))
        .collect::<Result<Vec<_>>>()?;
    entries.retain(|entry| entry.path() != staging_dir);
    entries.sort_by(|left, right| {
        let left_relative = left
            .path()
            .strip_prefix(staging_dir.as_path())
            .unwrap_or(left.path());
        let right_relative = right
            .path()
            .strip_prefix(staging_dir.as_path())
            .unwrap_or(right.path());
        left_relative.cmp(right_relative)
    });

    write_archive(&archive_path, &staging_dir, &top_level, &entries)?;

    let config_output = out_dir.join("config.h");
    fs::copy(&config_source, &config_output).with_context(|| {
        format!(
            "copying {} to {}",
            config_source.display(),
            config_output.display()
        )
    })?;

    let archive_hash = sha256_file(&archive_path)?;
    let config_hash = sha256_file(&config_output)?;
    let sums = format!("{archive_hash}  {archive_name}\n{config_hash}  config.h\n");
    fs::write(out_dir.join("SHA256SUMS"), sums)
        .with_context(|| format!("writing checksum file in {}", out_dir.display()))?;

    println!("{}", archive_path.display());
    Ok(())
}

fn resolve_path(repo_root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        repo_root.join(path)
    }
}

fn write_archive(
    archive_path: &Path,
    staging_dir: &Path,
    top_level: &str,
    entries: &[walkdir::DirEntry],
) -> Result<()> {
    let output = File::create(archive_path)
        .with_context(|| format!("creating archive {}", archive_path.display()))?;
    let gzip = GzBuilder::new()
        .mtime(0)
        .operating_system(255)
        .write(output, Compression::default());
    let mut archive = Builder::new(gzip);

    append_directory(&mut archive, Path::new(top_level), 0o755)?;
    for entry in entries {
        let relative = entry.path().strip_prefix(staging_dir)?;
        let archive_path = Path::new(top_level).join(relative);
        let metadata = fs::symlink_metadata(entry.path())?;

        if metadata.file_type().is_symlink() {
            let target = fs::read_link(entry.path())?;
            let mut header = new_header(0o777, 0, EntryType::Symlink);
            archive
                .append_link(&mut header, archive_path, target)
                .with_context(|| format!("adding symlink {}", entry.path().display()))?;
        } else if metadata.is_dir() {
            append_directory(&mut archive, &archive_path, archive_mode(&metadata, true))?;
        } else if metadata.is_file() {
            let mut header = new_header(
                archive_mode(&metadata, false),
                metadata.len(),
                EntryType::Regular,
            );
            let mut file = File::open(entry.path())
                .with_context(|| format!("opening {}", entry.path().display()))?;
            archive
                .append_data(&mut header, archive_path, &mut file)
                .with_context(|| format!("adding file {}", entry.path().display()))?;
        } else {
            anyhow::bail!("unsupported staging entry: {}", entry.path().display());
        }
    }

    let gzip: GzEncoder<File> = archive.into_inner().context("finalizing tar archive")?;
    gzip.finish().context("finalizing gzip stream")?;
    Ok(())
}

fn append_directory<W: std::io::Write>(
    archive: &mut Builder<W>,
    path: &Path,
    mode: u32,
) -> Result<()> {
    let mut header = new_header(mode, 0, EntryType::Directory);
    archive
        .append_data(&mut header, path, std::io::empty())
        .with_context(|| format!("adding directory {}", path.display()))?;
    Ok(())
}

fn new_header(mode: u32, size: u64, entry_type: EntryType) -> Header {
    let mut header = Header::new_gnu();
    header.set_entry_type(entry_type);
    header.set_size(size);
    header.set_mode(mode);
    header.set_uid(0);
    header.set_gid(0);
    header.set_mtime(0);
    header.set_username("").expect("empty username is valid");
    header.set_groupname("").expect("empty group name is valid");
    header
}

#[cfg(unix)]
fn archive_mode(metadata: &fs::Metadata, _is_dir: bool) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o777
}

#[cfg(not(unix))]
fn archive_mode(_metadata: &fs::Metadata, is_dir: bool) -> u32 {
    if is_dir {
        0o755
    } else {
        0o644
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use tempfile::tempdir;

    #[test]
    fn creates_reproducible_archive_and_sha256sums() {
        let temp = tempdir().unwrap();
        let repo = temp.path().join("repo");
        let staging = repo.join("stage");
        fs::create_dir_all(staging.join("nested")).unwrap();
        fs::create_dir_all(repo.join("config")).unwrap();
        fs::write(staging.join("nested/file.txt"), "contents\n").unwrap();
        fs::write(repo.join("config/config.h"), "#define TEST 1\n").unwrap();
        let lockfile = Lockfile {
            upstream_url: String::new(),
            upstream_tarball_base: String::new(),
            pinned_ref: String::new(),
            dwm_version: "6.5".into(),
            patches: BTreeMap::new(),
        };
        lockfile.save(&repo.join("config/dwm-lock.toml")).unwrap();

        let args = PackageArgs {
            staging_dir: PathBuf::from("stage"),
            out_dir: PathBuf::from("out-one"),
            sha: "0123456789abcdef".into(),
        };
        run_package(&repo, &args).unwrap();
        let first = fs::read(repo.join("out-one/dwm-patched-6.5-0123456789ab.tar.gz")).unwrap();

        let second_args = PackageArgs {
            out_dir: PathBuf::from("out-two"),
            ..args
        };
        run_package(&repo, &second_args).unwrap();
        let second = fs::read(repo.join("out-two/dwm-patched-6.5-0123456789ab.tar.gz")).unwrap();
        assert_eq!(first, second);

        let sums = fs::read_to_string(repo.join("out-one/SHA256SUMS")).unwrap();
        assert!(sums.contains("  dwm-patched-6.5-0123456789ab.tar.gz\n"));
        assert!(sums.contains("  config.h\n"));
        assert_eq!(
            sha256_file(&repo.join("out-one/config.h")).unwrap(),
            sums.lines()
                .nth(1)
                .unwrap()
                .split_whitespace()
                .next()
                .unwrap()
        );
    }

    #[test]
    fn rejects_output_directory_nested_in_staging() {
        let temp = tempdir().unwrap();
        let repo = temp.path().join("repo");
        let staging = repo.join("stage");
        fs::create_dir_all(&staging).unwrap();
        fs::create_dir_all(repo.join("config")).unwrap();
        Lockfile {
            upstream_url: String::new(),
            upstream_tarball_base: String::new(),
            pinned_ref: String::new(),
            dwm_version: "6.5".into(),
            patches: BTreeMap::new(),
        }
        .save(&repo.join("config/dwm-lock.toml"))
        .unwrap();

        let error = run_package(
            &repo,
            &PackageArgs {
                staging_dir: PathBuf::from("stage"),
                out_dir: PathBuf::from("stage/release/output"),
                sha: "0123456789ab".into(),
            },
        )
        .unwrap_err();
        assert!(error
            .to_string()
            .contains("must not be inside staging directory"));
    }
}
