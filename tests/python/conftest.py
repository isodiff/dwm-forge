from __future__ import annotations

import hashlib
import json
import os
import subprocess
from dataclasses import dataclass
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]


def run_git(*args: str, cwd: Path) -> str:
    completed = subprocess.run(
        ["git", *args],
        cwd=cwd,
        check=True,
        capture_output=True,
        text=True,
    )
    return completed.stdout.strip()


@dataclass
class MockRepository:
    root: Path
    seed_repo: Path
    upstream_bare: Path
    patch_hashes: dict[str, str]

    def write_lockfile(self, *, patch_hashes: dict[str, str] | None = None) -> None:
        hashes = patch_hashes if patch_hashes is not None else self.patch_hashes
        lines = [
            f"upstream_url = {json.dumps(str(self.upstream_bare))}",
            'upstream_tarball_base = "https://example.invalid/dwm"',
            'pinned_ref = ""',
            'dwm_version = "6.5"',
            "",
            "[patches]",
        ]
        lines.extend(
            f"{json.dumps(name)} = {json.dumps(digest)}" for name, digest in hashes.items()
        )
        (self.root / "config" / "dwm-lock.toml").write_text("\n".join(lines) + "\n")

    def update_patch_hash(self, patch_name: str) -> None:
        patch_path = self.root / "patches" / patch_name
        self.patch_hashes[patch_name] = hashlib.sha256(patch_path.read_bytes()).hexdigest()
        self.write_lockfile()

    def push_commit(self, filename: str, contents: str, message: str) -> None:
        (self.seed_repo / filename).write_text(contents)
        run_git("add", filename, cwd=self.seed_repo)
        run_git("commit", "-m", message, cwd=self.seed_repo)
        run_git("push", "origin", "master", cwd=self.seed_repo)


def _patch_hash(name: str, content: str) -> tuple[str, str]:
    return name, hashlib.sha256(content.encode()).hexdigest()


@pytest.fixture(scope="session")
def dwm_forge_bin() -> Path:
    candidates = [
        REPO_ROOT / "rust-orchestrator" / "target" / "release" / "dwm-forge",
        REPO_ROOT / "rust-orchestrator" / "target" / "debug" / "dwm-forge",
    ]
    for binary in candidates:
        if binary.is_file() and os.access(binary, os.X_OK):
            return binary
    pytest.fail(
        "dwm-forge binary not found; build rust-orchestrator with "
        "`cargo build --release` or `cargo build` first"
    )


@pytest.fixture
def mock_repo(tmp_path: Path) -> MockRepository:
    root = tmp_path / "workspace"
    root.mkdir()
    upstream_bare = tmp_path / "upstream.git"
    seed_repo = tmp_path / "upstream-seed"

    (root / "config").mkdir()
    (root / "patches").mkdir()
    (root / "config" / "config.h").write_text("#define DEFAULT_PRESENT 1\n#define USER_SETTING 2\n")

    (seed_repo).mkdir()
    run_git("init", "--initial-branch=master", cwd=seed_repo)
    run_git("config", "user.name", "dwm-forge tests", cwd=seed_repo)
    run_git("config", "user.email", "dwm-forge-tests@example.invalid", cwd=seed_repo)
    (seed_repo / "Makefile").write_text(
        "all: dwm\n\ndwm: dwm.c config.h\n\t$(CC) $(CFLAGS) -o $@ dwm.c\n\nclean:\n\trm -f dwm\n"
    )
    (seed_repo / "dwm.c").write_text('#include "config.h"\nint patch_value = 0;\n')
    (seed_repo / "config.def.h").write_text(
        "#define DEFAULT_PRESENT 1\n#define MISSING_FROM_USER 1\n"
    )
    run_git("init", "--bare", "--initial-branch=master", str(upstream_bare), cwd=tmp_path)
    run_git("remote", "add", "origin", str(upstream_bare), cwd=seed_repo)
    run_git("add", ".", cwd=seed_repo)
    run_git("commit", "-m", "initial upstream source", cwd=seed_repo)
    run_git("push", "--set-upstream", "origin", "master", cwd=seed_repo)

    patch_one = (
        "--- a/dwm.c\n"
        "+++ b/dwm.c\n"
        "@@ -1,2 +1,2 @@\n"
        ' #include "config.h"\n'
        "-int patch_value = 0;\n"
        "+int patch_value = 1;\n"
    )
    patch_two = (
        "--- a/dwm.c\n"
        "+++ b/dwm.c\n"
        "@@ -1,2 +1,2 @@\n"
        ' #include "config.h"\n'
        "-int patch_value = 1;\n"
        "+int patch_value = 2;\n"
    )
    patch_files = {
        "01-first.diff": patch_one,
        "02-second.diff": patch_two,
    }
    patch_hashes: dict[str, str] = {}
    for name, contents in patch_files.items():
        (root / "patches" / name).write_text(contents)
        patch_hashes.update([_patch_hash(name, contents)])

    repo = MockRepository(root, seed_repo, upstream_bare, patch_hashes)
    repo.write_lockfile()
    return repo


@pytest.fixture
def run_cli(dwm_forge_bin: Path):
    def invoke(repo_root: Path, *args: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [str(dwm_forge_bin), "--repo-root", str(repo_root), *args],
            cwd=repo_root,
            capture_output=True,
            text=True,
            check=False,
        )

    return invoke
