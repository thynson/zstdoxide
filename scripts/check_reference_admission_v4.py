#!/usr/bin/env python3
"""Freeze Lazy/Lazy2 admission over unchanged inputs without codec execution."""
from pathlib import Path
import argparse
import collections
import csv
import hashlib
import io
import json
import re
import tarfile

ROOT = Path(__file__).resolve().parents[1]
DESTINATION = ROOT / 'tests/fixtures/reference-admission-v4'
PINS = {
  "scripts/check_reference_admission_v3.py": "af3818761663797bfd748c50a2f844dca4512d5d274ea367450bcb95f64681ee",
  "scripts/check_greedy_interop_recipes.py": "f7d699761dfb612a8fa619f8360452205d700b4636c2c3fed920814cc2c87009",
  "scripts/oracles/greedy_interop.py": "c1eac37d63179fc02dcb2c6ac21d46b3810eb96763c782b59799973db07e6c7a",
  "docs/evidence/2026-10-12-greedy-public-interop.json": "b793c63accfbd2fcd2c013028c4f872282662d8ccf23b8bcf52d9f7b84e24c9a",
  "docs/evidence/2026-10-12-greedy-public-interop.tar.gz": "3440015eaf7d12504729b6cd225de36dec0543062c78bddb09381cf4deeefee8",
  "tests/fixtures/reference-greedy-interop-recipes/recipes.json": "3f14af699a7128c0e7a504b73190f99803b1ee9ad2772df69ea2ee42cc2cd1ac",
  "tests/fixtures/reference-admission-v3/README.md": "56603e97ef8b1b1021dc00ce0fa2e72635fc97df897c644f3a06b6097ce829aa",
  "tests/fixtures/reference-admission-v3/broader.tsv": "545ab96911821c87a0dafe2d8ad06b5bdc83de2da8ff08a7a9e69d791c4db24c",
  "tests/fixtures/reference-admission-v3/fuzz.tsv": "e7c8b01438e2bb35cc66a81d42ad4fa2f02203d222c25600efc2205267dae611",
  "tests/fixtures/reference-admission-v3/interop.tsv": "d1297efaeb9496515f41aaab46bc1a903dd652710234bbac4fbc7306463a7f24",
  "tests/fixtures/reference-admission-v3/loops.tsv": "bd35c8a40cc7f0bb01dc7393f1d072ff1240e923ffabe26cea24733e613d410e",
  "tests/fixtures/reference-admission-v3/provenance.json": "7e3f6d14a546390c525b00e1578e5e83d38f4f3ad5e4f95a2e1b1fc32b5e785f"
}
DATA_SHA = {
  "broader.tsv": "b859aaaa9dcc7ccafd3cad3a859524870e9d570fcef86bf8511812471122f7ba",
  "fuzz.tsv": "d82a887c2e9aff5848d2aa9385ced786b91422b239d4817a233b87b97e888f45",
  "greedy.tsv": "3fea6e0c916a1fd8d391f3473fb490db07dc8d746559d57b01e958da7063c765",
  "interop.tsv": "175ee2f09d3646bdf1e27083686eac4d7bb38b593c9dfc1daeed750f46d41110",
  "loops.tsv": "84a06c0ff6794d2e5c5eccc8fb160252dbd72511c6634985a18fa06e1ba92d85"
}


def require(value, reason):
    if not value:
        raise ValueError(reason)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def encoded(value):
    return (json.dumps(value, sort_keys=True, indent=2) + '\n').encode()


def table(data):
    return list(csv.DictReader(io.StringIO(data.decode('ascii')), delimiter='\t'))


def tsv(fields, rows):
    stream = io.StringIO(newline='')
    writer = csv.DictWriter(stream, fields, delimiter='\t', lineterminator='\n')
    writer.writeheader()
    writer.writerows(rows)
    return stream.getvalue().encode('ascii')


def classify(values):
    require(type(values) is list and len(values) == 7
            and all(type(n) is int for n in values), 'exact scalar tuple')
    limits = [(10, 31), (6, 30), (6, 30), (1, 30), (3, 7), (0, 131072), (1, 9)]
    require(all(lo <= n <= hi for n, (lo, hi) in zip(values, limits)), 'valid scalar tuple')
    window, chain, table_log, search, _, _, strategy = values
    if strategy not in (1, 2, 3, 4, 5):
        return 'unsupported'
    if strategy == 1:
        return 'resource' if window > 20 or table_log > 18 else 'success'
    if strategy == 2:
        return 'resource' if window > 21 or table_log > 18 or chain > 18 else 'success'
    return 'resource' if window > 21 or table_log > 19 or (window <= 14 and (chain > 18 or search > 8)) else 'success'


def archived_greedy():
    # Sequential bounded data-only reading. No archive extraction or execution.
    prefix = 'greedy-public-interop/'
    chunks = {}
    metadata = {}
    seen = set()
    total = 0
    with tarfile.open(ROOT / 'docs/evidence/2026-10-12-greedy-public-interop.tar.gz', 'r|gz') as archive:
        for item in archive:
            require(item.isfile() and item.name.startswith(prefix) and item.name not in seen,
                    'unique regular prefixed member')
            seen.add(item.name)
            name = item.name[len(prefix):]
            cap = 4096 if name.startswith('chunks/') else 64 << 20
            require(0 <= item.size <= cap, 'bounded member')
            data = archive.extractfile(item).read(cap + 1)
            require(len(data) == item.size, 'complete member bytes')
            total += len(data)
            require(total <= 96 << 20 and len(seen) <= 20000, 'bounded aggregate archive')
            if name.startswith('chunks/'):
                digest = sha(data)
                require(name == 'chunks/' + digest + '.bin', 'chunk content identity')
                chunks[digest] = data
            else:
                metadata[name] = data
    require(set(metadata) == {'file-map.json', 'report.json', 'reconstruct.py'}, 'metadata inventory')
    mapping = json.loads(metadata['file-map.json'])
    report = json.loads(metadata['report.json'])
    require(sha(metadata['file-map.json']) == report['file_map_sha256'], 'file map identity')
    record = mapping['capture/evidence.json']
    require(type(record['bytes']) is int and 0 < record['bytes'] <= 16 << 20
            and len(record['chunks']) == (record['bytes'] + 4095) // 4096, 'bounded evidence chunks')
    require(all(type(k) is str and re.fullmatch('[0-9a-f]{64}', k) and k in chunks
                for k in record['chunks']), 'present evidence chunk keys')
    data = b''.join(chunks[k] for k in record['chunks'])
    require(len(data) == record['bytes'] and sha(data) == record['sha256'], 'evidence identity')
    value = json.loads(data)
    require(value['status'] == 'passed' and value['source_before'] == value['source_after']
            and value['upstream_revision'] == 'f8745da6ff1ad1e7bab384bd1f9d742439278e99',
            'completed historical capture')
    return value, sha(data)


def projection():
    for name, digest in PINS.items():
        p = ROOT / name
        require(p.is_file() and not p.is_symlink() and sha(p.read_bytes()) == digest,
                'pinned input before import: ' + name)
    import check_reference_admission_v3 as prior
    previous = prior.projection()
    prior.verify(previous)
    import check_greedy_interop_recipes as recipes_check
    recipes_check.check()
    from oracles import greedy_interop as generator
    presets = {(int(r['tier']), int(r['row'])):
               [int(r[k]) for k in ('window_log', 'chain_log', 'hash_log', 'search_log',
                                     'min_match', 'target_length', 'strategy')]
               for r in table((ROOT / 'tests/upstream/parameters/presets.tsv').read_bytes())}
    files = {}
    coverage = {}
    changed = {}
    for filename in ('fuzz.tsv', 'loops.tsv', 'interop.tsv', 'broader.tsv'):
        rows = table(previous[filename])
        fields = list(rows[0])
        fields.remove('v2')
        fields.insert(fields.index('v3') + 1, 'v4')
        for row in rows:
            row.pop('v2')
            values = None if row['v3'] == 'advanced' else (presets[int(row['preset_tier']), int(row['preset_row'])]
                      if filename in ('fuzz.tsv', 'loops.tsv') else
                      list(map(int, row['historical_resolved_tuple'].split(','))))
            row['v4'] = 'advanced' if row['v3'] == 'advanced' else classify(values)
            if filename in ('fuzz.tsv', 'loops.tsv'):
                require(row['v4'] != 'resource', 'selector/numeric presets fit before reduction')
            if 'additive_c_required' in row:
                row['additive_c_required'] = int(row['v3'] != 'success' and row['v4'] == 'success')
        files[filename] = tsv(fields, rows)
        coverage[filename] = {v: dict(collections.Counter(r[v] for r in rows)) for v in ('v3', 'v4')}
        changed[filename] = [r for r in rows if r['v3'] != r['v4']]
    capture, evidence_sha = archived_greedy()
    recipes = generator.recipes()
    require(len(recipes) == len(capture['cases']) == 422, 'all historical Greedy inputs')
    rows = []
    for recipe, observed in zip(recipes, capture['cases'], strict=True):
        require(encoded(recipe.metadata()) == encoded(observed['recipe']), 'unchanged source/request identity')
        require(observed['rust']['safe'] == observed['rust']['all'], 'historical backend equality')
        resolved = observed['rust']['safe']['resolved']
        old = prior.classify(resolved)
        require(old == recipe.expected == observed['rust']['safe']['status'], 'historical admission')
        if old != 'success':
            require(observed['c'] == {'status': 'not-run-for-rejected-Rust-profile'}, 'unexecuted historical C')
        new = classify(resolved)
        rows.append(dict(case=recipe.name, source_sha256=sha(recipe.source), source_size=len(recipe.source),
                         request=','.join(map(str, recipe.request)), historical_resolved_tuple=','.join(map(str, resolved)),
                         v3=old, v4=new, additive_c_required=int(old != 'success' and new == 'success')))
    files['greedy.tsv'] = tsv(list(rows[0]), rows)
    coverage['greedy.tsv'] = {v: dict(collections.Counter(r[v] for r in rows)) for v in ('v3', 'v4')}
    changed['greedy.tsv'] = [r for r in rows if r['v3'] != r['v4']]
    require({n: sha(b) for n, b in files.items()} == DATA_SHA,
            'frozen successor data: ' + json.dumps({n: sha(b) for n, b in files.items()}, sort_keys=True))
    files['provenance.json'] = encoded(dict(
        schema_version=4, upstream_revision=generator.REVISION,
        generator=str(Path(__file__).relative_to(ROOT)), generator_sha256=sha(Path(__file__).read_bytes()),
        input_sha256=PINS, greedy_capture_evidence_sha256=evidence_sha, coverage=coverage, changed=changed,
        fixtures=[dict(path=n, bytes=len(b), sha256=sha(b)) for n, b in sorted(files.items())],
        qualification='Source-only expectations before public Lazy/Lazy2 admission. All 81 selectors, 43 numeric loops and 92/159/422 historical public requests are unchanged. Presets are pre-adjustment C values; archived tuples are historical Rust resolution observations. Newly positive C execution remains a separate obligation. No codec, compiler, archived program or child executes.'))
    return files


def verify(files, destination=DESTINATION):
    require(destination.is_dir() and not destination.is_symlink()
            and {p.name for p in destination.iterdir()} == set(files) | {'README.md'}, 'exact v4 inventory')
    require((destination / 'README.md').is_file() and not (destination / 'README.md').is_symlink(), 'regular documentation')
    for name, data in files.items():
        p = destination / name
        require(p.is_file() and not p.is_symlink() and p.read_bytes() == data, 'immutable successor: ' + name)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    action = parser.add_mutually_exclusive_group(required=True)
    action.add_argument('--write', action='store_true')
    action.add_argument('--check', action='store_true')
    args = parser.parse_args()
    if args.write:
        require(not DESTINATION.exists() and not DESTINATION.is_symlink(), 'fresh destination')
    files = projection()
    if args.write:
        DESTINATION.mkdir(parents=True)
        for name, data in files.items():
            with (DESTINATION / name).open('xb') as stream:
                stream.write(data)
    else:
        verify(files)
    print(json.dumps(json.loads(files['provenance.json'])['coverage'], sort_keys=True))


if __name__ == '__main__':
    main()
