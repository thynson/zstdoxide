#!/usr/bin/env python3
"""Project frozen DoubleFast inputs/observations into dependency-free Rust records."""

from pathlib import Path
import argparse
import hashlib
import json
import struct

ROOT = Path(__file__).resolve().parents[1]
DESTINATION = ROOT / "tests/fixtures/reference-double-fast"
CHECKERS = {
    "scripts/check_double_fast_tiny.py": "5b5333e4d09332fea7bdccd3b957afb5b015dabe04727ee4abb1cf0978d8258e",
    "scripts/check_double_fast_families.py": "f8bdf18cd81f5b4087e123d53ceab2e787943ea797e916df7df928b7cac2b212",
}


def require(valid, reason):
    if not valid:
        raise ValueError(reason)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def pack_cases(cases):
    result = bytearray(b"ZDFST02\0" + struct.pack("<I", len(cases)))
    for case, row in cases:
        body = bytearray()

        def numbers(width, values):
            for value in values:
                body.extend(int(value).to_bytes(width, "little"))

        def blob(value):
            numbers(4, [len(value)])
            body.extend(value)

        name = case.name.encode("ascii")
        numbers(2, [len(name)])
        body.extend(name)
        numbers(1, [case.window_log, case.long_log, case.short_log, case.search_log,
                    case.min_match, 0, 0, 0])
        numbers(4, [case.target, case.prefix, case.floor, *case.repeats,
                    *row["parser_repeats"], *row["wire_repeats"], row["trailing_literals"],
                    row["initial_current"], row["ordinary_floor"], row["repeat_floor"]])
        numbers(1, [row["long_length_type"]])
        numbers(4, [row["long_length_position"]])
        blob(case.source)
        numbers(8, [int(row["source_fnv1a64"], 16)])
        for seeds in (case.long_seeds, case.short_seeds):
            numbers(4, [len(seeds), *seeds])
        blob(bytes.fromhex(row["stored_literals_hex"]))
        numbers(4, [len(row["sequences"])])
        for sequence in row["sequences"]:
            numbers(4, [sequence[key] for key in ("raw_ll", "raw_ml_base", "ll", "ml", "distance", "off_base")])
        for key in ("long_table", "short_table"):
            table = row[key]
            numbers(4, [len(table["before"])])
            for entry in table["before"]:
                numbers(4, entry)
            numbers(4, [len(table["changes"])])
            for entry in table["changes"]:
                numbers(4, entry)
            numbers(8, [int(table["before_le32_fnv1a64"], 16), int(table["after_le32_fnv1a64"], 16)])
        result.extend(struct.pack("<I", len(body)))
        result.extend(body)
    return bytes(result)


def projection():
    # Fixed checker identities are verified before importing them. Each checker
    # separately pins its generator, source recipes and complete C observations.
    for name, digest in CHECKERS.items():
        require(sha(ROOT / name) == digest, "frozen checker identity: " + name)
    import check_double_fast_tiny as tiny
    import check_double_fast_families as families
    tiny_rows = tiny.check(tiny.COLLECTION)
    oracle = tiny.load_oracle()
    cases = list(zip(oracle.recipes(), (rows[1] for rows in tiny_rows)))
    families.check(families.COLLECTION)
    oracle, wider = families.load_cases()
    for case in wider:
        rows = [oracle.decode_json(line) for line in
                (families.COLLECTION / ("normal-" + case.name + ".stdout")).read_bytes().splitlines()]
        cases.append((case, rows[1]))
    require(len(cases) == len({case.name for case, _ in cases}) == 184, "combined recipe cardinality")
    data = pack_cases(cases)
    provenance = {
        "schema_version": 1, "binary_format": "ZDFST02: bounded little-endian test records",
        "recipes": 184, "represented_padding_observations": 368,
        "upstream_revision": oracle.REVISION,
        "generator": "scripts/pack_double_fast_parser_fixtures.py", "generator_sha256": sha(Path(__file__)),
        "checkers": CHECKERS,
        "source_collections": [{"path": str(module.COLLECTION.relative_to(ROOT)),
                                "catalog_sha256": module.CATALOG_SHA256,
                                "manifest_sha256": sha(module.COLLECTION / "manifest.json")}
                               for module in (tiny, families)],
        "fixture": {"path": "cases.bin", "bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()},
        "policy": "Test-only projection of both padding-equivalent constructed direct observations; no new C execution, ordinary frames or chronological trace.",
    }
    return data, (json.dumps(provenance, sort_keys=True, indent=2) + "\n").encode()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    action = parser.add_mutually_exclusive_group(required=True)
    action.add_argument("--check", action="store_true")
    action.add_argument("--write", action="store_true")
    args = parser.parse_args()
    data, metadata = projection()
    if args.write:
        DESTINATION.mkdir(parents=True, exist_ok=True)
        (DESTINATION / "cases.bin").write_bytes(data)
        (DESTINATION / "provenance.json").write_bytes(metadata)
    else:
        require((DESTINATION / "cases.bin").read_bytes() == data, "packed observations changed")
        require((DESTINATION / "provenance.json").read_bytes() == metadata, "packed provenance changed")
    print(f"{'Wrote' if args.write else 'Verified'} 184 DoubleFast records ({len(data)} bytes).")


if __name__ == "__main__":
    main()
