use crate::cli::{BuildArgs, FetchArgs, FetchSource};
use crate::fetch;
use crate::lockfile::{sha256_file, Lockfile};
use anyhow::{Context, Result};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use walkdir::WalkDir;

pub fn check_config_drift(def_h: &Path, user_h: &Path) -> Result<Vec<String>> {
    let def_content =
        fs::read_to_string(def_h).with_context(|| format!("reading {}", def_h.display()))?;
    let user_content =
        fs::read_to_string(user_h).with_context(|| format!("reading {}", user_h.display()))?;
    let def_identifiers = config_identifiers(&def_content);
    let user_identifiers = config_identifiers(&user_content);
    let mut missing = def_identifiers
        .difference(&user_identifiers)
        .cloned()
        .collect::<Vec<_>>();
    missing.sort();
    for identifier in &missing {
        eprintln!("WARN: missing symbol in config.h: {identifier}");
    }
    Ok(missing)
}

fn config_identifiers(content: &str) -> HashSet<String> {
    let mut identifiers = HashSet::new();
    for line in content.lines() {
        let line = line.trim();
        if let Some(definition) = line.strip_prefix("#define") {
            let name = definition.split_whitespace().next().unwrap_or("");
            let name = name.split('(').next().unwrap_or(name);
            if is_c_identifier(name) {
                identifiers.insert(name.to_owned());
            }
        } else if line.starts_with("static const ") {
            let declaration = line
                .split_once('=')
                .map_or(line, |(before_initializer, _)| before_initializer)
                .trim_end_matches(';');
            let names = c_identifiers(declaration);
            if let Some(name) = names.last() {
                if !is_type_keyword(name) {
                    identifiers.insert(name.clone());
                }
            }
        }
    }
    identifiers
}

fn c_identifiers(text: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        if ch == '_' || ch.is_ascii_alphanumeric() {
            current.push(ch);
        } else if !current.is_empty() {
            result.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        result.push(current);
    }
    result
}

fn is_c_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some(ch) if ch == '_' || ch.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn is_type_keyword(value: &str) -> bool {
    matches!(
        value,
        "const"
            | "volatile"
            | "register"
            | "extern"
            | "static"
            | "int"
            | "char"
            | "short"
            | "long"
            | "float"
            | "double"
            | "signed"
            | "unsigned"
            | "void"
            | "bool"
            | "_Bool"
            | "struct"
            | "union"
            | "enum"
    )
}

pub fn run_build(repo_root: &Path, args: &BuildArgs) -> Result<()> {
    let lockfile_path = repo_root.join("config/dwm-lock.toml");
    let lockfile = Lockfile::load(&lockfile_path)
        .with_context(|| format!("loading lockfile {}", lockfile_path.display()))?;
    let pinned_ref = lockfile.pinned_ref.trim();
    if pinned_ref.is_empty() {
        anyhow::bail!(
            "config/dwm-lock.toml has empty pinned_ref; run 'dwm-forge fetch --update-lock' first"
        );
    }
    let upstream_source = locate_cached_upstream(repo_root, pinned_ref)?;

    // Keep an automatically-created temporary directory alive through the full build.
    let temporary_directory = if args.work_dir.is_none() {
        Some(tempfile::tempdir().context("creating temporary build directory")?)
    } else {
        None
    };
    let staging_dir = match (&args.work_dir, &temporary_directory) {
        (Some(work_dir), _) => {
            if work_dir.exists() {
                fs::remove_dir_all(work_dir).with_context(|| {
                    format!("removing old work directory {}", work_dir.display())
                })?;
            }
            fs::create_dir_all(work_dir)
                .with_context(|| format!("creating work directory {}", work_dir.display()))?;
            work_dir.clone()
        }
        (None, Some(temp_dir)) => temp_dir.path().to_path_buf(),
        (None, None) => {
            unreachable!("temporary directory is present when no work directory is set")
        }
    };

    copy_upstream_files(&upstream_source, &staging_dir)?;

    let patches_dir = repo_root.join("patches");
    let mut patch_paths = fs::read_dir(&patches_dir)
        .with_context(|| format!("reading patches directory {}", patches_dir.display()))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    patch_paths.retain(|path| path.extension().is_some_and(|ext| ext == "diff"));
    patch_paths.sort();

    if !args.skip_hash_check {
        lockfile.verify_patches(&patches_dir)?;
    }

    for patch_path in &patch_paths {
        let filename = patch_path.file_name().unwrap_or_default().to_string_lossy();
        let absolute_patch =
            fs::canonicalize(patch_path).with_context(|| format!("resolving patch {filename}"))?;
        let dry_run = Command::new("patch")
            .args(["--dry-run", "--fuzz=0", "-p1", "-i"])
            .arg(&absolute_patch)
            .current_dir(&staging_dir)
            .output()
            .with_context(|| format!("starting dry-run for patch {filename}"))?;
        ensure_patch_success(&filename, "dry-run", dry_run)?;

        let apply = Command::new("patch")
            .args(["--fuzz=0", "-p1", "-i"])
            .arg(&absolute_patch)
            .current_dir(&staging_dir)
            .output()
            .with_context(|| format!("starting application of patch {filename}"))?;
        ensure_patch_success(&filename, "application", apply)?;
    }

    let repo_config = repo_root.join("config/config.h");
    let staging_config = staging_dir.join("config.h");
    fs::copy(&repo_config, &staging_config).with_context(|| {
        format!(
            "copying {} to {}",
            repo_config.display(),
            staging_config.display()
        )
    })?;
    let default_config = staging_dir.join("config.def.h");
    if default_config.is_file() {
        check_config_drift(&default_config, &staging_config)?;
    }

    if !args.patch_only {
        let output = Command::new("make")
            .args(["clean", "all"])
            .current_dir(&staging_dir)
            .output()
            .context("starting make clean all")?;
        ensure_command_success("make clean all", output)?;
    }

    if let Some(c_test_dir) = &args.c_test_dir {
        let test_dir = if c_test_dir.is_absolute() {
            c_test_dir.clone()
        } else {
            repo_root.join(c_test_dir)
        };
        let absolute_staging = fs::canonicalize(&staging_dir)?;
        let output = Command::new("make")
            .arg("-C")
            .arg(&test_dir)
            .arg("test")
            .arg(format!("STAGING_DIR={}", absolute_staging.display()))
            .output()
            .with_context(|| format!("starting C tests in {}", test_dir.display()))?;
        ensure_command_success("C tests", output)?;
    }

    // The guard intentionally lives until after compilation and optional C tests.
    drop(temporary_directory);
    Ok(())
}

fn locate_cached_upstream(repo_root: &Path, pinned_ref: &str) -> Result<PathBuf> {
    if is_sha256(pinned_ref) {
        return locate_cached_tarball(repo_root, pinned_ref);
    }
    if !is_git_sha(pinned_ref) {
        anyhow::bail!(
            "pinned_ref must be a 40-character Git SHA or a 64-character tarball SHA-256"
        );
    }

    let git_cache = repo_root.join(".cache/dwm-upstream");
    if !git_cache.is_dir() {
        fetch::run_fetch(
            repo_root,
            &FetchArgs {
                source: FetchSource::Git,
                update_lock: false,
            },
        )?;
    }
    if !git_cache.is_dir() {
        anyhow::bail!(
            "fetch completed but upstream cache {} is absent",
            git_cache.display()
        );
    }
    ensure_git_checkout(&git_cache, pinned_ref)?;
    Ok(git_cache)
}

fn locate_cached_tarball(repo_root: &Path, pinned_ref: &str) -> Result<PathBuf> {
    let cache_parent = repo_root.join(".cache");
    let archive_path = cache_parent.join("dwm-tarball.tar.gz");
    let cached_hash = if archive_path.is_file() {
        Some(sha256_file(&archive_path)?)
    } else {
        None
    };

    if cached_hash.as_deref() != Some(pinned_ref) {
        fetch::run_fetch(
            repo_root,
            &FetchArgs {
                source: FetchSource::Tarball,
                update_lock: false,
            },
        )?;
    }

    let actual_hash = sha256_file(&archive_path)
        .with_context(|| format!("reading cached tarball {}", archive_path.display()))?;
    if actual_hash != pinned_ref {
        anyhow::bail!(
            "cached tarball SHA-256 mismatch: pinned {}, got {}",
            pinned_ref,
            actual_hash
        );
    }
    fetch::materialize_cached_tarball(repo_root, pinned_ref)?;

    let unpacked_dir = cache_parent.join("dwm-tarball");
    let entries = fs::read_dir(&unpacked_dir)?.collect::<std::io::Result<Vec<_>>>()?;
    if entries.len() == 1 && entries[0].path().is_dir() {
        Ok(entries[0].path())
    } else {
        anyhow::bail!(
            "expected one source directory in tarball cache {}",
            unpacked_dir.display()
        )
    }
}

fn is_git_sha(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn ensure_git_checkout(cache_dir: &Path, pinned_ref: &str) -> Result<()> {
    let commit_ref = format!("{pinned_ref}^{{commit}}");
    let object = Command::new("git")
        .args(["-C"])
        .arg(cache_dir)
        .args(["cat-file", "-e", &commit_ref])
        .output()
        .with_context(|| format!("checking for pinned commit {pinned_ref}"))?;

    if !object.status.success() {
        let fetch_output = Command::new("git")
            .args(["-C"])
            .arg(cache_dir)
            .args(["fetch", "origin"])
            .output()
            .context("fetching origin to obtain pinned commit")?;
        ensure_command_success("git fetch origin", fetch_output)?;
    }

    let pre_checkout_reset = Command::new("git")
        .args(["-C"])
        .arg(cache_dir)
        .args(["reset", "--hard"])
        .output()
        .context("cleaning upstream cache before pinned checkout")?;
    ensure_command_success("git reset --hard", pre_checkout_reset)?;
    let pre_checkout_clean = Command::new("git")
        .args(["-C"])
        .arg(cache_dir)
        .args(["clean", "-fdx"])
        .output()
        .context("removing untracked files from upstream cache")?;
    ensure_command_success("git clean -fdx", pre_checkout_clean)?;

    let checkout = Command::new("git")
        .args(["-C"])
        .arg(cache_dir)
        .args(["checkout", "--detach"])
        .arg(pinned_ref)
        .output()
        .with_context(|| format!("checking out pinned commit {pinned_ref}"))?;
    ensure_command_success("git checkout --detach", checkout)?;

    let reset = Command::new("git")
        .args(["-C"])
        .arg(cache_dir)
        .args(["reset", "--hard"])
        .arg(pinned_ref)
        .output()
        .with_context(|| format!("resetting cache to pinned commit {pinned_ref}"))?;
    ensure_command_success("git reset --hard", reset)?;
    let clean = Command::new("git")
        .args(["-C"])
        .arg(cache_dir)
        .args(["clean", "-fdx"])
        .output()
        .context("removing untracked files from pinned upstream cache")?;
    ensure_command_success("git clean -fdx", clean)?;
    Ok(())
}

fn copy_upstream_files(upstream_source: &Path, staging_dir: &Path) -> Result<()> {
    for entry in WalkDir::new(upstream_source) {
        let entry = entry?;
        let relative = entry.path().strip_prefix(upstream_source)?;
        if relative
            .components()
            .any(|component| component.as_os_str() == ".git")
        {
            continue;
        }
        if relative.as_os_str().is_empty() {
            continue;
        }
        let destination = staging_dir.join(relative);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&destination)?;
        } else {
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(entry.path(), &destination).with_context(|| {
                format!(
                    "copying {} to {}",
                    entry.path().display(),
                    destination.display()
                )
            })?;
        }
    }
    Ok(())
}

fn ensure_patch_success(filename: &str, phase: &str, output: Output) -> Result<()> {
    if output.status.success() {
        Ok(())
    } else {
        anyhow::bail!(
            "patch {filename} {phase} failed: stderr: {}; stdout: {}",
            String::from_utf8_lossy(&output.stderr).trim(),
            String::from_utf8_lossy(&output.stdout).trim()
        )
    }
}

fn ensure_command_success(action: &str, output: Output) -> Result<()> {
    if output.status.success() {
        Ok(())
    } else {
        anyhow::bail!(
            "{action} failed (exit {}): stdout: {}; stderr: {}",
            output.status,
            String::from_utf8_lossy(&output.stdout).trim(),
            String::from_utf8_lossy(&output.stderr).trim()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use tempfile::tempdir;

    fn git(cwd: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(cwd)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }

    #[test]
    fn config_drift_checks_macros_and_static_const_identifiers() {
        let temp = tempdir().unwrap();
        let default_path = temp.path().join("config.def.h");
        let custom_path = temp.path().join("config.h");
        fs::write(
            &default_path,
            "#define DEFAULT_MACRO 1\nstatic const char *fonts[] = { \"x\" };\nstatic const Key keys[] = {};\n",
        )
        .unwrap();
        fs::write(
            &custom_path,
            "#define DEFAULT_MACRO 2\nstatic const char *fonts[] = { \"y\" };\n",
        )
        .unwrap();
        let missing = check_config_drift(&default_path, &custom_path).unwrap();
        assert_eq!(missing, ["keys"]);
    }

    #[test]
    fn build_fetches_and_checks_out_pinned_commit_and_cleans_work_dir() {
        let temp = tempdir().unwrap();
        let repo_root = temp.path().join("repo");
        let bare = temp.path().join("upstream.git");
        let seed = temp.path().join("seed");
        fs::create_dir_all(repo_root.join("config")).unwrap();
        fs::create_dir_all(repo_root.join("patches")).unwrap();
        fs::create_dir(&bare).unwrap();
        fs::create_dir(&seed).unwrap();
        git(&bare, &["init", "--bare", "--initial-branch=master"]);
        git(&seed, &["init", "--initial-branch=master"]);
        git(&seed, &["config", "user.name", "dwm-forge tests"]);
        git(
            &seed,
            &["config", "user.email", "dwm-forge-tests@example.invalid"],
        );
        fs::write(seed.join("Makefile"), "all:\n\t@true\nclean:\n\t@true\n").unwrap();
        fs::write(seed.join("dwm.c"), "int source_revision = 1;\n").unwrap();
        fs::write(seed.join("config.def.h"), "#define DEFAULT_PRESENT 1\n").unwrap();
        git(&seed, &["add", "."]);
        git(&seed, &["commit", "-m", "initial source"]);
        git(&seed, &["remote", "add", "origin", bare.to_str().unwrap()]);
        git(&seed, &["push", "origin", "master"]);

        let cache_dir = repo_root.join(".cache/dwm-upstream");
        fs::create_dir_all(cache_dir.parent().unwrap()).unwrap();
        let clone = Command::new("git")
            .args(["clone"])
            .arg(&bare)
            .arg(&cache_dir)
            .output()
            .unwrap();
        assert!(
            clone.status.success(),
            "{}",
            String::from_utf8_lossy(&clone.stderr)
        );

        fs::write(seed.join("dwm.c"), "int source_revision = 2;\n").unwrap();
        git(&seed, &["add", "dwm.c"]);
        git(&seed, &["commit", "-m", "pinned source revision"]);
        git(&seed, &["push", "origin", "master"]);
        let pinned_ref = git(&seed, &["rev-parse", "HEAD"]);
        fs::write(
            repo_root.join("config/config.h"),
            "#define DEFAULT_PRESENT 1\n",
        )
        .unwrap();
        Lockfile {
            upstream_url: bare.to_string_lossy().into_owned(),
            upstream_tarball_base: String::new(),
            pinned_ref: pinned_ref.clone(),
            dwm_version: "6.5".into(),
            patches: BTreeMap::new(),
        }
        .save(&repo_root.join("config/dwm-lock.toml"))
        .unwrap();
        fs::write(cache_dir.join("dwm.c"), "dirty cached worktree\n").unwrap();
        fs::write(cache_dir.join("untracked-cache-file"), "stale").unwrap();

        let work_dir = repo_root.join("stage");
        fs::create_dir_all(work_dir.join("stale/nested")).unwrap();
        fs::write(work_dir.join("stale/nested/file"), "stale").unwrap();
        fs::write(work_dir.join("obsolete.txt"), "obsolete").unwrap();
        let args = BuildArgs {
            work_dir: Some(work_dir.clone()),
            skip_hash_check: false,
            patch_only: true,
            c_test_dir: None,
        };

        run_build(&repo_root, &args).unwrap();

        assert_eq!(
            fs::read_to_string(work_dir.join("dwm.c")).unwrap(),
            "int source_revision = 2;\n"
        );
        assert!(!work_dir.join("stale").exists());
        assert!(!work_dir.join("obsolete.txt").exists());
        assert!(!cache_dir.join("untracked-cache-file").exists());
        assert_eq!(git(&cache_dir, &["rev-parse", "HEAD"]), pinned_ref);
        assert!(git(&cache_dir, &["status", "--porcelain"]).is_empty());
    }

    #[test]
    fn build_uses_tarball_cache_when_pinned_sha256_matches_archive() {
        let temp = tempdir().unwrap();
        let repo_root = temp.path();
        let cache_dir = repo_root.join(".cache/dwm-tarball");
        let source_dir = cache_dir.join("dwm-6.5");
        fs::create_dir_all(&source_dir).unwrap();
        fs::create_dir_all(repo_root.join("config")).unwrap();
        fs::create_dir_all(repo_root.join("patches")).unwrap();
        fs::write(source_dir.join("dwm.c"), "int tarball_source = 1;\n").unwrap();
        fs::write(source_dir.join("config.def.h"), "#define PRESENT 1\n").unwrap();
        fs::write(repo_root.join("config/config.h"), "#define PRESENT 1\n").unwrap();
        let archive_path = repo_root.join(".cache/dwm-tarball.tar.gz");
        let archive_file = fs::File::create(&archive_path).unwrap();
        let gzip = flate2::write::GzEncoder::new(archive_file, flate2::Compression::default());
        let mut archive = tar::Builder::new(gzip);
        for (name, contents) in [
            ("dwm-6.5/dwm.c", "int tarball_source = 1;\n"),
            ("dwm-6.5/config.def.h", "#define PRESENT 1\n"),
        ] {
            let mut header = tar::Header::new_gnu();
            header.set_size(contents.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            archive
                .append_data(&mut header, name, contents.as_bytes())
                .unwrap();
        }
        archive.into_inner().unwrap().finish().unwrap();
        let pinned_ref = sha256_file(&archive_path).unwrap();
        Lockfile {
            upstream_url: String::new(),
            upstream_tarball_base: String::new(),
            pinned_ref,
            dwm_version: "6.5".into(),
            patches: BTreeMap::new(),
        }
        .save(&repo_root.join("config/dwm-lock.toml"))
        .unwrap();
        fs::write(source_dir.join("dwm.c"), "tampered unpacked cache\n").unwrap();

        let work_dir = repo_root.join("stage");
        run_build(
            repo_root,
            &BuildArgs {
                work_dir: Some(work_dir.clone()),
                skip_hash_check: false,
                patch_only: true,
                c_test_dir: None,
            },
        )
        .unwrap();
        assert_eq!(
            fs::read_to_string(work_dir.join("dwm.c")).unwrap(),
            "int tarball_source = 1;\n"
        );
    }

    #[test]
    fn build_requires_a_non_empty_pinned_ref() {
        let temp = tempdir().unwrap();
        let repo_root = temp.path();
        fs::create_dir_all(repo_root.join("config")).unwrap();
        Lockfile {
            upstream_url: String::new(),
            upstream_tarball_base: String::new(),
            pinned_ref: "  ".into(),
            dwm_version: "6.5".into(),
            patches: BTreeMap::new(),
        }
        .save(&repo_root.join("config/dwm-lock.toml"))
        .unwrap();

        let error = run_build(
            repo_root,
            &BuildArgs {
                work_dir: None,
                skip_hash_check: false,
                patch_only: true,
                c_test_dir: None,
            },
        )
        .unwrap_err();
        assert_eq!(
            error.to_string(),
            "config/dwm-lock.toml has empty pinned_ref; run 'dwm-forge fetch --update-lock' first"
        );
    }
}
