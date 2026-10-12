"""Versioned lazy-family packets and independent replay; never runs a codec.

The unchanged Greedy transport supplies common bounds, table reconstruction and
wire replay. New selectors are checked before projecting only their schema to
that common validator. No match-selection algorithm generates expectations.
"""
from dataclasses import dataclass, fields
import json
import struct

from . import greedy_direct as predecessor

REVISION = predecessor.REVISION
MAX_PREFIX, MAX_BLOCK, MAX_SEEDS = predecessor.MAX_PREFIX, predecessor.MAX_BLOCK, predecessor.MAX_SEEDS
MAX_PACKET, MAX_RECORDS = predecessor.MAX_PACKET, predecessor.MAX_RECORDS
MAX_ALLOCATION, SUFFIX = predecessor.MAX_ALLOCATION, predecessor.SUFFIX
require, integer, exact = predecessor.require, predecessor.integer, predecessor.exact
sha, fnv, decode_json = predecessor.sha, predecessor.fnv, predecessor.decode_json
key_hash, initial_tables = predecessor.key_hash, predecessor.initial_tables
resolve_repeat, bitmix = predecessor.resolve_repeat, predecessor.bitmix
PROTOCOL = 'ZLAZY01'
MAGIC = b'ZLAZY01\0'


def entry_point(depth, branch):
    integer(depth, 0, 2, 'lookahead depth')
    integer(branch, 0, 1, 'search branch')
    return 'ZSTD_compressBlock_' + ('greedy', 'lazy', 'lazy2')[depth] + ('_row' if branch else '')


@dataclass(frozen=True)
class Recipe(predecessor.Recipe):
    depth: int = 0

    def legacy(self):
        """Share immutable source bytes while retaining every common scalar."""
        return predecessor.Recipe(**{f.name: getattr(self, f.name) for f in fields(predecessor.Recipe)})

    def packet(self):
        integer(self.depth, 0, 2, 'lookahead depth')
        packet = self.legacy().packet()
        # Both fixed slices are in the already validated 152-byte header.
        return MAGIC + packet[8:144] + struct.pack('<I', self.depth) + packet[148:]

    def metadata(self):
        packet = self.packet()
        return self.legacy().metadata() | {
            'depth': self.depth, 'strategy': 3 + self.depth,
            'packet_bytes': len(packet), 'packet_sha256': sha(packet),
        }


def decode_packet(data):
    require(type(data) is bytes and 152 <= len(data) <= MAX_PACKET, 'packet extent')
    require(data[:8] == MAGIC, 'packet magic')
    depth = integer(struct.unpack_from('<I', data, 144)[0], 0, 2, 'lookahead depth')
    old = predecessor.decode_packet(b'ZGRDY01\0' + data[8:144] + bytes(4) + data[148:])
    case = Recipe(**{f.name: getattr(old, f.name) for f in fields(predecessor.Recipe)}, depth=depth)
    require(case.packet() == data, 'canonical lazy packet round trip')
    return case


def _project_metadata(row, branch, depth, mode):
    integer(depth, 0, 2, 'lookahead depth')
    integer(branch, 0, 1, 'search branch')
    require(mode in ('direct', 'fresh-salt'), 'metadata mode')
    expected_entry = ('ZSTD_compressBegin_advanced_internal' if mode == 'fresh-salt'
                      else entry_point(depth, branch))
    require(type(row) is dict and exact(row.get('depth'), depth)
            and exact(row.get('strategy'), 3 + depth) and row.get('protocol') == PROTOCOL
            and row.get('entry_point') == expected_entry, 'lazy metadata selectors')
    common = {k: v for k, v in row.items() if k not in ('depth', 'strategy')}
    common['protocol'] = 'ZGRDY01'
    common['entry_point'] = ('ZSTD_compressBegin_advanced_internal' if mode == 'fresh-salt'
                             else entry_point(0, branch))
    predecessor.validate_metadata(common, branch, mode)
    return common


def validate_metadata(row, branch, depth, mode='direct'):
    _project_metadata(row, branch, depth, mode)
    return row


def validate_case(case, row, metadata):
    common_metadata = _project_metadata(metadata, case.branch, case.depth, 'direct')
    require(type(row) is dict and exact(row.get('depth'), case.depth)
            and exact(row.get('strategy'), 3 + case.depth), 'lazy case selectors')
    common = {k: v for k, v in row.items() if k not in ('depth', 'strategy')}
    # Only the independently checked selector fields are removed. Every source,
    # sequence, table, owner, repeat and entropy field still reaches full replay.
    predecessor.validate_case(case.legacy(), common, common_metadata)


def records(data):
    return predecessor.records(data)


def validate(case, data):
    case.packet()
    rows = records(data)
    metadata = validate_metadata(rows[0], case.branch, case.depth)
    require(exact(rows[3], {'record': 'complete', 'mode': 'direct', 'id': case.name,
                           'depth': case.depth, 'observations': 2, 'success': True}), 'lazy completion')
    for row, padding in zip(rows[1:3], (106, 195), strict=True):
        validate_case(case, row, metadata)
        require(exact(row['padding'], padding), 'ordered padding pair')
    require(exact({k: v for k, v in rows[1].items() if k != 'padding'},
                  {k: v for k, v in rows[2].items() if k != 'padding'}), 'padding affected observations')
    return rows


def validate_salt(data, depth):
    integer(depth, 0, 2, 'lookahead depth')
    rows = records(data)
    common = [_project_metadata(rows[0], 1, depth, 'fresh-salt')]
    for row in rows[1:3]:
        require(type(row) is dict and exact(row.get('depth'), depth)
                and exact(row.get('resolved'), [15, 6, 6, 1, 4, 0, 3 + depth]), 'salt depth/strategy')
        projected = {k: v for k, v in row.items() if k != 'depth'}
        projected['resolved'] = [15, 6, 6, 1, 4, 0, 3]
        common.append(projected)
    require(exact(rows[3], {'record': 'complete', 'mode': 'fresh-salt', 'depth': depth,
                           'observations': 2, 'success': True}), 'salt completion')
    common.append({k: v for k, v in rows[3].items() if k != 'depth'})
    # This tiny schema projection reuses exact common fill/owner/salt checks.
    # It is not evidence that a fresh Lazy/Lazy2 context has actually been run.
    predecessor.validate_salt(('\n'.join(json.dumps(r, allow_nan=False) for r in common) + '\n').encode())
    return rows
