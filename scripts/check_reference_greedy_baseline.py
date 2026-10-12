#!/usr/bin/env python3
"""Verify pre-Lazy frame bytes and allocation records without running a codec.

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
COLLECTION = 'tests/fixtures/reference-greedy-baseline'
ARCHIVE = 'docs/evidence/2026-10-12-greedy-clock-free.tar.gz'
ARCHIVE_SHA = '2b4e46c113e24bc5e36ef7195de75dc4ad2b27fc202422539d6af8c673c788f3'
FIELDS = ['attempts', 'allocate_calls', 'grow_calls', 'peak_live_blocks',
          'peak_requested_bytes', 'peak_bytes', 'returned_requested_bytes',
          'returned_bytes', 'identities_created', 'owners_dropped',
          'final_live_blocks', 'final_live_bytes']
HEADER = ['workload', 'profile', 'source_bytes', 'source_fnv1a64', 'source_sha256',
          'window_log', 'chain_log', 'hash_log', 'search_log', 'min_match',
          'target_length', 'strategy', 'frame_offset', 'frame_bytes',
          'frame_fnv1a64', 'frame_sha256', *FIELDS]
FEATURES = ('unsafe-byte-cursor', 'unsafe-bitstream', 'unsafe-history', 'unsafe-ring-history',
    'unsafe-fse', 'unsafe-fse-encoder', 'unsafe-huffman', 'unsafe-huffman-codebook',
    'unsafe-match-table', 'unsafe-match-chain', 'unsafe-dictionary', 'unsafe-training',
    'unsafe-reference-chain', 'unsafe-row-table')
CONFIGS = ['safe', *FEATURES, 'all']
QUALIFICATION = ('Complete zstdoxide frames and exact allocation survey from the reviewed '
    '2026-10-12 clock-free Greedy campaign, before Lazy/Lazy2 changes. Thirty-two logical '
    'cases include twenty-four Greedy and eight Fast/DoubleFast controls. All sixteen '
    'backends agree. No timing or independent C decode is claimed for these exact frames. '
    'Allocation replay is qualified to the recorded target/toolchain.')


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
    require(path.stat().st_size <= 32 << 20, 'file exceeds bound: ' + name)
    data = path.read_bytes()
    require(len(data) <= 32 << 20, 'file grew beyond bound: ' + name)
    return data


def projection(root):
    """Project immutable recorded bytes; never execute current or archived code."""
    data = read(root, ARCHIVE)
    require(sha(data) == ARCHIVE_SHA, 'original archive SHA256 differs')
    prefix = 'greedy-clock-free/'
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
                    and len(members) < 5282, 'archive bounds')
            raw = archive.extractfile(member).read()
            require(len(raw) == member.size, 'truncated archive member')
            members[member.name] = raw
    require(len(members) == 5282, 'archive inventory differs')
    non_chunks = {prefix + n for n in ('report.json', 'file-map.json', 'reconstruct.py')}
    for name, raw in members.items():
        if name in non_chunks:
            continue
        require(name == prefix + 'chunks/' + sha(raw) + '.bin'
                and 0 < len(raw) <= 4096, 'invalid archive chunk')
    files = parse(members[prefix + 'file-map.json'])
    require(len(files) == 669, 'raw file catalog differs')

    def stored(name):
        entry = files[name]
        require(type(entry['bytes']) is int and 0 <= entry['bytes'] <= 8 << 20
                and len(entry['chunks']) <= 2048, 'stored file bounds')
        raw = b''.join(members[prefix + 'chunks/' + c + '.bin'] for c in entry['chunks'])
        require(len(raw) == entry['bytes'] and sha(raw) == entry['sha256'],
                'stored file identity differs: ' + name)
        return raw

    e = parse(stored('clock-free/evidence.json'))
    require(e['status'] == 'passed' and e['timed'] is False
            and e['configs'] == CONFIGS and e['source_unchanged'] is True,
            'original capture scope')
    records = {c: [parse(line) for line in stored('clock-free/run-' + c + '.stdout').splitlines()]
               for c in e['configs']}
    for config, values in records.items():
        require(len(values) == 33 and values[0]['timed'] is False
                and values[0]['expected_rows'] == 32, 'clock-free run metadata')
        require(values[0]['features'] == ([] if config == 'safe' else list(FEATURES) if config == 'all' else [config]),
                'captured backend inventory')
    rows = {c: values[1:] for c, values in records.items()}
    require(all(json.dumps(rows['safe'], sort_keys=True) == json.dumps(rows[c], sort_keys=True) for c in CONFIGS),
            'backend observations differ')
    text, pack, offsets = ['\t'.join(HEADER)], bytearray(), {}
    names = set()
    for row in rows['safe']:
        name = row['workload'] + '-' + row['profile'] + '.zst'
        require(name not in names, 'duplicate logical case')
        names.add(name)
        encoded = stored('clock-free/frames/safe/' + name)
        require(all(encoded == stored('clock-free/frames/' + c + '/' + name) for c in CONFIGS), 'backend frame bytes differ')
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
        'campaign_file': 'clock-free/evidence.json', 'campaign_sha256': sha(stored('clock-free/evidence.json')),
        'captured_revision': stored('clock-free/head.stdout').decode().strip(),
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
        print('Greedy baseline audit failed:', error, file=sys.stderr)
        return 1
    print(f'Verified {count} complete Greedy/DoubleFast/Fast frame/allocator records from the fixed clock-free archive.')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())

