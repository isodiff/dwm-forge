from __future__ import annotations

import re
import subprocess
import tomllib
from collections.abc import Callable

from conftest import MockRepository

CliRunner = Callable[..., subprocess.CompletedProcess[str]]


def test_fetch_updates_lockfile_tracks_upstream_and_reports_current(
    mock_repo: MockRepository,
    run_cli: CliRunner,
) -> None:
    first = run_cli(mock_repo.root, "fetch", "--update-lock")
    assert first.returncode == 0, first.stderr
    pinned_ref = tomllib.loads((mock_repo.root / "config" / "dwm-lock.toml").read_text())[
        "pinned_ref"
    ]
    assert re.fullmatch(r"[0-9a-f]{40}", pinned_ref)
    assert "UPDATE_AVAILABLE:" in first.stdout

    second = run_cli(mock_repo.root, "fetch")
    assert second.returncode == 0, second.stderr
    assert "UP_TO_DATE" in second.stdout

    mock_repo.push_commit("upstream-change.txt", "new upstream revision\n", "upstream update")
    third = run_cli(mock_repo.root, "fetch")
    assert third.returncode == 0, third.stderr
    assert "UPDATE_AVAILABLE:" in third.stdout
    assert pinned_ref in third.stdout
