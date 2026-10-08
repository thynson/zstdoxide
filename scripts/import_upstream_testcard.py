#!/usr/bin/env python3
"""Import pinned official example/kernel byte fixtures, with attribution."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess

REVISION = "f8745da6ff1ad1e7bab384bd1f9d742439278e99"
ROOT = Path(__file__).resolve().parents[1]
DESTINATION = ROOT / "tests" / "upstream" / "testcard"
EXAMPLES = "build/single_file_libs/examples"
SPECS = [
    (f"{EXAMPLES}/testcard-dxt1.inl", "testcard.dxt1", "CC0-1.0"),
    (f"{EXAMPLES}/testcard-zstd.inl", "testcard.zst", "CC0-1.0"),
    ("contrib/linux-kernel/test/static_test.c", "kernel-empty.zst", "BSD-3-Clause"),
]


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("upstream", type=Path)
    parser.add_argument("--zstd", default=os.environ.get("ZSTD_BIN", "zstd"))
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    revision = subprocess.check_output(["git", "-C", str(args.upstream), "rev-parse", "HEAD"], text=True).strip()
    if revision != REVISION:
        parser.error(f"expected source {REVISION}; found {revision}")
    subprocess.run(["git", "-C", str(args.upstream), "diff", "--quiet", "HEAD", "--", EXAMPLES, SPECS[-1][0]], check=True)
    if "v1.5.7" not in subprocess.check_output([args.zstd, "--version"], text=True):
        parser.error("official zstd v1.5.7 executable is required")
    outputs = {}
    manifest = {"upstream_revision": REVISION, "fixtures": []}
    for source, target, license_name in SPECS:
        text = (args.upstream / source).read_text()
        initializer = text
        lines = [1, len(text.splitlines())]
        if target == "kernel-empty.zst":
            match = re.search(r"kEmptyZstdFrame\[\]\s*=\s*\{(.*?)\};", text, re.DOTALL)
            if not match:
                raise RuntimeError("kernel fixture declaration missing")
            initializer = match.group(1)
            lines = [text.count("\n", 0, match.start()) + 1, text.count("\n", 0, match.end()) + 1]
        values = [token.strip() for token in initializer.split(",") if token.strip()]
        if not all(re.fullmatch(r"0x[0-9a-fA-F]{1,2}", token) for token in values):
            raise RuntimeError(f"unexpected byte initializer syntax: {source}")
        data = bytes(int(token, 16) for token in values)
        outputs[target] = data
        manifest["fixtures"].append({"path": target, "source": source, "source_lines": lines,
            "source_sha256": sha256(text.encode()), "size": len(data), "sha256": sha256(data), "license": license_name})
    for compressed, expected in [("testcard.zst", outputs["testcard.dxt1"]), ("kernel-empty.zst", b"")]:
        actual = subprocess.check_output([args.zstd, "-q", "-d", "-c"], input=outputs[compressed])
        if actual != expected:
            raise RuntimeError(f"official output mismatch for {compressed}")
    outputs["manifest.json"] = (json.dumps(manifest, indent=2) + "\n").encode()
    for relative, data in outputs.items():
        path = DESTINATION / relative
        if args.check:
            if path.read_bytes() != data:
                raise RuntimeError(f"fixture mismatch: {relative}")
        else:
            path.write_bytes(data)
    print(f"{'Verified' if args.check else 'Imported'} three official fixtures")


if __name__ == "__main__":
    main()
