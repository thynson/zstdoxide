#!/usr/bin/env python3
"""Reproduce the official streaming offset/window sweep with a pinned C oracle."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile

REVISION = "f8745da6ff1ad1e7bab384bd1f9d742439278e99"
ROOT = Path(__file__).resolve().parents[1]
GENERATOR = ROOT / "scripts/oracles/window_sequences.c"
DESTINATION = ROOT / "tests/upstream/window-sequences"
SOURCES = ["tests/zstreamtest.c", "lib/zstd.h", "lib/compress/zstd_compress.c"]


def digest(data):
    return hashlib.sha256(data).hexdigest()


def zero_hash(size):
    result = hashlib.sha256()
    block = bytes(65536)
    while size:
        count = min(size, len(block))
        result.update(block[:count])
        size -= count
    return result.hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("upstream", type=Path)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    upstream = args.upstream.resolve()
    revision = subprocess.check_output(["git", "-C", str(upstream), "rev-parse", "HEAD"], text=True).strip()
    if revision != REVISION:
        parser.error(f"expected {REVISION}, found {revision}")
    subprocess.run(["git", "-C", str(upstream), "diff", "--quiet", "HEAD", "--", "lib", "tests/zstreamtest.c"], check=True)
    outputs = {}
    fixtures = {}
    with tempfile.TemporaryDirectory(prefix="zstdoxide-windows-") as temporary:
        executable = Path(temporary) / "window-sequences"
        subprocess.run([
            os.environ.get("CC", "cc"), "-std=c99", "-O2", "-Wall", "-Wextra", "-Werror",
            "-I", str(upstream / "lib"), str(GENERATOR),
            str(upstream / "lib/libzstd.a"), "-o", str(executable),
        ], check=True)
        for log in range(10, 30):
            name = f"window-{log:02}.zst"
            encoded = subprocess.check_output([str(executable), str(log)])
            size = (1 << log) + 10 * 9999
            outputs[name] = encoded
            fixtures[name] = {
                "size": len(encoded), "sha256": digest(encoded), "window_log": log,
                "sequence_count": 10000, "seed": 0x12345678,
                "expected_output": {"repeat_byte": 0, "size": size, "sha256": zero_hash(size)},
            }
    metadata = {
        "upstream_revision": REVISION, "source_group": "tests/zstreamtest.c:1652",
        "generator": str(GENERATOR.relative_to(ROOT)),
        "generator_sha256": digest(GENERATOR.read_bytes()),
        "source_sha256": {name: digest((upstream / name).read_bytes()) for name in SOURCES},
        "fixtures": fixtures,
    }
    outputs["manifest.json"] = (json.dumps(metadata, indent=2) + "\n").encode()
    DESTINATION.mkdir(parents=True, exist_ok=True)
    for name, data in outputs.items():
        path = DESTINATION / name
        if args.check:
            if path.read_bytes() != data:
                raise RuntimeError(f"oracle reproducibility mismatch: {name}")
        else:
            path.write_bytes(data)
    print(f"{'Verified' if args.check else 'Generated'} 20 official window-distance frames (logs 10–29)")


if __name__ == "__main__":
    main()
