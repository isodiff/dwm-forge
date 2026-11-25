from __future__ import annotations

import hashlib
import json
import subprocess
import tarfile
import threading
from collections.abc import Callable, Iterator
from contextlib import contextmanager
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import unquote, urlsplit

from conftest import MockRepository

CliRunner = Callable[..., subprocess.CompletedProcess[str]]
ARCHIVE_NAME = "dwm-patched-6.5-0123456789ab.tar.gz"


class ReleaseServer(ThreadingHTTPServer):
    daemon_threads = True
    artifacts: dict[str, bytes]


@contextmanager
def serve_release(artifacts: dict[str, bytes]) -> Iterator[str]:
    class Handler(BaseHTTPRequestHandler):
        def do_GET(self) -> None:
            request_path = urlsplit(self.path).path
            if request_path == "/repos/isodiff/dwm-forge/releases/latest":
                base_url = f"http://{self.server.server_address[0]}:{self.server.server_address[1]}"
                release = {
                    "assets": [
                        {
                            "name": name,
                            "browser_download_url": f"{base_url}/assets/{name}",
                        }
                        for name in artifacts
                    ]
                }
                body = json.dumps(release).encode()
                content_type = "application/json"
            elif request_path.startswith("/assets/"):
                name = unquote(request_path.removeprefix("/assets/"))
                body = artifacts.get(name, b"")
                if name not in artifacts:
                    self.send_error(404)
                    return
                content_type = "application/octet-stream"
            else:
                self.send_error(404)
                return

            self.send_response(200)
            self.send_header("Content-Type", content_type)
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def log_message(self, _format: str, *args: object) -> None:
            return

    server = ReleaseServer(("127.0.0.1", 0), Handler)
    server.artifacts = artifacts
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    base_url = f"http://127.0.0.1:{server.server_address[1]}"
    try:
        yield base_url
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=5)


def test_package_is_deterministic_and_writes_valid_checksums(
    mock_repo: MockRepository,
    run_cli: CliRunner,
    tmp_path: Path,
) -> None:
    staging = tmp_path / "staging"
    (staging / "nested").mkdir(parents=True)
    (staging / "dwm.c").write_text("int main(void) { return 0; }\n")
    (staging / "nested" / "source.h").write_text("#define VALUE 1\n")
    out_one = tmp_path / "dist-one"
    out_two = tmp_path / "dist-two"

    for out_dir in (out_one, out_two):
        result = run_cli(
            mock_repo.root,
            "package",
            "--staging-dir",
            str(staging),
            "--out-dir",
            str(out_dir),
            "--sha",
            "0123456789abcdef",
        )
        assert result.returncode == 0, result.stderr

    archive_one = out_one / ARCHIVE_NAME
    archive_two = out_two / ARCHIVE_NAME
    assert archive_one.read_bytes() == archive_two.read_bytes()
    assert (out_one / "config.h").read_bytes() == (
        mock_repo.root / "config" / "config.h"
    ).read_bytes()

    with tarfile.open(archive_one, "r:gz") as archive:
        members = archive.getmembers()
        names = {member.name for member in members}
        assert "dwm-patched-6.5-0123456789ab/dwm.c" in names
        assert "dwm-patched-6.5-0123456789ab/nested/source.h" in names
        assert all(member.mtime == 0 for member in members)
        assert all(member.uid == 0 and member.gid == 0 for member in members)

    expected_lines = [
        f"{hashlib.sha256(archive_one.read_bytes()).hexdigest()}  {ARCHIVE_NAME}",
        f"{hashlib.sha256((out_one / 'config.h').read_bytes()).hexdigest()}  config.h",
    ]
    assert (out_one / "SHA256SUMS").read_text().splitlines() == expected_lines


def test_sync_gentoo_dry_run_verifies_release_assets_and_rejects_corruption(
    mock_repo: MockRepository,
    run_cli: CliRunner,
    tmp_path: Path,
) -> None:
    archive_content = b"mock deterministic dwm archive\n"
    config_content = b"#define RELEASE_CONFIG 1\n"
    artifacts = {
        ARCHIVE_NAME: archive_content,
        "config.h": config_content,
        "SHA256SUMS": (
            f"{hashlib.sha256(archive_content).hexdigest()}  {ARCHIVE_NAME}\n"
            f"{hashlib.sha256(config_content).hexdigest()}  config.h\n"
        ).encode(),
    }
    savedconfig = tmp_path / "portage" / "dwm-config.h"
    distdir = tmp_path / "distfiles"
    ebuild = tmp_path / "dwm-6.5.ebuild"

    with serve_release(artifacts) as api_base:
        result = run_cli(
            mock_repo.root,
            "sync-gentoo",
            "--repo",
            "isodiff/dwm-forge",
            "--api-base",
            api_base,
            "--savedconfig-path",
            str(savedconfig),
            "--distdir",
            str(distdir),
            "--ebuild-path",
            str(ebuild),
            "--dry-run",
        )
        assert result.returncode == 0, result.stderr
        assert "DRY_RUN" in result.stdout
        assert not savedconfig.exists()
        assert not distdir.exists()

        artifacts[ARCHIVE_NAME] = b"corrupt artifact bytes\n"
        corrupted = run_cli(
            mock_repo.root,
            "sync-gentoo",
            "--repo",
            "isodiff/dwm-forge",
            "--api-base",
            api_base,
            "--savedconfig-path",
            str(savedconfig),
            "--distdir",
            str(distdir),
            "--ebuild-path",
            str(ebuild),
            "--dry-run",
        )

    assert corrupted.returncode != 0
    assert "SHA-256 mismatch" in corrupted.stderr
