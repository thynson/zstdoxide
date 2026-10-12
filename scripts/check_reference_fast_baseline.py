#!/usr/bin/env python3
"""Verify pre-DoubleFast frame bytes and allocation records without running a codec.

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
COLLECTION = 'tests/fixtures/reference-fast-baseline'
ARCHIVE = 'docs/evidence/2026-10-09-fast-benchmark-harness.tar.gz'
ARCHIVE_SHA = '86dbe4cf13e542944bb6ae7fc2bd8e8dfbee513b1b24b25729b97c47096da968'
FIELDS = ['attempts', 'allocate_calls', 'grow_calls', 'peak_live_blocks',
          'peak_requested_bytes', 'peak_bytes', 'returned_requested_bytes',
          'returned_bytes', 'identities_created', 'owners_dropped',
          'final_live_blocks', 'final_live_bytes']
HEADER = ['workload', 'profile', 'source_bytes', 'source_fnv1a64', 'source_sha256',
          'window_log', 'chain_log', 'hash_log', 'search_log', 'min_match',
          'target_length', 'strategy', 'frame_offset', 'frame_bytes',
          'frame_fnv1a64', 'frame_sha256', *FIELDS]
QUALIFICATION = ('Complete zstdoxide frames and exact allocation survey from the reviewed '
    'clock-free smoke-5, before DoubleFast changes. No timing or exact independent C decode '
    'is claimed for these nine frames. Exact allocation replay is qualified to the recorded target/toolchain.')


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
    require(path.stat().st_size <= 2 << 20, 'file exceeds bound: ' + name)
    data = path.read_bytes()
    require(len(data) <= 2 << 20, 'file grew beyond bound: ' + name)
    return data


def audit(root):
    data = read(root, ARCHIVE)
    require(sha(data) == ARCHIVE_SHA, 'original archive SHA256 differs')
    members = {}
    total = 0
    with tarfile.open(fileobj=io.BytesIO(data), mode='r:gz') as archive:
        for member in archive:
            name = PurePosixPath(member.name)
            require(member.isfile() and not name.is_absolute() and '..' not in name.parts
                    and str(name) == member.name and '\\' not in member.name
                    and member.name not in members, 'invalid archive member')
            total += member.size
            require(0 <= member.size <= 2 << 20 and total <= 8 << 20
                    and len(members) < 55, 'archive bounds')
            members[member.name] = archive.extractfile(member).read()
    require(len(members) == 55, 'archive inventory differs')
    e = parse(members['smoke-5/evidence.json'])
    logs, frames = parse(members['log-map.json']), parse(members['frame-map.json'])
    require(e['status'] == 'passed' and e['timed'] is False
            and e['configs'] == ['safe', 'all'], 'original capture scope')
    rows = {c: [parse(line) for line in members[logs['smoke-5/run-' + c + '.stdout']].splitlines()][1:]
            for c in e['configs']}
    require(rows['safe'] == rows['all'] and len(rows['safe']) == 9, 'backend observations differ')
    text, pack, offsets = ['\t'.join(HEADER)], bytearray(), {}
    for row in rows['safe']:
        name = row['workload'] + '-' + row['profile'] + '.zst'
        encoded = members[frames['smoke-5/frames/safe/' + name]]
        require(encoded == members[frames['smoke-5/frames/all/' + name]], 'backend frame bytes differ')
        digest = sha(encoded)
        require(e['frames']['safe'][name] == {'bytes': len(encoded), 'sha256': digest}, 'saved frame metadata')
        if digest not in offsets:
            offsets[digest] = len(pack)
            pack.extend(encoded)
        require(row['allocation']['violations'] is False and row['samples'] == [], 'clock-free owner record')
        values = [row['workload'], row['profile'], row['source_bytes'], row['source_fnv1a64'],
                  e['inputs'][row['workload']]['sha256'], *row['resolved'], offsets[digest],
                  len(encoded), row['frame_fnv1a64'], digest, *[row['allocation'][f] for f in FIELDS]]
        text.append('\t'.join(map(str, values)))
    expected = {'expected.tsv': ('\n'.join(text) + '\n').encode(), 'frames.bin': bytes(pack)}
    require(len(offsets) == 7 and len(pack) == 1090164, 'distinct frame catalog')
    provenance = {'schema_version': 1, 'archive': ARCHIVE, 'archive_sha256': ARCHIVE_SHA,
        'campaign_member': 'smoke-5/evidence.json', 'campaign_sha256': sha(members['smoke-5/evidence.json']),
        'captured_revision': members[logs['smoke-5/head.stdout']].decode().strip(),
        'compiler': e['compiler'], 'target': e['host'], 'toolchain': e['toolchain'],
        'configs': e['configs'], 'timed': False, 'source': e['source'], 'inputs': e['inputs'],
        'rows': 9, 'unique_frames': 7, 'pack_bytes': len(pack), 'allocation_fields': FIELDS,
        'files': {n: sha(b) for n, b in expected.items()}, 'qualification': QUALIFICATION}
    require({p.name for p in (root / COLLECTION).iterdir()} ==
            {'expected.tsv', 'frames.bin', 'provenance.json', 'README.md'}, 'fixture file inventory')
    for name, data in expected.items():
        require(read(root, COLLECTION + '/' + name) == data, 'archived bytes differ: ' + name)
    actual = parse(read(root, COLLECTION + '/provenance.json'))
    # Serialized equality also rejects bool-as-int and float-as-int substitutions.
    require(json.dumps(actual, sort_keys=True) == json.dumps(provenance, sort_keys=True),
            'provenance differs from the pinned archive')
    require(read(root, COLLECTION + '/README.md'), 'empty qualification document')
    return len(rows['safe'])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=ROOT)
    args = parser.parse_args()
    try:
        count = audit(args.root.resolve())
    except (ValueError, OSError, KeyError, TypeError, tarfile.TarError) as error:
        print('Fast baseline audit failed:', error, file=sys.stderr)
        return 1
    print(f'Verified {count} complete Fast frame/allocator records from the fixed clock-free archive.')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
