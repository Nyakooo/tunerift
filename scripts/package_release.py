#!/usr/bin/env python3
"""Package CI-built executables; never builds or ships a Python runtime."""
import argparse
import os
from pathlib import Path
import tarfile
import tomllib
import zipfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="Validate tag/version only")
    parser.add_argument("--target", choices=[
        "linux-x86_64", "linux-arm64", "windows-x86_64", "windows-arm64",
        "macos-x86_64", "macos-arm64",
    ])
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    version = tomllib.loads((root / "Cargo.toml").read_text())["package"]["version"]
    if os.environ.get("GITHUB_REF_TYPE") == "tag":
        tag = os.environ.get("GITHUB_REF_NAME", "")
        if tag != f"v{version}":
            parser.error(f"Tag {tag!r} must match Cargo.toml version v{version}")
    if args.check:
        print(f"Version OK: v{version}")
        return
    if not args.target:
        parser.error("--target is required for packaging")
    name = f"tunerift-v{version}-{args.target}"
    executable = "tunerift.exe" if args.target.startswith("windows-") else "tunerift"
    files = [(root / "target" / "release" / executable, executable)]
    files.extend((root / file, file) for file in ["README.md", "LICENSE", "RELEASE_NOTES.md"])
    for path, _ in files:
        if not path.is_file():
            parser.error(f"Required package file missing: {path}")
    dist = root / "dist"
    dist.mkdir(exist_ok=True)
    if args.target.startswith("windows-"):
        output = dist / f"{name}.zip"
        with zipfile.ZipFile(output, "w", compression=zipfile.ZIP_DEFLATED) as archive:
            for path, filename in files:
                archive.write(path, f"{name}/{filename}")
    else:
        output = dist / f"{name}.tar.gz"
        with tarfile.open(output, "w:gz") as archive:
            for path, filename in files:
                info = archive.gettarinfo(str(path), f"{name}/{filename}")
                info.mode = 0o755 if filename == executable else 0o644
                with path.open("rb") as file:
                    archive.addfile(info, file)
    print(output)


if __name__ == "__main__":
    main()
