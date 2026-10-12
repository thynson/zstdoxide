#!/usr/bin/env python3
"""Check the pinned source-only DoubleFast family envelope; no C execution."""

from pathlib import Path
import argparse
import hashlib
import json
import struct
import sys

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "tests/fixtures/reference-double-fast-recipes/recipes.json"
MANIFEST_SHA256 = "b2e9b2c8302e99a5974ea5d9d8fba67986c4f3cd94a34810631fa56c04d811e7"
GENERATOR = "scripts/oracles/double_fast_families.py"
PINS = {GENERATOR: "56fa1aa0df89cb173c596c94d40ac8aa08c183b2dc8af50c9cc128308877e15e",
        "scripts/oracles/double_fast_direct.py": "5280b94e6e10e34a263b7ae1b31a24026adfd92327767b776e81344362604065",
        "scripts/oracles/double_fast_direct.c": "e20d452423a3f913831ac04f6edd79be3d0d18fb0fe6609357ef4ea47816d9d4"}


def require(valid, reason):
    if not valid:
        raise ValueError(reason)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def load_families():
    for name, digest in PINS.items():
        require(sha(ROOT / name) == digest, "frozen source identity: " + name)
    from oracles import double_fast_families
    return double_fast_families


def check_packet(case, data):
    """Independently decode fields and bounds rather than trusting serialization."""
    require(type(data) is bytes and 132 <= len(data) <= 2236548, "packet size")
    require(data[:8] == b"ZDFST01\0", "packet magic")
    identity = data[8:72]
    name = identity.split(b"\0", 1)[0]
    require(1 <= len(name) <= 63 and all(byte in b"abcdefghijklmnopqrstuvwxyz0123456789_-" for byte in name), "packet id")
    require(identity == name.ljust(64, b"\0") and name.decode() == case.recipe.name, "packet id padding")
    values = struct.unpack_from("<15I", data, 72)
    prefix, block, window, long_log, short_log, search, minimum, target, floor = values[:9]
    repeats, long_count, short_count, reserved = values[9:12], *values[12:]
    require(prefix <= 2 << 20 and 7 <= block <= 128 << 10 and 10 <= window <= 21, "source/window extent")
    require(block <= 1 << window and floor <= prefix, "window/floor extent")
    require(6 <= long_log <= 18 and 6 <= short_log <= 18 and 1 <= search <= 29, "search dimensions")
    require(3 <= minimum <= 7 and target <= 131072 and all(repeats), "match/repeat fields")
    require(long_count <= 1024 and short_count <= 1024 and reserved == 0, "seed counts/reserved")
    start = 132 + 4 * (long_count + short_count)
    require(len(data) == start + prefix + block, "exact packet extent")
    seeds = struct.unpack_from("<" + "I" * (long_count + short_count), data, 132)
    long_seeds, short_seeds = seeds[:long_count], seeds[long_count:]
    require(all(p + 8 <= prefix for p in long_seeds), "long seed key")
    require(all(p + max(4, minimum) <= prefix for p in short_seeds), "short seed key")
    r = case.recipe
    require(values == (r.prefix, r.block, r.window_log, r.long_log, r.short_log, r.search_log,
                       r.min_match, r.target, r.floor, *r.repeats, len(r.long_seeds), len(r.short_seeds), 0), "encoded scalar identity")
    require(long_seeds == r.long_seeds and short_seeds == r.short_seeds and data[start:] == r.source, "encoded seed/source identity")


def check_manifest(manifest):
    families = load_families()
    require(manifest.is_file() and manifest.stat().st_size <= 2 << 20, "bounded manifest")
    require(sha(manifest) == MANIFEST_SHA256, "fixed pre-capture input manifest")
    actual = families.direct.decode_json(manifest.read_bytes())
    cases = families.recipes()
    expected = {"schema_version": 1, "scope": "source-only hypotheses; no new C outputs",
                "generator": GENERATOR, "generator_sha256": PINS[GENERATOR],
                "direct_generator_sha256": PINS["scripts/oracles/double_fast_direct.py"],
                "helper_sha256": PINS["scripts/oracles/double_fast_direct.c"],
                "families": families.FAMILY_COUNTS, "cases": [case.metadata() for case in cases]}
    require(families.direct.exact(actual, expected), "independent family identities")
    for case in cases:
        check_packet(case, case.recipe.packet())
    require(sum(len(case.recipe.source) for case in cases) == 5242885, "aggregate source extent")
    require(max(len(case.recipe.packet()) for case in cases) == 2228372, "maximum packet extent")
    return cases


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", required=True)
    parser.add_argument("--manifest", type=Path, default=MANIFEST)
    args = parser.parse_args()
    check_manifest(args.manifest)
    print("Validated 168 source-only DoubleFast recipes in 14 families; no C observation was generated.")


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, KeyError, TypeError) as error:
        print("ERROR: " + str(error), file=sys.stderr)
        raise SystemExit(1)
