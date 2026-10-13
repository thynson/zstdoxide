#!/usr/bin/env python3
"""Project immutable C price records into bounded Rust fixtures; never execute C."""
import argparse
import hashlib
import io
import json
from pathlib import Path, PurePosixPath
import struct
import tarfile
from oracles.optimal_price_recipes import recipes
from oracles import optimal_price_validation as model
from oracles.optimal_prices_v2 import require

ROOT = Path(__file__).resolve().parents[1]
ARCHIVE = 'docs/evidence/2026-10-13-optimal-price-capture.tar.gz'
ARCHIVE_SHA = '0e47689b8ffe271d2f714f3027386b8200a5e92faa93fc1e9dc7cfa6a7f96427'
DEST = ROOT / 'tests/fixtures/reference-optimal-prices'
MAGIC = b'ZOPFX01\0'


def sha(data):
    return hashlib.sha256(data).hexdigest()


def load(data):
    return json.loads(data, object_pairs_hook=model.unique, parse_constant=model.invalid_constant)


def project():
    path = ROOT / ARCHIVE
    require(path.is_file() and not path.is_symlink() and path.stat().st_size == 9484451, 'bounded original archive')
    raw = path.read_bytes()
    require(sha(raw) == ARCHIVE_SHA, 'immutable archive identity')
    prefix = 'optimal-price-capture/'
    members = {}
    total = 0
    with tarfile.open(fileobj=io.BytesIO(raw), mode='r:gz') as archive:
        for entry in archive:
            name = PurePosixPath(entry.name)
            require(entry.isfile() and entry.name.startswith(prefix) and not name.is_absolute()
                    and '..' not in name.parts and str(name) == entry.name and '\\' not in entry.name
                    and entry.name not in members, 'regular unique archive member')
            total += entry.size
            require(0 <= entry.size <= 8 << 20 and total <= 48 << 20 and len(members) < 6008, 'archive limits')
            data = archive.extractfile(entry).read()
            require(len(data) == entry.size, 'complete member')
            members[entry.name] = data
    require(len(members) == 6008, 'complete archive inventory')
    for name, data in members.items():
        if name in {prefix + n for n in ('file-map.json', 'report.json', 'reconstruct.py')}:
            continue
        require(0 < len(data) <= 4096 and name == prefix + 'chunks/' + sha(data) + '.bin', 'chunk identity')
    mapping = load(members[prefix + 'file-map.json'])
    require(len(mapping) == 1017, 'complete retained file map')

    def stored(name):
        row = mapping[name]
        require(type(row['bytes']) is int and 0 <= row['bytes'] <= 8 << 20
                and len(row['chunks']) <= 2048, 'selected file bound')
        data = b''.join(members[prefix + 'chunks/' + key + '.bin'] for key in row['chunks'])
        require(len(data) == row['bytes'] and sha(data) == row['sha256'], 'selected file identity')
        return data

    evidence = load(stored('capture/evidence.json'))
    require(evidence['status'] == 'passed' and evidence['source_unchanged'] is True
            and evidence['audit']['recipes'] == 147, 'captured qualification')
    local = ['scripts/oracles/optimal_prices_v2.py', 'scripts/oracles/optimal_price_recipes.py',
             'scripts/oracles/optimal_price_validation.py', 'tests/fixtures/reference-optimal-price-recipes/recipes.json']
    for name in local:
        require(sha((ROOT / name).read_bytes()) == evidence['source'][name]
                and stored('capture/source/' + name) == (ROOT / name).read_bytes(), 'frozen replay dependency')
    cases = recipes()
    pack = bytearray(MAGIC + struct.pack('<I', len(cases)))
    catalog = []
    for recipe in cases:
        request = recipe.request
        require(stored('capture/requests/' + recipe.name + '.bin') == request.packet(), 'recorded request identity')
        logs = [stored('capture/logs/' + mode + '-run-' + recipe.name + '.stdout') for mode in ('normal', 'sanitized')]
        require(logs[0] == logs[1], 'complete C build equality')
        model.validate_output(logs[0], request)
        observed = [load(line) for line in logs[0].splitlines()][1:-1]
        start = len(pack)
        name = recipe.name.encode('ascii')
        pack.extend(struct.pack('<4I', len(name), len(request.source), len(request.operations), request.literal_mode))
        pack.extend(name)
        pack.extend(request.source)
        for operation, row in zip(request.operations, observed):
            pack.extend(struct.pack('<6I', *operation))
            words = [int(row['price_type'] == 'predefined')]
            for key in model.KEYS:
                words.extend(row['frequencies'][key])
            words.extend(row['sums'][key] for key in model.KEYS)
            words.extend(row['base_prices'][key] for key in model.KEYS)
            words.extend([int(row['query'] is not None), *(row['query'] or [0, 0, 0])])
            require(len(words) == 390, 'fixed observation width')
            pack.extend(struct.pack('<390I', *words))
        catalog.append(dict(id=recipe.name, offset=start, bytes=len(pack) - start,
                            source_sha256=sha(request.source), request_sha256=sha(request.packet()),
                            transcript_sha256=sha(logs[0]), operations=len(observed)))
    provenance = dict(schema_version=1, archive=ARCHIVE, archive_sha256=ARCHIVE_SHA,
                      captured_evidence_sha256=sha(stored('capture/evidence.json')),
                      cases=len(cases), operations=sum(c['operations'] for c in catalog),
                      source_bytes=sum(len(r.request.source) for r in cases), pack_bytes=len(pack),
                      pack_sha256=sha(pack), catalog=catalog,
                      qualification='Complete audited normal/instrumented C observations, independently replayed before projection. No synthesized expected prices, production Rust implementation or public Optimal support.')
    return bytes(pack), provenance


def main(write=False):
    pack, provenance = project()
    expected = {'cases.bin': pack, 'provenance.json': (json.dumps(provenance, sort_keys=True, indent=2) + '\n').encode()}
    if write:
        require(not DEST.exists(), 'fresh fixture directory')
        DEST.mkdir(parents=True)
        for name, data in expected.items():
            (DEST / name).write_bytes(data)
    for name, data in expected.items():
        path = DEST / name
        require(path.is_file() and not path.is_symlink() and path.read_bytes() == data, 'exact fixture projection')
    print(json.dumps({k: v for k, v in provenance.items() if k != 'catalog'}, sort_keys=True, indent=2))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--write', action='store_true')
    main(parser.parse_args().write)
