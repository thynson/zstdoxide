#!/usr/bin/env python3
"""Bounded DoubleFast recipes and independent observation validation (no codec)."""

from dataclasses import dataclass, replace
from pathlib import Path
import hashlib
import json
import re
import struct

REVISION = "f8745da6ff1ad1e7bab384bd1f9d742439278e99"
MAX_PREFIX = 2 << 20
MAX_BLOCK = 128 << 10
MAX_PACKET = 132 + 8192 + MAX_PREFIX + MAX_BLOCK
MAX_RECORDS = 64 << 20
MAX_ALLOCATION = 64 << 20
SUFFIX = MAX_BLOCK + 512


def require(condition, reason):
    if not condition:
        raise ValueError(reason)


def exact(left, right):
    return json.dumps(left, sort_keys=True, allow_nan=False) == json.dumps(right, sort_keys=True, allow_nan=False)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def fnv(data):
    value = 0xcbf29ce484222325
    for byte in data:
        value = ((value ^ byte) * 0x100000001b3) & 0xffffffffffffffff
    return f"{value:016x}"


def decode_json(data):
    def pairs(items):
        result = {}
        for key, value in items:
            require(key not in result, "duplicate JSON field: " + key)
            result[key] = value
        return result

    def nonfinite(value):
        raise ValueError("nonfinite JSON scalar: " + value)

    return json.loads(data, object_pairs_hook=pairs, parse_constant=nonfinite)


def integer(value, lower, upper, label):
    require(type(value) is int and lower <= value <= upper, label)
    return value


def bucket(key, log):
    """Independent LE hash arithmetic, zstd_compress_internal.h:903-925."""
    width = len(key)
    require(4 <= width <= 8 and 6 <= log <= 18, "hash domain")
    word = int.from_bytes(key, "little")
    if width == 4:
        return ((word * 2654435761) & 0xffffffff) >> (32 - log)
    prime = {5: 889523592379, 6: 227718039650203, 7: 58295818150454627, 8: 0xcf1bbcdcb7a56463}[width]
    return (((word << (64 - 8 * width)) * prime) & 0xffffffffffffffff) >> (64 - log)


@dataclass(frozen=True)
class Recipe:
    name: str
    source: bytes
    prefix: int = 0
    window_log: int = 10
    long_log: int = 6
    short_log: int = 6
    search_log: int = 1
    min_match: int = 4
    target: int = 0
    floor: int = 0
    repeats: tuple = (1, 4, 8)
    long_seeds: tuple = ()
    short_seeds: tuple = ()

    @property
    def block(self):
        return len(self.source) - self.prefix

    @property
    def width(self):
        return max(4, self.min_match)

    def packet(self):
        require(type(self.name) is str and re.fullmatch("[a-z0-9_-]{1,63}", self.name), "recipe id")
        require(type(self.source) is bytes, "recipe source")
        integer(self.prefix, 0, MAX_PREFIX, "prefix extent")
        integer(self.block, 7, MAX_BLOCK, "block extent")
        integer(self.window_log, 10, 21, "window log")
        require(self.block <= 1 << self.window_log, "block exceeds window")
        for log in (self.long_log, self.short_log):
            integer(log, 6, 18, "table log")
        integer(self.search_log, 1, 29, "search log")
        integer(self.min_match, 3, 7, "minimum match")
        integer(self.target, 0, 131072, "target")
        integer(self.floor, 0, self.prefix, "prefix floor")
        require(type(self.repeats) is tuple and len(self.repeats) == 3, "repeat tuple")
        for value in self.repeats:
            integer(value, 1, 0xffffffff, "repeat cell")
        for seeds, width in ((self.long_seeds, 8), (self.short_seeds, self.width)):
            require(type(seeds) is tuple and len(seeds) <= 1024, "seed count")
            for position in seeds:
                integer(position, 0, self.prefix - width, "selected seed key outside prefix")
        scalars = (self.prefix, self.block, self.window_log, self.long_log, self.short_log,
                   self.search_log, self.min_match, self.target, self.floor, *self.repeats,
                   len(self.long_seeds), len(self.short_seeds), 0)
        packet = b"ZDFST01\0" + self.name.encode().ljust(64, b"\0") + struct.pack("<15I", *scalars)
        packet += b"".join(struct.pack("<I", p) for p in self.long_seeds + self.short_seeds) + self.source
        require(len(packet) <= MAX_PACKET, "packet cap")
        return packet

    def metadata(self):
        packet = self.packet()
        return {"id": self.name, "source_bytes": len(self.source), "source_sha256": hashlib.sha256(self.source).hexdigest(),
                "packet_bytes": len(packet), "packet_sha256": hashlib.sha256(packet).hexdigest(),
                "prefix_bytes": self.prefix, "block_bytes": self.block, "window_log": self.window_log,
                "long_hash_log": self.long_log, "short_hash_log": self.short_log, "search_log": self.search_log,
                "min_match": self.min_match, "target_length": self.target, "prefix_floor": self.floor,
                "initial_repeats": list(self.repeats), "long_seeds": list(self.long_seeds), "short_seeds": list(self.short_seeds)}


def recipes():
    """The fifteen source-only hypotheses from the reviewed initial protocol."""
    unavailable = (257, 263, 269)
    prefix = b"0123456789ABCDEF"
    period = Recipe("repeat-four", b"abcd" * 16)
    result = [Recipe("fresh-" + name, b"abcdefghij"[:size])
              for name, size in (("seven", 7), ("eight", 8), ("nine", 9), ("ten", 10))]
    result += [Recipe("prefix-" + name, prefix + prefix[:size], prefix=len(prefix),
                      repeats=unavailable, long_seeds=(0,), short_seeds=(0,))
               for name, size in (("nine", 9), ("ten", 10))]
    result += [Recipe("repeat-one", b"A" * 64), period,
               Recipe("current-long-floor", prefix * 2, prefix=16, repeats=unavailable, long_seeds=(0,)),
               Recipe("current-short-floor", b"abcdWXYZ01234567" + b"abcdQRSTuvwx1234", prefix=16,
                      repeats=unavailable, short_seeds=(0,))]
    for name, prefix, long_seed, short_seed in (
            ("later-floor", b"ABCDEFGH01234567xABCpqrs", 0, 16),
            ("later-above-floor", b"_ABCDEFGH01234567xABCpqrs", 1, 17)):
        result.append(Recipe(name, prefix + b"xABCDEFGH0123456", prefix=len(prefix), repeats=unavailable,
                             long_seeds=(long_seed,), short_seeds=(short_seed,)))
    result += [Recipe("extend-to-floor", b"abcdefghijklmnop" * 3, prefix=16,
                      repeats=unavailable, long_seeds=(4,)),
               replace(period, name="dispatch-three", min_match=3),
               replace(period, name="ignored-controls", search_log=29, target=131072)]
    require(len(result) == len({case.name for case in result}) == 15, "tiny recipe count")
    for case in result:
        case.packet()
    return result


def validate_metadata(row):
    require(type(row) is dict, "metadata object")
    fixed = {"record": "metadata", "schema_version": 1, "revision": REVISION,
             "entry_point": "ZSTD_compressBlock_doubleFast", "strategy": 2, "protocol": "ZDFST01",
             "ordinary_frames": False, "collector": False,
             "observation_kind": "sequences-and-final-tables-not-chronological-trace",
             "guard_bytes": 64, "leading_bytes": 64, "suffix_bytes": SUFFIX,
             "max_prefix_bytes": MAX_PREFIX, "max_block_bytes": MAX_BLOCK,
             "max_packet_bytes": MAX_PACKET, "max_allocation_bytes": MAX_ALLOCATION,
             "hash_read_bytes": 8, "window_start_index": 2, "minimum_match": 3,
             "wildcopy_overlength": 32, "u32_bytes": 4, "sequence_bytes": 8}
    host = {"pointer_bits", "little_endian", "u32_alignment", "sequence_alignment",
            "observation_bytes", "observation_alignment", "state_bytes", "state_alignment", "store_bytes", "store_alignment"}
    require(set(row) == set(fixed) | host, "exact metadata fields")
    require(all(exact(row[key], value) for key, value in fixed.items()), "pinned metadata constants")
    require(type(row["little_endian"]) is bool, "host endian flag")
    require(type(row["pointer_bits"]) is int and row["pointer_bits"] in (32, 64), "host pointer width")
    require(row["observation_bytes"] == row["pointer_bits"] // 4 + 8, "observation layout")
    for key in host - {"pointer_bits", "little_endian"}:
        integer(row[key], 1, 1 << 20, "host layout scalar")
        if key.endswith("alignment"):
            require(row[key] <= 64 and 64 % row[key] == 0, "fundamental alignment")
    return row


def validate_table(case, row, log, width, seeds):
    require(type(row) is dict and set(row) == {"log", "width", "entries", "before", "changes", "before_le32_fnv1a64", "after_le32_fnv1a64"}, "table fields")
    for key, value in {"log": log, "width": width, "entries": 1 << log}.items():
        require(exact(row[key], value), "table dimension")
    before = [0] * (1 << log)
    for position in seeds:
        before[bucket(case.source[position:position + width], log)] = position + 2
    require(exact(row["before"], [[i, p] for i, p in enumerate(before) if p]), "ordered seed map")
    require(type(row["changes"]) is list and len(row["changes"]) <= len(before), "bounded table changes")
    after = before.copy()
    last = -1
    for change in row["changes"]:
        require(type(change) is list and len(change) == 3, "delta shape")
        index, old, new = change
        integer(index, last + 1, len(before) - 1, "sorted unique delta bucket")
        integer(old, 0, len(case.source) + 2, "old cell")
        integer(new, 2, len(case.source) + 2 - width, "new complete key cell")
        require(old == before[index] and new != old and new - 2 >= case.prefix, "changed cell provenance")
        after[index] = new
        last = index
    for i, position in enumerate(after):
        if position:
            require(bucket(case.source[position - 2:position - 2 + width], log) == i, "final key/bucket identity")
    for label, cells in (("before", before), ("after", after)):
        require(row[label + "_le32_fnv1a64"] == fnv(struct.pack("<" + "I" * len(cells), *cells)), "complete LE32 table hash")
    return after


def resolve_repeat(repeats, off_base, ll):
    a, b, c = repeats
    if off_base > 3:
        return off_base - 3, [off_base - 3, a, b]
    index = off_base - 1 + (ll == 0)
    if index == 0:
        return a, [a, b, c]
    if index == 1:
        return b, [b, a, c]
    distance = c if index == 2 else a - 1
    require(distance > 0, "repeat-minus-one must be positive")
    return distance, [distance, a, b]


def validate_case(case, row, metadata):
    require(type(row) is dict, "case object")
    n, q = len(case.source), case.block // 3 + 1
    window = 1 << case.window_log
    ordinary_floor = max(case.floor, n - window)
    current = case.prefix + (case.prefix == ordinary_floor)
    payloads = [n, 64 + n + SUFFIX, 64 + n + SUFFIX,
                *(2 * [4 << case.long_log]), *(2 * [4 << case.short_log]),
                q * metadata["sequence_bytes"], case.block + 32, 3 * q, n, q * metadata["observation_bytes"]]
    fixed = {"record": "case", **{k: v for k, v in case.metadata().items() if k not in {"source_sha256", "packet_bytes", "packet_sha256"}},
             "source_fnv1a64": fnv(case.source), "initial_dict_limit": case.floor + 2,
             "initial_low_limit": case.floor + 2, "initial_next_to_update": 2, "loaded_dict_end": 0,
             "dictionary_state_null": True, "ordinary_floor": ordinary_floor, "initial_current": current,
             "repeat_floor": max(case.floor, current - window), "physical_leading_bytes": 64,
             "physical_suffix_bytes": SUFFIX, "sequence_capacity": q, "literal_slack_bytes": 32,
             "requested_allocation_bytes": sum(p + 128 for p in payloads),
             "source_and_slack_unchanged": True, "replay_equal": True, "guards_intact": True}
    variable = {"padding", "parser_repeats", "wire_repeats", "sequence_count", "stored_literal_bytes",
                "trailing_literals", "long_length_type", "long_length_position", "stored_literals_hex",
                "sequences", "long_table", "short_table"}
    require(set(row) == set(fixed) | variable, "exact case fields")
    require(all(exact(row[key], value) for key, value in fixed.items()), "recipe/scalar/arena mismatch")
    require(fixed["requested_allocation_bytes"] <= MAX_ALLOCATION, "aggregate allocation cap")
    require(type(row["padding"]) is int and row["padding"] in (106, 195), "padding pattern")
    count = integer(row["sequence_count"], 0, q, "sequence count")
    size = integer(row["stored_literal_bytes"], 0, case.block, "literal count")
    trailing = integer(row["trailing_literals"], 0, case.block, "trailing literals")
    kind = integer(row["long_length_type"], 0, 2, "long length kind")
    position = integer(row["long_length_position"], 0, max(0, count - 1), "long length position")
    require((kind == 0 and position == 0) or (kind != 0 and count != 0), "long marker position")
    require(type(row["stored_literals_hex"]) is str and re.fullmatch("[0-9a-f]*", row["stored_literals_hex"])
            is not None and len(row["stored_literals_hex"]) == 2 * size, "canonical literal hex")
    literals = bytes.fromhex(row["stored_literals_hex"])
    require(type(row["sequences"]) is list and len(row["sequences"]) == count, "sequence list size")
    replay = bytearray(case.source[:case.prefix])
    literal_cursor, repeats = 0, list(case.repeats)
    for i, sequence in enumerate(row["sequences"]):
        require(type(sequence) is dict and set(sequence) == {"raw_ll", "raw_ml_base", "off_base", "ll", "ml", "distance", "intent"}, "sequence fields")
        ll = integer(sequence["raw_ll"], 0, 65535, "raw literal length") + (65536 if kind == 1 and i == position else 0)
        ml = integer(sequence["raw_ml_base"], 0, 65535, "raw match base") + 3 + (65536 if kind == 2 and i == position else 0)
        off_base = integer(sequence["off_base"], 1, 0xffffffff, "offset base")
        distance, repeats = resolve_repeat(repeats, off_base, ll)
        require(exact(sequence["ll"], ll) and exact(sequence["ml"], ml) and exact(sequence["distance"], distance), "expanded sequence")
        require(sequence["intent"] == ("repeat" if off_base <= 3 else "offset"), "repeat intent")
        require(4 <= ml and literal_cursor + ll <= size and len(replay) + ll + ml <= n, "bounded sequence replay")
        replay.extend(literals[literal_cursor:literal_cursor + ll])
        literal_cursor += ll
        require(0 < distance <= window and len(replay) - distance >= case.floor, "available history")
        for _ in range(ml):
            replay.append(replay[-distance])
    require(literal_cursor == size and len(replay) + trailing == n, "complete sequence consumption")
    replay.extend(case.source[len(replay):])
    require(bytes(replay) == case.source, "independent replay")
    require(exact(row["wire_repeats"], repeats), "full wire repeats")
    require(exact(row["parser_repeats"], repeats[:2] + [case.repeats[2]]), "raw parser-used repeats")
    validate_table(case, row["long_table"], case.long_log, 8, case.long_seeds)
    validate_table(case, row["short_table"], case.short_log, case.width, case.short_seeds)
    return row


def validate(case, path):
    require(path.is_file() and path.stat().st_size <= MAX_RECORDS, "bounded observation file")
    lines = path.read_bytes().splitlines()
    require(len(lines) == 4, "exact NDJSON record count")
    rows = [decode_json(line) for line in lines]
    metadata = validate_metadata(rows[0])
    require(exact(rows[3], {"record": "complete", "id": case.name, "recipes": 1, "observations": 2, "success": True}), "completion marker")
    for row, padding in zip(rows[1:3], (106, 195)):
        validate_case(case, row, metadata)
        require(row["padding"] == padding, "ordered padding pair")
    require(exact({k: v for k, v in rows[1].items() if k != "padding"},
                  {k: v for k, v in rows[2].items() if k != "padding"}), "padding affected observations")
    return rows
