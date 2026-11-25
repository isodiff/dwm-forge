use crate::cli::{BuildArgs, FetchArgs, FetchSource};
use crate::fetch;
use crate::lockfile::Lockfile;
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
    let upstream_source = locate_cached_upstream(repo_root)?;

    // Keep an automatically-created temporary directory alive through the full build.
    let temporary_directory = if args.work_dir.is_none() {
        Some(tempfile::tempdir().context("creating temporary build directory")?)
    } else {
        None
    };
    let staging_dir = match (&args.work_dir, &temporary_directory) {
        (Some(work_dir), _) => {
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

fn locate_cached_upstream(repo_root: &Path) -> Result<PathBuf> {
    let cache_dir = repo_root.join(".cache");
    let git_cache = cache_dir.join("dwm-upstream");
    if git_cache.is_dir() {
        return Ok(git_cache);
    }

    let tarball_cache = cache_dir.join("dwm-tarball");
    if tarball_cache.is_dir() {
        let entries = fs::read_dir(&tarball_cache)?.collect::<std::io::Result<Vec<_>>>()?;
        if entries.len() == 1 && entries[0].path().is_dir() {
            return Ok(entries[0].path());
        }
        return Ok(tarball_cache);
    }

    fetch::run_fetch(
        repo_root,
        &FetchArgs {
            source: FetchSource::Git,
            update_lock: false,
        },
    )?;
    if git_cache.is_dir() {
        Ok(git_cache)
    } else {
        anyhow::bail!(
            "fetch completed but upstream cache {} is absent",
            git_cache.display()
        )
    }
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
    use tempfile::tempdir;

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
}
