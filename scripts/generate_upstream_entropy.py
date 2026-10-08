#!/usr/bin/env python3
"""Reproduce independent entropy decoder vectors with the pinned C oracle."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile

REVISION = "f8745da6ff1ad1e7bab384bd1f9d742439278e99"
ROOT = Path(__file__).resolve().parents[1]
DESTINATION = ROOT / "tests/upstream/entropy-data"
GENERATOR = ROOT / "scripts/oracles/entropy_vectors.c"
SOURCES = [
    "tests/fuzz/fse_read_ncount.c", "tests/fuzz/huf_decompress.c",
    "tests/fuzz/huf_round_trip.c", "lib/common/entropy_common.c",
    "lib/compress/fse_compress.c", "lib/compress/huf_compress.c",
    "lib/decompress/huf_decompress.c",
]


def digest(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("upstream", type=Path)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    upstream = args.upstream.resolve()
    revision = subprocess.check_output(["git", "-C", str(upstream), "rev-parse", "HEAD"], text=True).strip()
    if revision != REVISION:
        parser.error(f"expected {REVISION}, found {revision}")
    subprocess.run(["git", "-C", str(upstream), "diff", "--quiet", "HEAD", "--", "lib", "tests/fuzz"], check=True)
    outputs = {}
    with tempfile.TemporaryDirectory(prefix="zstdoxide-entropy-") as temporary:
        executable = Path(temporary) / "entropy-vectors"
        subprocess.run([
            os.environ.get("CC", "cc"), "-std=c99", "-O2", "-Wall", "-Wextra", "-Werror",
            "-I", str(upstream / "lib/common"), "-I", str(upstream / "lib"),
            str(GENERATOR), str(upstream / "lib/libzstd.a"), "-o", str(executable),
        ], check=True)
        for name in ["fse", "huffman"]:
            outputs[f"{name}.bin"] = subprocess.check_output([str(executable), name])
    manifest = {
        "upstream_revision": REVISION,
        "generator": str(GENERATOR.relative_to(ROOT)),
        "generator_sha256": digest(GENERATOR.read_bytes()),
        "source_sha256": {name: digest((upstream / name).read_bytes()) for name in SOURCES},
        "fixtures": {name: {"size": len(data), "sha256": digest(data)} for name, data in outputs.items()},
    }
    outputs["manifest.json"] = (json.dumps(manifest, indent=2) + "\n").encode()
    DESTINATION.mkdir(parents=True, exist_ok=True)
    for name, data in outputs.items():
        path = DESTINATION / name
        if args.check:
            if path.read_bytes() != data:
                raise RuntimeError(f"oracle reproducibility mismatch: {name}")
        else:
            path.write_bytes(data)
    print(f"{'Verified' if args.check else 'Generated'} 512 FSE and 30 Huffman vectors")


if __name__ == "__main__":
    main()
