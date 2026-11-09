#!/usr/bin/env python3
"""Set the workspace version in Cargo.toml and in Cargo.lock.

Usage: python3 ops/local/bump-version.py <version>

`just bump <version>` runs it. It does the work of `cargo set-version --workspace`, so
the repo needs no cargo-edit: it replaces the `version` line of `[workspace.package]`,
then runs `cargo update --workspace --offline`, which writes the new version of each
workspace member into Cargo.lock and changes no dependency.
"""

import re
import subprocess
import sys
from pathlib import Path

VERSION = re.compile(r"^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$")


def set_workspace_version(text: str, version: str) -> str:
    """The manifest text with the [workspace.package] version replaced."""
    out = []
    section = None
    done = False
    for line in text.splitlines(keepends=True):
        header = re.match(r"^\s*\[([^\]]+)\]\s*$", line)
        if header:
            section = header.group(1).strip()
        elif section == "workspace.package" and not done:
            match = re.match(r'^(\s*version\s*=\s*)"[^"]*"(.*)$', line, re.DOTALL)
            if match:
                line = f'{match.group(1)}"{version}"{match.group(2)}'
                done = True
        out.append(line)
    if not done:
        raise SystemExit("error: Cargo.toml has no version in [workspace.package]")
    return "".join(out)


def main() -> None:
    if len(sys.argv) != 2 or not VERSION.match(sys.argv[1]):
        raise SystemExit("usage: bump-version.py <major.minor.patch>")
    version = sys.argv[1]
    root = Path(__file__).resolve().parents[2]
    manifest = root / "Cargo.toml"
    manifest.write_text(set_workspace_version(manifest.read_text(), version))
    subprocess.run(
        ["cargo", "update", "--workspace", "--offline"], cwd=root, check=True
    )
    print(f"workspace version: {version}")


if __name__ == "__main__":
    main()
