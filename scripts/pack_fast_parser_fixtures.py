#!/usr/bin/env python3
"""Pack frozen Fast observations for dependency-free Rust tests; never runs C."""

from pathlib import Path
import argparse
import hashlib
import json
import struct

import import_fast_parser_families as importer

ROOT = Path(__file__).resolve().parents[1]
DESTINATION = ROOT / "tests/fixtures/reference-fast"


def pack_cases(cases):
    result = bytearray(b"ZFAST03\0" + struct.pack("<I", len(cases)))
    for case in cases:
        rows = [importer.families.decode_json(line) for line in
                (importer.COLLECTION / ("normal-" + case.name + ".stdout")).read_text().splitlines()]
        row = rows[1]  # Both padding records were already checked equal.
        body = bytearray()

        def numbers(width, values):
            for value in values:
                body.extend(int(value).to_bytes(width, "little"))

        def blob(data):
            numbers(4, [len(data)])
            body.extend(data)

        name = case.name.encode("ascii")
        numbers(2, [len(name)])
        body.extend(name)
        numbers(1, [case.window_log, case.hash_log, case.min_match, 0])
        numbers(4, [case.target, case.prefix, case.prefix_floor,
                    *case.repeats, *row["parser_repeats"], *row["wire_repeats"],
                    row["trailing_literals"]])
        numbers(1, [row["long_length_type"]])
        numbers(4, [row["long_length_position"]])
        blob(case.source)
        numbers(4, [len(case.seeds), *case.seeds])
        blob(bytes.fromhex(row["stored_literals_hex"]))
        numbers(4, [len(row["sequences"])])
        for sequence in row["sequences"]:
            numbers(4, [sequence[key] for key in ("ll", "ml", "distance", "off_base")])
        numbers(4, [len(row["table_before_nonzero"])])
        for entry in row["table_before_nonzero"]:
            numbers(4, entry)
        numbers(4, [len(row["table_changes"])])
        for change in row["table_changes"]:
            numbers(4, change)
        numbers(8, [int(row["table_before_le32_fnv1a64"], 16),
                    int(row["table_after_le32_fnv1a64"], 16)])
        result.extend(struct.pack("<I", len(body)))
        result.extend(body)
    return bytes(result)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    action = parser.add_mutually_exclusive_group(required=True)
    action.add_argument("--check", action="store_true")
    action.add_argument("--write", action="store_true")
    args = parser.parse_args()
    _, cases = importer.offline_check(importer.COLLECTION)
    data = pack_cases(cases)
    provenance = {
        "schema_version": 1, "binary_format": "ZFAST03: checked little-endian test records",
        "recipes": len(cases), "represented_c_observations": 2 * len(cases),
        "upstream_revision": importer.families.REVISION,
        "source_catalog_sha256": importer.CATALOG_SHA256,
        "source_manifest_sha256": importer.sha(importer.COLLECTION / "manifest.json"),
        "generator": "scripts/pack_fast_parser_fixtures.py",
        "generator_sha256": importer.sha(Path(__file__)),
        "fixture": {"path": "cases.bin", "size": len(data), "sha256": hashlib.sha256(data).hexdigest()},
        "policy": "Derived from frozen unchanged-C observations; no new C capture, parser implementation or frame-publication claim.",
    }
    metadata = (json.dumps(provenance, indent=2, sort_keys=True) + "\n").encode()
    if args.write:
        DESTINATION.mkdir(parents=True, exist_ok=True)
        (DESTINATION / "cases.bin").write_bytes(data)
        (DESTINATION / "provenance.json").write_bytes(metadata)
    else:
        importer.require((DESTINATION / "cases.bin").read_bytes() == data, "packed C observations changed")
        importer.require((DESTINATION / "provenance.json").read_bytes() == metadata, "packed fixture provenance changed")
    print(f"{'Wrote' if args.write else 'Verified'} {len(cases)} Rust-readable Fast records ({len(data)} bytes).")


if __name__ == "__main__":
    main()
