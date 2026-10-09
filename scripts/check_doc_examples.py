#!/usr/bin/env python3
"""Compile standalone usage examples with Cargo's actual library artifacts.

The pinned compiler emits a separate metadata file. Passing an rlib alone can
make every compile-fail example pass for the wrong reason. Resolve both paths
from compiler-artifact messages and require positive examples to pass as well.
"""

import argparse
import json
from pathlib import Path
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[1]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("documents", nargs="*", default=[
        "docs/dictionary-usage.md", "docs/streaming-usage.md", "docs/allocator-usage.md",
    ])
    parser.add_argument("--all-features", action="store_true")
    args = parser.parse_args()
    build = ["cargo", "build", "--locked", "--lib", "--message-format=json"]
    if args.all_features:
        build.append("--all-features")
    result = subprocess.run(build, cwd=ROOT, stdout=subprocess.PIPE, text=True, check=False)
    if result.returncode:
        return result.returncode
    artifacts = []
    for line in result.stdout.splitlines():
        record = json.loads(line)
        target = record.get("target", {})
        if (record.get("reason") == "compiler-artifact"
                and target.get("name") == "zstdoxide" and target.get("kind") == ["lib"]):
            artifacts = [Path(name) for name in record["filenames"]
                         if Path(name).suffix in (".rlib", ".rmeta")]
    if not artifacts or not any(path.suffix == ".rlib" for path in artifacts):
        print("Cargo did not report the zstdoxide library artifacts", file=sys.stderr)
        return 1
    if not all(path.is_file() for path in artifacts):
        print("A reported zstdoxide artifact is missing", file=sys.stderr)
        return 1
    dependencies = []
    for path in artifacts:
        dependencies.extend(["--extern", f"zstdoxide={path}"])
    for document in args.documents:
        path = ROOT / document
        if not path.is_file():
            print(f"Missing usage document: {path}", file=sys.stderr)
            return 1
        print(f"Checking {document}", flush=True)
        command = ["rustdoc", "--edition", "2024", "--test", str(path), *dependencies]
        checked = subprocess.run(command, cwd=ROOT, check=False)
        if checked.returncode:
            return checked.returncode
    return 0


if __name__ == "__main__":
    sys.exit(main())
