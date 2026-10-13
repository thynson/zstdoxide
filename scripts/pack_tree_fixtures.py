#!/usr/bin/env python3
"""Project frozen direct tree observations without executing archived code."""
from pathlib import Path
import argparse
import hashlib
import json
import struct
import sys
import tarfile
sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1]
ARCHIVE = 'docs/evidence/2026-10-13-tree-families.tar.gz'
ARCHIVE_SHA = 'c2b8b223d0978681b8baf3db6889c2d9cf2ac8f8b397942c5a2779be11847bbf'
INPUTS_SHA = '4469d331ca935749fe75a0a3841ac823d8d20d09f60cf7bc84214f777e4faa1e'
DESTINATION = ROOT / 'tests/fixtures/reference-tree'


def require(value, reason):
    if not value:
        raise ValueError(reason)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def archived_files():
    path = ROOT / ARCHIVE
    require(path.is_file() and not path.is_symlink() and path.stat().st_size == 27949006
            and sha(path.read_bytes()) == ARCHIVE_SHA, 'immutable broader archive')
    files, total = {}, 0
    with tarfile.open(path) as archive:
        for member in archive:
            require(member.isfile() and member.name not in files and
                    0 <= member.size <= (16 << 20), 'bounded regular unique member')
            total += member.size
            require(total <= (128 << 20), 'bounded archive expansion')
            files[member.name] = archive.extractfile(member).read()
    require(len(files) == 18754, 'archive member inventory')
    mapping = json.loads(files['tree-families/file-map.json'])
    require(len(mapping) == 820, 'raw file inventory')
    result, total = {}, 0
    for name, entry in mapping.items():
        require(type(entry['bytes']) is int and 0 <= entry['bytes'] <= (128 << 20), 'raw file bound')
        total += entry['bytes']
        require(total <= 143454268, 'raw aggregate bound')
        chunks = []
        for digest in entry['chunks']:
            chunk = files['tree-families/chunks/' + digest + '.bin']
            require(len(chunk) <= 4096 and sha(chunk) == digest, 'chunk identity')
            chunks.append(chunk)
        data = b''.join(chunks)
        require(len(data) == entry['bytes'] and sha(data) == entry['sha256'], 'reconstructed file identity')
        result[name] = data
    require(total == 143454268, 'complete raw bytes')
    return result


def project():
    require(sha((ROOT / 'scripts/tree_family_capture_inputs.py').read_bytes()) == INPUTS_SHA,
            'reviewed input owner')
    import tree_family_capture_inputs as inputs
    g, cases, _ = inputs.inputs()
    files = archived_files()
    e = g.decode_json(files['evidence.json'])
    require(e['status'] == 'passed' and e['source_before'] == e['source_after'], 'completed capture')
    for name, digest in e['source_before'].items():
        require(sha((ROOT / name).read_bytes()) == digest, 'unchanged capture source ' + name)
    output = bytearray(b'ZTREEFX1' + struct.pack('<I', len(cases)))
    provenance = []

    def numbers(values):
        require(all(type(v) is int and 0 <= v <= 0xffffffff for v in values), 'u32 domain')
        output.extend(struct.pack('<' + str(len(values)) + 'I', *values))

    for case in cases:
        packet = case.packet()
        require(files['requests/' + case.name + '.bin'] == packet, 'original packet')
        data = files['logs/normal-case-' + case.name + '.stdout']
        g.validate(case, data)
        require(files['logs/sanitized-case-' + case.name + '.stdout'] == data, 'instrumented equality')
        rows = [g.decode_json(line) for line in data.splitlines()]
        metadata = rows[0]
        operations = [r for r in rows if r['record'] == 'operation' and r['pass'] == 0]
        request = case.request
        require([r['query'] for r in operations] == list(request.queries), 'exact schedule')
        name = case.name.encode('ascii')
        require(1 <= len(name) <= 63, 'case name bound')
        numbers([len(name), len(request.source), len(operations), *metadata['parameters']])
        output.extend(name)
        output.extend(request.source)
        heads = [0] * (1 << request.hash_log)
        nodes = [[0, 0] for _ in range(1 << (request.chain_log - 1))]
        for row in operations:
            require(row['before_heads'] == heads and row['before_nodes'] == nodes, 'reconstructible before maps')
            numbers([row[k] for k in ('query', 'before_frontier', 'after_frontier', 'window_floor_raw',
                                      'tree_floor_raw', 'length', 'offset_base')])
            heads, nodes = row['after_heads'], row['after_nodes']
            numbers(heads)
            numbers([v for pair in nodes for v in pair])
        provenance.append(dict(name=case.name,packet_sha256=sha(packet),stdout_sha256=sha(data),operations=len(operations)))
    require(len(output) <= (32 << 20), 'bounded fixture extent')
    return bytes(output), dict(schema_version=1,archive=ARCHIVE,archive_sha256=ARCHIVE_SHA,
                              upstream_revision=g.REVISION,cases=provenance,
                              qualification='First-padding normal records; complete padding/build equality and ledger validated from unchanged captured bytes. Redundant before maps reconstruct from zero initial maps and preceding after maps.')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--write', action='store_true')
    parser.add_argument('--directory', type=Path, default=DESTINATION)
    args = parser.parse_args()
    try:
        data, provenance = project()
        provenance.update(bytes=len(data),sha256=sha(data),projector_sha256=sha(Path(__file__).read_bytes()))
        encoded = (json.dumps(provenance,sort_keys=True,indent=2)+'\n').encode()
        if args.write:
            require(not args.directory.exists(), 'fresh fixture directory')
            args.directory.mkdir(parents=True)
            (args.directory/'cases.bin').write_bytes(data)
            (args.directory/'provenance.json').write_bytes(encoded)
        else:
            require((args.directory/'cases.bin').read_bytes() == data, 'fixture projection bytes')
            require((args.directory/'provenance.json').read_bytes() == encoded, 'fixture provenance')
        print(json.dumps(dict(cases=len(provenance['cases']),operations=sum(r['operations'] for r in provenance['cases']),bytes=len(data),sha256=sha(data)),sort_keys=True))
    except (ValueError, OSError, KeyError, TypeError) as error:
        parser.exit(1, 'Tree projection failed: '+str(error)+'\n')


if __name__ == '__main__':
    main()
