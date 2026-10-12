#!/usr/bin/env python3
"""Verify pre-Greedy frame bytes and allocation records without running a codec.

The original clock-free archive is read in memory. This checker never imports
benchmark code, starts a process, writes a fixture, or requires current codec
source to equal historical source. --root supports isolated rejection controls.
"""
import argparse
import hashlib
import io
import json
from pathlib import Path, PurePosixPath
import sys
import tarfile

ROOT = Path(__file__).resolve().parents[1]
COLLECTION = 'tests/fixtures/reference-double-fast-baseline'
ARCHIVE = 'docs/evidence/2026-10-12-double-fast-benchmark-harness.tar.gz'
ARCHIVE_SHA = '43b8d17bdf9c43dbc84d4cfc808c656b12ff82d434cebfc5ac2e664553ae04b1'
FIELDS = ['attempts', 'allocate_calls', 'grow_calls', 'peak_live_blocks',
          'peak_requested_bytes', 'peak_bytes', 'returned_requested_bytes',
          'returned_bytes', 'identities_created', 'owners_dropped',
          'final_live_blocks', 'final_live_bytes']
HEADER = ['workload', 'profile', 'source_bytes', 'source_fnv1a64', 'source_sha256',
          'window_log', 'chain_log', 'hash_log', 'search_log', 'min_match',
          'target_length', 'strategy', 'frame_offset', 'frame_bytes',
          'frame_fnv1a64', 'frame_sha256', *FIELDS]
QUALIFICATION = ('Complete zstdoxide frames and exact allocation survey from the reviewed '
    '2026-10-12 clock-free DoubleFast smoke, before Greedy changes. Sixteen logical cases '
    'include twelve DoubleFast and four Fast controls. No timing or independent C decode '
    'is claimed for these exact frames. Allocation replay is qualified to the recorded target/toolchain.')


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def unique(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, 'duplicate JSON key: ' + key)
        result[key] = value
    return result


def bad_constant(value):
    raise ValueError('invalid JSON constant: ' + value)


def parse(data):
    return json.loads(data, object_pairs_hook=unique, parse_constant=bad_constant)


def read(root, name):
    path = root / name
    require(path.resolve().is_relative_to(root) and not path.is_symlink()
            and path.is_file(), 'missing or escaping regular file: ' + name)
    require(path.stat().st_size <= 16 << 20, 'file exceeds bound: ' + name)
    data = path.read_bytes()
    require(len(data) <= 16 << 20, 'file grew beyond bound: ' + name)
    return data


def projection(root):
    """Project immutable recorded bytes; never execute current or archived code."""
    data = read(root, ARCHIVE)
    require(sha(data) == ARCHIVE_SHA, 'original archive SHA256 differs')
    prefix = 'double-fast-benchmark-harness/'
    members, total = {}, 0
    with tarfile.open(fileobj=io.BytesIO(data), mode='r:gz') as archive:
        for member in archive:
            name = PurePosixPath(member.name)
            require(member.isfile() and not name.is_absolute() and '..' not in name.parts
                    and str(name) == member.name and '\\' not in member.name
                    and member.name.startswith(prefix) and member.name not in members,
                    'invalid archive member')
            total += member.size
            require(0 <= member.size <= 24 << 20 and total <= 32 << 20
                    and len(members) < 1582, 'archive bounds')
            raw = archive.extractfile(member).read()
            require(len(raw) == member.size, 'truncated archive member')
            members[member.name] = raw
    require(len(members) == 1582, 'archive inventory differs')
    non_chunks = {prefix + n for n in ('report.json', 'file-map.json', 'reconstruct.py')}
    for name, raw in members.items():
        if name in non_chunks:
            continue
        require(name == prefix + 'chunks/' + sha(raw) + '.bin'
                and 0 < len(raw) <= 4096, 'invalid archive chunk')
    files = parse(members[prefix + 'file-map.json'])
    require(len(files) == 4003, 'raw file catalog differs')

    def stored(name):
        entry = files[name]
        require(type(entry['bytes']) is int and 0 <= entry['bytes'] <= 8 << 20
                and len(entry['chunks']) <= 2048, 'stored file bounds')
        raw = b''.join(members[prefix + 'chunks/' + c + '.bin'] for c in entry['chunks'])
        require(len(raw) == entry['bytes'] and sha(raw) == entry['sha256'],
                'stored file identity differs: ' + name)
        return raw

    e = parse(stored('smoke/evidence.json'))
    require(e['status'] == 'passed' and e['timed'] is False
            and e['configs'] == ['safe', 'all'] and e['source_unchanged'] is True,
            'original capture scope')
    records = {c: [parse(line) for line in stored('smoke/run-' + c + '.stdout').splitlines()]
               for c in e['configs']}
    for config, values in records.items():
        require(len(values) == 17 and values[0]['timed'] is False
                and values[0]['expected_rows'] == 16, 'clock-free run metadata')
        require(len(values[0]['features']) == (0 if config == 'safe' else 12),
                'captured backend inventory')
    rows = {c: values[1:] for c, values in records.items()}
    require(json.dumps(rows['safe'], sort_keys=True) == json.dumps(rows['all'], sort_keys=True),
            'backend observations differ')
    text, pack, offsets = ['\t'.join(HEADER)], bytearray(), {}
    names = set()
    for row in rows['safe']:
        name = row['workload'] + '-' + row['profile'] + '.zst'
        require(name not in names, 'duplicate logical case')
        names.add(name)
        encoded = stored('smoke/frames/safe/' + name)
        require(encoded == stored('smoke/frames/all/' + name), 'backend frame bytes differ')
        digest = sha(encoded)
        for config in e['configs']:
            require(e['frames'][config][name] == {'bytes': len(encoded), 'sha256': digest},
                    'saved frame metadata')
        require(row['frame_bytes'] == len(encoded), 'recorded frame length')
        if digest not in offsets:
            offsets[digest] = len(pack)
            pack.extend(encoded)
        require(row['allocation']['violations'] is False and row['samples'] == [],
                'clock-free owner record')
        require(all(type(row['allocation'][f]) is int and row['allocation'][f] >= 0
                    for f in FIELDS), 'allocation field type')
        values = [row['workload'], row['profile'], row['source_bytes'], row['source_fnv1a64'],
                  e['inputs'][row['workload']]['sha256'], *row['resolved'], offsets[digest],
                  len(encoded), row['frame_fnv1a64'], digest, *[row['allocation'][f] for f in FIELDS]]
        text.append('\t'.join(map(str, values)))
    expected = {'expected.tsv': ('\n'.join(text) + '\n').encode(), 'frames.bin': bytes(pack)}
    provenance = {'schema_version': 1, 'archive': ARCHIVE, 'archive_sha256': ARCHIVE_SHA,
        'campaign_file': 'smoke/evidence.json', 'campaign_sha256': sha(stored('smoke/evidence.json')),
        'captured_revision': stored('smoke/head.stdout').decode().strip(),
        'compiler': e['compiler'], 'target': e['host'], 'toolchain': e['toolchain'],
        'configs': e['configs'], 'timed': False, 'source': e['source'], 'inputs': e['inputs'],
        'rows': len(rows['safe']), 'unique_frames': len(offsets), 'pack_bytes': len(pack),
        'allocation_fields': FIELDS, 'files': {n: sha(b) for n, b in expected.items()},
        'qualification': QUALIFICATION}
    return expected, provenance


def audit(root):
    expected, provenance = projection(root)
    require({p.name for p in (root / COLLECTION).iterdir()} ==
            {'expected.tsv', 'frames.bin', 'provenance.json', 'README.md'}, 'fixture file inventory')
    for name, data in expected.items():
        require(read(root, COLLECTION + '/' + name) == data, 'archived bytes differ: ' + name)
    actual = parse(read(root, COLLECTION + '/provenance.json'))
    # Serialized equality rejects bool-as-int and float-as-int substitutions.
    require(json.dumps(actual, sort_keys=True) == json.dumps(provenance, sort_keys=True),
            'provenance differs from the pinned archive')
    require(read(root, COLLECTION + '/README.md'), 'empty qualification document')
    return provenance['rows']


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=ROOT)
    args = parser.parse_args()
    try:
        count = audit(args.root.resolve())
    except (ValueError, OSError, KeyError, TypeError, tarfile.TarError) as error:
        print('DoubleFast baseline audit failed:', error, file=sys.stderr)
        return 1
    print(f'Verified {count} complete DoubleFast/Fast frame/allocator records from the fixed clock-free archive.')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())

