#!/usr/bin/env python3
"""Import zstd v1.5.7 golden assets and generate independently verified oracles.

Run with --check to verify committed fixture bytes and manifest reproducibility.
The official CLI is used only during fixture generation, never by codec code.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess

REVISION = "f8745da6ff1ad1e7bab384bd1f9d742439278e99"
ROOT = Path(__file__).resolve().parents[1]
DESTINATION = ROOT / "tests" / "upstream" / "data"
DIRECTORIES = (
    "golden-decompression",
    "golden-decompression-errors",
    "golden-compression",
    "golden-dictionaries",
)


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("upstream", type=Path, help="external zstd v1.5.7 git checkout")
    parser.add_argument("--zstd", default=os.environ.get("ZSTD_BIN", "zstd"))
    parser.add_argument("--check", action="store_true", help="verify without writing")
    args = parser.parse_args()
    actual_revision = subprocess.check_output(
        ["git", "-C", str(args.upstream), "rev-parse", "HEAD"], text=True
    ).strip()
    if actual_revision != REVISION:
        parser.error(f"expected upstream {REVISION}; found {actual_revision}")
    subprocess.run(
        ["git", "-C", str(args.upstream), "diff", "--quiet", "HEAD", "--"]
        + [f"tests/{directory}" for directory in DIRECTORIES],
        check=True,
    )
    version = subprocess.check_output([args.zstd, "--version"], text=True)
    if "v1.5.7" not in version:
        parser.error(f"expected zstd v1.5.7; found {version.strip()}")
    outputs = {}
    manifest = {"upstream_revision": REVISION, "upstream_version": "1.5.7", "files": []}

    def record(path, data, **metadata):
        outputs[path] = data
        manifest["files"].append(
            {"path": path, "size": len(data), "sha256": sha256(data), **metadata}
        )

    def oracle(arguments, data):
        return subprocess.check_output([args.zstd, *arguments], input=data)

    for directory in DIRECTORIES:
        for source in sorted((args.upstream / "tests" / directory).iterdir()):
            if not source.is_file() or source.name.startswith("."):
                continue
            relative = f"{directory}/{source.name}"
            data = source.read_bytes()
            record(relative, data, origin=f"tests/{relative}")
            if directory == "golden-decompression":
                decoded = oracle(["-q", "-d", "-c"], data)
                expected = {"size": len(decoded), "sha256": sha256(decoded)}
                if not decoded or all(byte == decoded[0] for byte in decoded):
                    expected["repeat_byte"] = decoded[0] if decoded else 0
                else:
                    expected["hex"] = decoded.hex()
                manifest["files"][-1]["expected_output"] = expected
            elif directory == "golden-decompression-errors":
                result = subprocess.run(
                    [args.zstd, "-q", "-d", "-c"], input=data, capture_output=True
                )
                if result.returncode == 0:
                    raise RuntimeError(f"oracle accepted malformed fixture {relative}")
                manifest["files"][-1]["expected_error"] = "corrupt input"
            elif directory == "golden-compression":
                arguments = ["-q", "-c", "-3", "--single-thread", "--check", f"--stream-size={len(data)}"]
                encoded = oracle(arguments, data)
                if oracle(["-q", "-d", "-c"], encoded) != data:
                    raise RuntimeError(f"oracle round-trip mismatch for {relative}")
                record(
                    f"generated/{source.name}.zst", encoded,
                    derived_from=relative, oracle_arguments=arguments,
                    expected_output={"size": len(data), "sha256": sha256(data)},
                )

    dictionary = args.upstream / "tests" / "golden-dictionaries" / "http-dict-missing-symbols"
    http = (args.upstream / "tests" / "golden-compression" / "http").read_bytes()
    arguments = ["-q", "-c", "-3", "--single-thread", "--check", f"--stream-size={len(http)}", "-D", str(dictionary)]
    encoded = oracle(arguments, http)
    if oracle(["-q", "-d", "-c", "-D", str(dictionary)], encoded) != http:
        raise RuntimeError("oracle dictionary round-trip mismatch")
    # Store a portable path instead of this machine's checkout location.
    arguments[-1] = "UPSTREAM/tests/golden-dictionaries/http-dict-missing-symbols"
    record(
        "generated/http-dictionary.zst", encoded,
        derived_from="golden-compression/http",
        dictionary="golden-dictionaries/http-dict-missing-symbols",
        oracle_arguments=arguments,
        expected_output={"size": len(http), "sha256": sha256(http)},
    )
    outputs["manifest.json"] = (json.dumps(manifest, indent=2) + "\n").encode()
    if args.check:
        for relative, data in outputs.items():
            if (DESTINATION / relative).read_bytes() != data:
                raise RuntimeError(f"fixture mismatch: {relative}")
        print(f"Verified {len(manifest['files'])} original/derived assets and manifest")
    else:
        for relative, data in outputs.items():
            target = DESTINATION / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(data)
        print(f"Imported {len(manifest['files'])} original/derived assets and manifest")


if __name__ == "__main__":
    main()
