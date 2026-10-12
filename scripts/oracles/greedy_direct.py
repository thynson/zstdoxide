#!/usr/bin/env python3
"""Bounded Greedy input recipes and independent replay, without a codec or process."""
from dataclasses import dataclass, replace
from pathlib import Path
import hashlib
import json
import re
import struct

REVISION = 'f8745da6ff1ad1e7bab384bd1f9d742439278e99'
MAX_PREFIX, MAX_BLOCK, MAX_SEEDS = 2 << 20, 128 << 10, 524288
MAX_PACKET = 152 + 4 * MAX_SEEDS + MAX_PREFIX + MAX_BLOCK
MAX_RECORDS, MAX_ALLOCATION, SUFFIX = 128 << 20, 64 << 20, 2048


def require(condition, message):
    if not condition:
        raise ValueError(message)


def integer(value, low, high, message):
    require(type(value) is int and low <= value <= high, message)
    return value


def exact(left, right):
    return json.dumps(left, sort_keys=True, allow_nan=False) == json.dumps(right, sort_keys=True, allow_nan=False)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def fnv(data):
    value = 0xcbf29ce484222325
    for byte in data:
        value = ((value ^ byte) * 0x100000001b3) & 0xffffffffffffffff
    return f'{value:016x}'


def decode_json(data):
    def pairs(items):
        result = {}
        for key, value in items:
            require(key not in result, 'duplicate JSON field: ' + key)
            result[key] = value
        return result
    def nonfinite(value):
        raise ValueError('nonfinite scalar: ' + value)
    return json.loads(data, object_pairs_hook=pairs, parse_constant=nonfinite)


def key_hash(key, bits, salt=0):
    """Independent selected-byte arithmetic; zstd_compress_internal.h:903-960."""
    require(type(key) is bytes and 4 <= len(key) <= 6, 'hash key domain')
    integer(bits, 1, 32, 'hash width')
    integer(salt, 0, 0xffffffffffffffff, 'hash salt')
    value = int.from_bytes(key, 'little')
    if len(key) == 4:
        return (((value * 2654435761) & 0xffffffff) ^ (salt & 0xffffffff)) >> (32 - bits)
    prime = 889523592379 if len(key) == 5 else 227718039650203
    return ((((value << (64 - len(key) * 8)) * prime) & 0xffffffffffffffff) ^ salt) >> (64 - bits)


@dataclass(frozen=True)
class Recipe:
    name: str
    source: bytes
    branch: int = 0
    prefix: int = 0
    window_log: int = 10
    hash_log: int = 6
    chain_log: int = 6
    search_log: int = 1
    min_match: int = 4
    target: int = 0
    floor: int = 0
    next: int = 0
    repeats: tuple = (1, 4, 8)
    salt: int = 0
    entropy: int = 0
    seeds: tuple = ()

    @property
    def block(self):
        return len(self.source) - self.prefix

    @property
    def width(self):
        return min(6, max(4, self.min_match))

    @property
    def row_log(self):
        return min(6, max(4, self.search_log)) if self.branch else 0

    def packet(self):
        require(type(self.name) is str and re.fullmatch('[a-z0-9_-]{1,63}', self.name), 'recipe id')
        require(type(self.source) is bytes, 'source bytes')
        integer(self.branch, 0, 1, 'branch')
        integer(self.prefix, 0, MAX_PREFIX, 'prefix')
        integer(self.block, 7, MAX_BLOCK, 'block')
        integer(self.window_log, 10, 21, 'window log')
        integer(self.hash_log, 6, 19, 'hash log')
        integer(self.chain_log, 6, 18, 'chain log')
        integer(self.search_log, 1, 29 if self.branch else 8, 'search log')
        integer(self.min_match, 3, 7, 'minimum match')
        integer(self.target, 0, 131072, 'target')
        integer(self.floor, 0, self.prefix, 'floor')
        integer(self.next, self.floor, self.prefix, 'pending update')
        integer(self.salt, 0, 0xffffffffffffffff if self.branch else 0, 'salt')
        integer(self.entropy, 0, 0xffffffff if self.branch else 0, 'entropy')
        require(self.block <= 1 << self.window_log, 'block exceeds window')
        require(type(self.repeats) is tuple and len(self.repeats) == 3, 'repeat shape')
        for value in self.repeats:
            integer(value, 1, 0xffffffff, 'repeat distance')
        require(type(self.seeds) is tuple and len(self.seeds) <= MAX_SEEDS, 'seed count')
        previous = -1
        for position in self.seeds:
            integer(position, previous + 1, min(self.next - 1, self.prefix - self.width), 'ordered prefix seed')
            previous = position
        values = (self.branch, self.prefix, self.block, self.window_log, self.hash_log,
                  self.chain_log, self.search_log, self.min_match, self.target, self.floor,
                  self.next, *self.repeats, self.salt, self.entropy, len(self.seeds), 0, 0)
        packet = b'ZGRDY01\0' + self.name.encode().ljust(64, b'\0') + struct.pack('<14IQ4I', *values)
        packet += b''.join(struct.pack('<I', p) for p in self.seeds) + self.source
        require(len(packet) <= MAX_PACKET, 'packet cap')
        return packet

    def metadata(self):
        packet = self.packet()
        return {'id': self.name, 'branch': self.branch, 'source_bytes': len(self.source),
                'source_sha256': sha(self.source), 'packet_bytes': len(packet), 'packet_sha256': sha(packet),
                'prefix_bytes': self.prefix, 'block_bytes': self.block, 'window_log': self.window_log,
                'hash_log': self.hash_log, 'chain_log': self.chain_log, 'search_log': self.search_log,
                'min_match': self.min_match, 'target_length': self.target, 'prefix_floor': self.floor,
                'next_to_update': self.next, 'initial_repeats': list(self.repeats), 'seeds': list(self.seeds),
                'initial_salt': f'{self.salt:016x}', 'initial_entropy': self.entropy}


def decode_packet(data):
    require(type(data) is bytes and 152 <= len(data) <= MAX_PACKET, 'packet extent')
    require(data[:8] == b'ZGRDY01\0', 'packet magic')
    raw_id = data[8:72]
    require(b'\0' in raw_id, 'id terminator')
    name, tail = raw_id.split(b'\0', 1)
    require(tail == bytes(len(tail)), 'id padding')
    values = struct.unpack('<14IQ4I', data[72:152])
    branch, prefix, block, w, h, c, s, m, t, floor, pending, a, b, d, salt, entropy, count, r1, r2 = values
    require(r1 == r2 == 0 and count <= MAX_SEEDS and prefix <= MAX_PREFIX
            and block <= MAX_BLOCK and len(data) == 152 + 4 * count + prefix + block, 'packet scalar extent')
    seeds = struct.unpack(f'<{count}I', data[152:152 + 4 * count])
    case = Recipe(name.decode('ascii'), data[152 + 4 * count:], branch, prefix, w, h, c, s, m,
                  t, floor, pending, (a, b, d), salt, entropy, seeds)
    require(case.packet() == data, 'canonical packet round trip')
    return case


def recipes():
    """Source-only tiny hypotheses, frozen before any reference execution."""
    result = []
    prefix = b'0123456789ABCDEFGHIJKLMNOPQRSTUV'
    for branch, label in ((0, 'chain'), (1, 'row')):
        for size in (7, 8, 9, 10, 15, 16, 17, 18):
            result.append(Recipe(f'{label}-fresh-{size}', b'abcdefghijklmnopqr'[:size], branch=branch))
        for size in (9, 10, 16, 17):
            result.append(Recipe(f'{label}-prefix-{size}', prefix + prefix[:size], branch=branch,
                                 prefix=len(prefix), next=len(prefix), seeds=(0,), repeats=(257, 263, 269)))
        period = Recipe(f'{label}-period-four', b'abcd' * 16, branch=branch)
        result += [Recipe(f'{label}-period-one', b'A' * 64, branch=branch), period,
                   replace(period, name=f'{label}-minimum-three', min_match=3),
                   replace(period, name=f'{label}-minimum-seven', min_match=7),
                   replace(period, name=f'{label}-unavailable-repeats', repeats=(257, 263, 269))]
    result += [replace(next(c for c in result if c.name == 'row-prefix-17'), name='row-prefix-salted', salt=0x0123456789abcdef),
               Recipe('row-period-salted', b'abcd' * 16, branch=1, salt=0x0123456789abcdef)]
    require(len(result) == len({c.name for c in result}) == 36, 'tiny inventory')
    for case in result:
        case.packet()
    return result


def initial_tables(case):
    case.packet()  # Validate dimensions before allocating maps.
    first = [0] * (1 << case.hash_log)
    second = [0] * (1 << (case.hash_log if case.branch else case.chain_log))
    for p in case.seeds:
        if case.branch:
            hashed = key_hash(case.source[p:p + case.width], case.hash_log - case.row_log + 8, case.salt)
            row, mask = (hashed >> 8) << case.row_log, (1 << case.row_log) - 1
            slot = (second[row] - 1) & mask
            slot = slot or mask
            second[row] = slot
            second[row + slot] = hashed & 255
            first[row + slot] = p + 2
        else:
            hashed = key_hash(case.source[p:p + case.width], case.hash_log)
            second[(p + 2) & (len(second) - 1)] = first[hashed]
            first[hashed] = p + 2
    return first, second


def validate_metadata(row, branch, mode='direct'):
    fixed = {'record': 'metadata', 'schema_version': 1, 'revision': REVISION, 'protocol': 'ZGRDY01',
             'mode': mode, 'branch': branch, 'ordinary_frames': False, 'collector': False,
             'entry_point': ('ZSTD_compressBegin_advanced_internal' if mode == 'fresh-salt' else
                             ('ZSTD_compressBlock_greedy_row' if branch else 'ZSTD_compressBlock_greedy')),
             'observation_kind': 'sequences-and-final-tables-not-chronological-trace',
             'guard_bytes': 64, 'alignment': 64, 'alignment_slack': 63, 'leading_bytes': 64,
             'suffix_bytes': SUFFIX, 'max_prefix_bytes': MAX_PREFIX, 'max_block_bytes': MAX_BLOCK,
             'max_seeds': MAX_SEEDS, 'max_packet_bytes': MAX_PACKET, 'max_allocation_bytes': MAX_ALLOCATION,
             'max_context_bytes': 8 << 20, 'u32_bytes': 4, 'sequence_bytes': 8, 'hash_read_bytes': 8,
             'window_start_index': 2, 'minimum_match': 3, 'wildcopy_overlength': 32,
             'hash_cache_size': 8, 'row_tag_bits': 8, 'upstream_sanitizer_annotations_retained': True}
    host = {'pointer_bits', 'little_endian', 'u32_alignment', 'sequence_alignment',
            'observation_bytes', 'observation_alignment', 'state_bytes', 'state_alignment'}
    require(type(row) is dict and set(row) == set(fixed) | host, 'metadata fields')
    require(all(exact(row[k], v) for k, v in fixed.items()), 'metadata constants')
    integer(row['pointer_bits'], 32, 64, 'host pointer width')
    require(row['pointer_bits'] in (32, 64) and type(row['little_endian']) is bool, 'host description')
    require(exact(row['observation_bytes'], row['pointer_bits'] // 4 + 8), 'observation layout')
    for name in host - {'pointer_bits', 'little_endian'}:
        integer(row[name], 1, 1 << 20, 'host layout')
        if name.endswith('alignment'):
            require(row[name] <= 64 and 64 % row[name] == 0, 'host alignment')
    return row


def validate_table(row, before, log, element_bytes, source_max):
    keys = {'log', 'element_bytes', 'entries', 'before', 'changes', 'before_fnv1a64', 'after_fnv1a64'}
    require(type(row) is dict and set(row) == keys, 'table fields')
    for name, value in {'log': log, 'element_bytes': element_bytes, 'entries': len(before)}.items():
        require(exact(row[name], value), 'table dimensions')
    require(exact(row['before'], [[i, p] for i, p in enumerate(before) if p]), 'initial full map')
    require(type(row['changes']) is list and len(row['changes']) <= len(before), 'delta count')
    after, previous = before.copy(), -1
    for change in row['changes']:
        require(type(change) is list and len(change) == 3, 'delta shape')
        index, old, new = change
        integer(index, previous + 1, len(before) - 1, 'sorted delta index')
        integer(old, 0, 255 if element_bytes == 1 else source_max, 'old cell')
        integer(new, 0, 255 if element_bytes == 1 else source_max, 'new cell')
        require(old == before[index] and new != old, 'delta provenance')
        after[index], previous = new, index
    for label, cells in (('before', before), ('after', after)):
        raw = bytes(cells) if element_bytes == 1 else struct.pack(f'<{len(cells)}I', *cells)
        require(row[label + '_fnv1a64'] == fnv(raw), 'complete canonical table fingerprint')
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
    require(distance > 0, 'positive repeat-minus-one')
    return distance, [distance, a, b]


def validate_case(case, row, metadata):
    case.packet()  # Reject invalid caller recipes before any shift or derived allocation.
    n, q = len(case.source), case.block // 3 + 1
    payloads = [4 * len(case.seeds), n, 64 + n + SUFFIX, 64 + n + SUFFIX,
                *(2 * [4 << case.hash_log]),
                *(2 * [(1 << case.hash_log) if case.branch else (4 << case.chain_log)]),
                q * metadata['sequence_bytes'], case.block + 32, 3 * q, n,
                q * metadata['observation_bytes']]
    total = sum(max(p, 1) + 191 for p in payloads)
    require(total <= MAX_ALLOCATION, 'aggregate allocation cap')
    current = case.prefix + (case.prefix == case.floor)
    fixed = {'record': 'case', **{k: v for k, v in case.metadata().items()
             if k not in {'packet_bytes', 'packet_sha256', 'source_sha256'}},
             'source_fnv1a64': fnv(case.source), 'key_width': case.width, 'row_log': case.row_log,
             'initial_dict_limit': case.floor + 2, 'initial_low_limit': case.floor + 2,
             'initial_next_to_update': case.next + 2, 'final_salt': f'{case.salt:016x}',
             'loaded_dict_end': 0, 'dictionary_state_null': True, 'initial_current': current,
             'repeat_floor': max(case.floor, current - (1 << case.window_log)),
             'physical_leading_bytes': 64, 'physical_suffix_bytes': SUFFIX, 'sequence_capacity': q,
             'literal_slack_bytes': 32, 'requested_allocation_bytes': total,
             'source_and_slack_unchanged': True, 'replay_equal': True, 'guards_intact': True}
    variable = {'padding', 'final_next_to_update', 'final_lazy_skipping', 'final_entropy', 'hash_cache',
                'parser_repeats', 'wire_repeats', 'sequence_count', 'stored_literal_bytes', 'trailing_literals',
                'long_length_type', 'long_length_position', 'stored_literals_hex', 'sequences',
                'first_table', 'second_table'}
    require(type(row) is dict and set(row) == set(fixed) | variable, 'case fields')
    require(all(exact(row[k], v) for k, v in fixed.items()), 'fixed recipe/state values')
    integer(row['final_next_to_update'], case.next + 2, n + 2, 'update frontier')
    integer(row['final_lazy_skipping'], 0, 1, 'skipping flag')
    integer(row['final_entropy'], 0, 0xffffffff if case.branch else 0, 'salt entropy')
    require(type(row['hash_cache']) is list and len(row['hash_cache']) == 8, 'cache observation')
    for value in row['hash_cache']:
        integer(value, 0, (1 << (case.hash_log - case.row_log + 8)) - 1 if case.branch else 0, 'cache value')
    count = integer(row['sequence_count'], 0, q, 'sequence count')
    size = integer(row['stored_literal_bytes'], 0, case.block, 'stored literal bytes')
    trailing = integer(row['trailing_literals'], 0, case.block, 'trailing literal count')
    kind = integer(row['long_length_type'], 0, 2, 'long length kind')
    position = integer(row['long_length_position'], 0, max(0, count - 1), 'long length position')
    require((kind == 0 and position == 0) or (kind != 0 and count != 0), 'long marker position')
    require(type(row['stored_literals_hex']) is str and re.fullmatch('[0-9a-f]*', row['stored_literals_hex'])
            is not None and len(row['stored_literals_hex']) == 2 * size, 'literal hex')
    literals = bytes.fromhex(row['stored_literals_hex'])
    require(type(row['sequences']) is list and len(row['sequences']) == count, 'sequence count')
    replay, literal_cursor, repeats = bytearray(case.source[:case.prefix]), 0, list(case.repeats)
    for i, sequence in enumerate(row['sequences']):
        require(type(sequence) is dict and set(sequence) ==
                {'raw_ll', 'raw_ml_base', 'off_base', 'll', 'ml', 'distance', 'intent'}, 'sequence fields')
        ll = integer(sequence['raw_ll'], 0, 65535, 'raw LL') + (65536 if kind == 1 and i == position else 0)
        ml = integer(sequence['raw_ml_base'], 0, 65535, 'raw ML') + 3 + (65536 if kind == 2 and i == position else 0)
        off_base = integer(sequence['off_base'], 1, 0xffffffff, 'offset base')
        distance, repeats = resolve_repeat(repeats, off_base, ll)
        require(exact(sequence['ll'], ll) and exact(sequence['ml'], ml)
                and exact(sequence['distance'], distance), 'expanded sequence')
        require(sequence['intent'] == ('repeat' if off_base <= 3 else 'offset'), 'sequence intent')
        require(ml >= 4 and literal_cursor + ll <= size and len(replay) + ll + ml <= n, 'replay bounds')
        replay.extend(literals[literal_cursor:literal_cursor + ll])
        literal_cursor += ll
        require(0 < distance <= 1 << case.window_log and len(replay) - distance >= case.floor, 'available history')
        for _ in range(ml):
            replay.append(replay[-distance])
    require(literal_cursor == size and len(replay) + trailing == n, 'complete sequence consumption')
    replay.extend(case.source[len(replay):])
    require(bytes(replay) == case.source, 'independent replay')
    require(exact(row['wire_repeats'], repeats) and
            exact(row['parser_repeats'], repeats[:2] + [case.repeats[2]]), 'wire versus raw repeats')
    first, second = initial_tables(case)
    first = validate_table(row['first_table'], first, case.hash_log, 4, n + 2)
    second = validate_table(row['second_table'], second, case.hash_log if case.branch else case.chain_log,
                            1 if case.branch else 4, n + 2)
    mask = (1 << case.row_log) - 1
    for i, p in enumerate(first):
        if case.branch and i & mask == 0:
            require(p == 0 and second[i] <= mask, 'reserved row head')
        elif p:
            integer(p, 2, n + 2 - case.width, 'complete candidate key')
            h = key_hash(case.source[p - 2:p - 2 + case.width],
                         case.hash_log - case.row_log + 8 if case.branch else case.hash_log,
                         case.salt if case.branch else 0)
            require(((h >> 8) << case.row_log == i & ~mask and h & 255 == second[i])
                    if case.branch else h == i, 'final bucket/tag identity')
        elif case.branch:
            require(second[i] == 0, 'empty row position tag')
    if not case.branch:
        for p in second:
            require(p == 0 or 2 <= p <= n + 2 - case.width, 'complete predecessor key')
    return row


def records(data):
    require(type(data) is bytes and len(data) <= MAX_RECORDS and data.endswith(b'\n'), 'bounded NDJSON')
    lines = data.splitlines()
    require(len(lines) == 4 and all(lines), 'four complete records')
    return [decode_json(line) for line in lines]


def validate(case, data):
    rows = records(data)
    metadata = validate_metadata(rows[0], case.branch)
    require(exact(rows[3], {'record': 'complete', 'mode': 'direct', 'id': case.name,
                           'observations': 2, 'success': True}), 'completion')
    for row, padding in zip(rows[1:3], (106, 195)):
        validate_case(case, row, metadata)
        require(exact(row['padding'], padding), 'ordered padding pair')
    require(exact({k: v for k, v in rows[1].items() if k != 'padding'},
                  {k: v for k, v in rows[2].items() if k != 'padding'}), 'padding affected observations')
    return rows


def bitmix(value, length):
    """Source-only cross-check, zstd_compress.c:1970-1982; not a captured salt."""
    mask = (1 << 64) - 1
    def rotate(n):
        return (value >> n) | ((value << (64 - n)) & mask)
    value ^= rotate(49) ^ rotate(24)
    value = (value * 0x9fb21c651e98df25) & mask
    value ^= (value >> 35) + length
    value = (value * 0x9fb21c651e98df25) & mask
    return value ^ (value >> 28)


def validate_salt(data):
    rows = records(data)
    validate_metadata(rows[0], 1, 'fresh-salt')
    require(exact(rows[3], {'record': 'complete', 'mode': 'fresh-salt', 'observations': 2,
                           'success': True}), 'salt completion')
    predicted = bitmix(0, 8) ^ bitmix(0, 4)
    for row, run, fill in zip(rows[1:3], (0, 1), (59, 209)):
        fixed = {'record': 'salt', 'run': run, 'allocation_fill': fill,
                 'resolved': [15, 6, 6, 1, 4, 0, 3], 'initial_salt': '0000000000000000',
                 'final_salt': f'{predicted:016x}', 'initial_entropy': '00000000',
                 'final_entropy': '00000000', 'row_hash_log': 2, 'table_entries': 64,
                 'zero_tables': True, 'final_live_bytes': 0}
        require(type(row) is dict and set(row) == set(fixed) |
                {'allocate_calls', 'free_calls', 'peak_guarded_bytes'}, 'salt fields')
        require(all(exact(row[k], v) for k, v in fixed.items()), 'observed salt differs from source cross-check')
        integer(row['allocate_calls'], 1, 64, 'fresh allocation count')
        require(exact(row['free_calls'], row['allocate_calls']), 'fresh context release')
        integer(row['peak_guarded_bytes'], 1, 8 << 20, 'fresh allocation cap')
    require(exact({k: v for k, v in rows[1].items() if k not in {'run', 'allocation_fill'}},
                  {k: v for k, v in rows[2].items() if k not in {'run', 'allocation_fill'}}), 'fresh fills differ')
    return rows
