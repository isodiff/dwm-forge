from __future__ import annotations

import hashlib
import re
import sys
import tomllib
from pathlib import Path

PATCH_NAME = re.compile(r"^[0-9]{2}-[a-z0-9-]+\.diff$")
PACKAGES_WRITE = re.compile(r"(?:^|[,{\s])packages\s*:\s*write(?:\s|[,}]|$)")


def strip_yaml_comment(line: str) -> str:
    quote: str | None = None
    escaped = False
    for index, character in enumerate(line):
        if escaped:
            escaped = False
            continue
        if character == "\\" and quote == '"':
            escaped = True
            continue
        if character in {"'", '"'}:
            if quote is None:
                quote = character
            elif quote == character:
                quote = None
            continue
        if character == "#" and quote is None and (index == 0 or line[index - 1].isspace()):
            return line[:index].rstrip()
    return line.rstrip()


def structural_yaml_check(contents: str, workflow: Path) -> tuple[bool, set[str], list[str]]:
    """Check the common YAML structure used by GitHub workflow files without PyYAML."""
    errors: list[str] = []
    top_permissions_empty = False
    package_write_jobs: set[str] = set()
    jobs_indent: int | None = None
    current_job: str | None = None
    permissions_indent: int | None = None
    block_scalar_indent: int | None = None
    flow_stack: list[tuple[str, int]] = []

    for line_number, raw_line in enumerate(contents.splitlines(), start=1):
        if "\t" in raw_line[: len(raw_line) - len(raw_line.lstrip())]:
            errors.append(f"{workflow}:{line_number}: tabs are not valid indentation")
            continue

        indent = len(raw_line) - len(raw_line.lstrip(" "))
        line = strip_yaml_comment(raw_line.lstrip(" "))
        if not line or line in {"---", "..."}:
            continue
        if block_scalar_indent is not None:
            if indent > block_scalar_indent:
                continue
            block_scalar_indent = None

        is_sequence_entry = line == "-" or line.startswith("- ")
        structural = line[2:].lstrip() if line.startswith("- ") else line
        if not is_sequence_entry and ":" not in structural:
            errors.append(f"{workflow}:{line_number}: expected a YAML mapping or sequence entry")
            continue
        if ":" in structural and not structural.split(":", 1)[0].strip():
            errors.append(f"{workflow}:{line_number}: empty YAML mapping key")

        quote: str | None = None
        escaped = False
        for character in line:
            if escaped:
                escaped = False
                continue
            if character == "\\" and quote == '"':
                escaped = True
                continue
            if character in {"'", '"'}:
                if quote is None:
                    quote = character
                elif quote == character:
                    quote = None
                continue
            if quote is None:
                if character in "[{":
                    flow_stack.append((character, line_number))
                elif character in "]}":
                    expected = "[" if character == "]" else "{"
                    if not flow_stack or flow_stack[-1][0] != expected:
                        errors.append(f"{workflow}:{line_number}: unbalanced YAML flow delimiter")
                        break
                    flow_stack.pop()
        if quote is not None:
            errors.append(f"{workflow}:{line_number}: unterminated quoted YAML scalar")

        mapping_line = line
        if mapping_line.startswith("- "):
            mapping_line = mapping_line[2:].lstrip()
        key, separator, value = mapping_line.partition(":")
        if not separator:
            continue
        value = value.strip()
        if indent == 0 and key == "permissions":
            top_permissions_empty = value in {"{}", "{ }"}
        if indent == 0 and key == "jobs":
            jobs_indent = indent
            current_job = None
            permissions_indent = None
            continue
        if jobs_indent is not None and indent <= jobs_indent:
            jobs_indent = None
            current_job = None
            permissions_indent = None
        elif jobs_indent is not None:
            if indent == jobs_indent + 2 and key and key != "permissions":
                current_job = key.strip("'\"")
                permissions_indent = None
            elif current_job is not None and indent == jobs_indent + 4 and key == "permissions":
                if PACKAGES_WRITE.search(value):
                    package_write_jobs.add(current_job)
                permissions_indent = indent if not value else None
            elif (
                current_job is not None
                and permissions_indent is not None
                and indent > permissions_indent
                and key == "packages"
                and value.strip("'\"") == "write"
            ):
                package_write_jobs.add(current_job)

        if value in {"|", ">", "|-", ">-", "+", "|+", ">+"}:
            block_scalar_indent = indent

    for _, line_number in flow_stack:
        errors.append(f"{workflow}:{line_number}: unclosed YAML flow delimiter")
    return top_permissions_empty, package_write_jobs, errors


def validate_workflows(root: Path) -> tuple[list[str], int]:
    errors: list[str] = []
    workflow_files = sorted((root / ".github" / "workflows").glob("*.yml"))
    try:
        import yaml
    except ImportError:
        yaml = None

    for workflow in workflow_files:
        try:
            contents = workflow.read_text(encoding="utf-8")
        except OSError as error:
            errors.append(f"cannot read workflow {workflow}: {error}")
            continue

        if yaml is None:
            permissions_empty, package_write_jobs, syntax_errors = structural_yaml_check(
                contents, workflow
            )
            errors.extend(syntax_errors)
            if not permissions_empty:
                errors.append(f"{workflow}: top-level `permissions: {{}}` is required")
        else:
            try:
                document = yaml.safe_load(contents)
            except yaml.YAMLError as error:
                errors.append(f"{workflow}: invalid YAML: {error}")
                continue
            if not isinstance(document, dict):
                errors.append(f"{workflow}: workflow document must be a YAML mapping")
                continue
            if document.get("permissions") != {}:
                errors.append(f"{workflow}: top-level `permissions: {{}}` is required")
            jobs = document.get("jobs", {})
            if not isinstance(jobs, dict):
                errors.append(f"{workflow}: `jobs` must be a mapping")
                continue
            package_write_jobs = {
                str(job_name)
                for job_name, job in jobs.items()
                if isinstance(job, dict)
                and isinstance(job.get("permissions"), dict)
                and job["permissions"].get("packages") == "write"
            }

        if len(package_write_jobs) > 1:
            names = ", ".join(sorted(package_write_jobs))
            errors.append(f"{workflow}: more than one job declares `packages: write` ({names})")

    return errors, len(workflow_files)


def validate_dockerfile(root: Path) -> list[str]:
    dockerfile = root / "Dockerfile"
    if not dockerfile.is_file():
        return ["Dockerfile is missing"]
    try:
        lines = dockerfile.read_text(encoding="utf-8").splitlines()
    except OSError as error:
        return [f"cannot read Dockerfile: {error}"]
    if not any(line.lstrip().startswith("FROM ") for line in lines):
        return ["Dockerfile must contain a `FROM ` instruction"]
    if not any(line.lstrip().startswith("ENTRYPOINT ") for line in lines):
        return ["Dockerfile must contain an `ENTRYPOINT ` instruction"]
    return []


def validate_lockfile_and_patches(root: Path) -> list[str]:
    errors: list[str] = []
    lock_path = root / "config" / "dwm-lock.toml"
    patches_dir = root / "patches"
    try:
        lockfile = tomllib.loads(lock_path.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError) as error:
        return [f"cannot parse {lock_path}: {error}"]

    patch_hashes = lockfile.get("patches")
    if not isinstance(patch_hashes, dict):
        errors.append(f"{lock_path}: `[patches]` must be a mapping")
        patch_hashes = {}

    if not patches_dir.is_dir():
        return [*errors, f"patch directory is missing: {patches_dir}"]

    patch_files = sorted(path for path in patches_dir.iterdir() if path.is_file())
    for patch_file in patch_files:
        if PATCH_NAME.fullmatch(patch_file.name) is None:
            errors.append(f"patch filename does not match required format: {patch_file.name}")
        try:
            patch_contents = patch_file.read_text(encoding="utf-8")
        except (OSError, UnicodeError) as error:
            errors.append(f"cannot read patch {patch_file}: {error}")
            continue
        patch_lines = patch_contents.splitlines()
        if not any(line.startswith("--- ") for line in patch_lines) or not any(
            line.startswith("+++ ") for line in patch_lines
        ):
            errors.append(f"patch is missing unified diff headers: {patch_file.name}")

    for patch_name, expected_hash in patch_hashes.items():
        patch_path = patches_dir / patch_name
        if not patch_path.is_file():
            errors.append(f"lockfile patch is missing: {patch_name}")
            continue
        if not isinstance(expected_hash, str):
            errors.append(f"lockfile hash for {patch_name} must be a string")
            continue
        actual_hash = hashlib.sha256(patch_path.read_bytes()).hexdigest()
        if actual_hash != expected_hash:
            errors.append(
                f"SHA-256 mismatch for {patch_name}: expected {expected_hash}, got {actual_hash}"
            )

    for patch_file in patch_files:
        if patch_file.name not in patch_hashes:
            errors.append(f"patch is not listed in dwm-lock.toml: {patch_file.name}")
    return errors


def main() -> int:
    root = Path(__file__).resolve().parents[1]
    errors: list[str] = []
    workflow_errors, workflow_count = validate_workflows(root)
    errors.extend(workflow_errors)
    errors.extend(validate_dockerfile(root))
    errors.extend(validate_lockfile_and_patches(root))

    if errors:
        for error in errors:
            print(f"ERROR: {error}", file=sys.stderr)
        return 1

    print(f"REPOSITORY_VALIDATION_OK (workflows checked: {workflow_count})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
