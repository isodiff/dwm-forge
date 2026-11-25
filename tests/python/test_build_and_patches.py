from __future__ import annotations

import subprocess
from collections.abc import Callable

from conftest import MockRepository

CliRunner = Callable[..., subprocess.CompletedProcess[str]]


def test_patch_only_applies_patches_in_order_and_copies_config(
    mock_repo: MockRepository,
    run_cli: CliRunner,
) -> None:
    staging = mock_repo.root / "stage"
    result = run_cli(
        mock_repo.root,
        "build",
        "--work-dir",
        str(staging),
        "--patch-only",
    )

    assert result.returncode == 0, result.stderr
    assert (staging / "dwm.c").read_text().splitlines()[-1] == "int patch_value = 2;"
    assert (staging / "config.h").read_bytes() == (
        mock_repo.root / "config" / "config.h"
    ).read_bytes()


def test_build_rejects_patch_hash_mismatch(
    mock_repo: MockRepository,
    run_cli: CliRunner,
) -> None:
    patch_path = mock_repo.root / "patches" / "01-first.diff"
    patch_path.write_text(patch_path.read_text() + "# changed without updating the lock\n")

    result = run_cli(mock_repo.root, "build", "--patch-only")

    assert result.returncode != 0
    assert "Patch hash mismatch for 01-first.diff" in result.stderr


def test_build_rejects_patch_hunk_conflict(
    mock_repo: MockRepository,
    run_cli: CliRunner,
) -> None:
    second_patch = mock_repo.root / "patches" / "02-second.diff"
    second_patch.write_text(
        "--- a/dwm.c\n"
        "+++ b/dwm.c\n"
        "@@ -1,2 +1,2 @@\n"
        ' #include "config.h"\n'
        "-int patch_value = 999;\n"
        "+int patch_value = 3;\n"
    )
    mock_repo.update_patch_hash("02-second.diff")

    result = run_cli(mock_repo.root, "build", "--patch-only")

    assert result.returncode != 0
    assert "02-second.diff" in result.stderr
    assert "dry-run failed" in result.stderr


def test_build_warns_for_default_define_missing_from_user_config(
    mock_repo: MockRepository,
    run_cli: CliRunner,
) -> None:
    result = run_cli(
        mock_repo.root,
        "build",
        "--work-dir",
        str(mock_repo.root / "stage"),
        "--patch-only",
    )

    assert result.returncode == 0, result.stderr
    assert "WARN: missing symbol in config.h: MISSING_FROM_USER" in result.stderr
