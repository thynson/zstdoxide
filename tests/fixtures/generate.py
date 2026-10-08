#!/usr/bin/env python3
"""Regenerate original deterministic test inputs with the zstd v1.5.7 oracle."""

import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parent
ORACLE = os.environ.get("ZSTD_BIN", "zstd")
version = subprocess.check_output([ORACLE, "--version"], text=True)
if "v1.5.7" not in version:
    raise SystemExit(f"zstd v1.5.7 is required; got {version.strip()}")


def random_values(count):
    state = 0x4D595DF4D0F33173
    for _ in range(count):
        state ^= (state << 13) & ((1 << 64) - 1)
        state ^= state >> 7
        state ^= (state << 17) & ((1 << 64) - 1)
        yield state


def corpus(name):
    if name == "empty":
        return b""
    if name == "tiny":
        return b"Safe Rust, explicit ownership, checked decoding.\n"
    if name == "runs":
        return b"r" * 300000
    if name == "random":
        return bytes((value >> 24) & 255 for value in random_values(4096))
    if name == "skewed":
        return bytes(97 + min((value & -value).bit_length() - 1, 7) for value in random_values(64000))
    if name == "text":
        return b"".join(
            f"record {index:05}: safe Rust keeps ownership explicit; repeated tokens compress well; group={index % 17}.\n".encode()
            for index in range(4096)
        )
    if name == "history":
        block = bytes((value >> 24) & 255 for value in random_values(65536))
        return (block + b"|history-window|") * 5
    raise ValueError(name)


cases = [
    ("empty", 3), ("tiny", 1), ("runs", 3), ("random", 3),
    ("skewed", 3), ("skewed", 19), ("text", 1), ("text", 3),
    ("text", 9), ("text", 19), ("history", 9),
]
for name, level in cases:
    data = corpus(name)
    encoded = subprocess.check_output(
        [ORACLE, "-q", "-c", f"-{level}", "--check", f"--stream-size={len(data)}"],
        input=data,
    )
    (ROOT / f"{name}-{level}.zst").write_bytes(encoded)
    restored = subprocess.check_output([ORACLE, "-q", "-d", "-c"], input=encoded)
    assert restored == data
    print(f"{name}-{level}.zst: {len(data)} -> {len(encoded)}")

encoded = subprocess.check_output([ORACLE, "-q", "-c", "-3", "--no-check"], input=corpus("text"))
(ROOT / "text-unknown-size.zst").write_bytes(encoded)
