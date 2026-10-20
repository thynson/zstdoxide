#!/usr/bin/env python3
"""Project audited retained C states with persistent deltas and publication owners."""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import tarfile
from oracles.optimal_retained_frames import require
from oracles.optimal_retained_frames_recipes import recipes
from oracles.optimal_retained_frames_validation import validate_output
from oracles.tree_validation import parse

ROOT = Path(__file__).resolve().parents[1]
DEST = ROOT / 'tests/fixtures/reference-optimal-retained-frames'
STAGES = ['begin','query','origin','seed','literal','relax','select','reverse_start','reverse','sequence','refresh','complete']
KEYS = ['lit','ll','ml','of']
NONE = 0xffffffff


def sha(data): return hashlib.sha256(data).hexdigest()
def enc(value): return (json.dumps(value, sort_keys=True, indent=2)+'\n').encode()
def words(*values):
    require(all(type(v) is int and 0 <= v <= NONE for v in values), 'u32 domain')
    return struct.pack('<'+'I'*len(values), *values)


def captured():
    report = parse((ROOT/'docs/evidence/2026-10-16-optimal-retained-wide-capture.json').read_bytes())
    path = ROOT/'docs/evidence'/report['archive']
    require(path.is_file() and not path.is_symlink() and path.stat().st_size == report['archive_bytes']
            and sha(path.read_bytes()) == report['archive_sha256'], 'immutable retained archive')
    expected = {r['path']: r for r in report['files']}
    require(len(expected) == len(report['files']), 'unique member inventory')
    stored = {}; total = 0
    with tarfile.open(path, 'r:gz') as archive:
        entries = archive.getmembers()
        require(len(entries) == len(expected) and {e.name for e in entries} == set(expected), 'complete inventory')
        for entry in entries:
            require(entry.isfile() and 0 <= entry.size <= 256 << 20, 'bounded regular member')
            total += entry.size; require(total <= 1 << 30, 'bounded expanded archive')
            data = archive.extractfile(entry).read(); row = expected[entry.name]
            require(len(data) == row['bytes'] and sha(data) == row['sha256'], 'exact member identity')
            if entry.name == 'evidence.json' or entry.name.startswith(('requests/','artifacts/','source/')) or ('-run-wide_' in entry.name and entry.name.endswith('.stdout')):
                stored[entry.name] = data
    evidence = parse(stored['evidence.json'])
    require(evidence['status'] == 'passed' and evidence['source_unchanged'] is True
            and evidence['audit'] == report['qualification'], 'complete independently audited capture')
    require(sum(n.endswith('.zst') for n in evidence['artifacts']) == 76
            and sum(n.endswith('.json') for n in evidence['artifacts']) == 76, 'complete successor inventory')
    for name, digest in evidence['source'].items():
        require(sha((ROOT/name).read_bytes()) == digest and sha(stored['source/'+name]) == digest, 'frozen capture source')
    return stored, report['archive_sha256']


def search_words(state):
    return state['heads']+[v for pair in state['nodes'] for v in pair]+state['short_heads']+[state['frontier'],state['short_frontier']]


def delta(pack, current, previous, width):
    require(len(current) == len(previous), 'fixed delta extent')
    changed = [(i, v) for i, v in enumerate(current) if v != previous[i]]
    pack.extend(words(len(changed)))
    for i, value in changed:
        pack.extend(words(i, *(value if width > 1 else [value])))
    return current


def observed_delta(pack,record,previous,width):
    require(type(record)is dict and set(record)=={'length','changes'} and record['length']==len(previous),'fixed complete sparse owner')
    require(type(record['changes'])is list and len(record['changes'])<=len(previous),'bounded sparse changes')
    pack.extend(words(len(record['changes'])));last=-1
    for index,value in record['changes']:
        require(type(index)is int and last<index<len(previous) and previous[index]!=value,'canonical changed index')
        if width>1:require(type(value)is list and len(value)==width,'complete sparse cell')
        pack.extend(words(index,*(value if width>1 else [value])))
        previous[index]=value.copy() if width>1 else value;last=index


def project():
    stored, digest = captured(); cases = recipes(); pack = bytearray(b'ZORFS03\0'+words(len(cases))); catalog = []
    for name, recipe in cases.items():
        request = recipe.request
        require(stored['requests/'+name+'.bin'] == request.packet(), 'exact immutable packet')
        raw = stored['logs/normal-run-'+name+'.stdout']
        require(raw == stored['logs/sanitized-run-'+name+'.stdout'], 'normal/sanitized equality')
        validate_output(raw, request, recipe.kinds)
        records = [parse(line) for line in raw.splitlines()]
        records = [r for r in records if r.get('padding_pass') == 0]
        frame = stored[f'artifacts/normal/{name}.0.zst']
        for mode in ['normal','sanitized']:
            for padding in [0,1]:
                require(stored[f'artifacts/{mode}/{name}.{padding}.zst'] == frame, 'all four frames equal')
        start = len(pack); encoded_name = name.encode('ascii')
        pack.extend(words(len(encoded_name), len(request.source), request.window_log, request.hash_log,
            request.chain_log, request.search_log, request.min_match, request.target, request.literal_mode,
            len(request.blocks), len(frame), 4099))
        for mode in ['normal','sanitized']:
            ledger = parse(stored[f'artifacts/{mode}/{name}.0.json'])
            require(len(ledger['allocations']) == 2 and ledger['released'] is True, 'two released owners')
            pack.extend(words(*ledger['allocations'], ledger['peak_bytes']))
        pack.extend(encoded_name); pack.extend(request.source); pack.extend(frame)
        paths = [[0]*7 for _ in range(4099)]; frequencies = [0]*386
        size = (1 << request.hash_log)+(1 << request.chain_log)+(1 << request.window_log if request.min_match == 3 else 0)+2
        search = [0]*size; block_catalog = []
        for block, length in enumerate(request.blocks):
            rows = [r for r in records if r['block'] == block]
            begin, publish = rows[0], rows[-1]; events = rows[1:-1]
            require(begin['record'] == 'block_begin' and publish['record'] == 'block_publish'
                    and all(e['record'] == 'event' for e in events), 'chronological block records')
            pack.extend(words(begin['start'], length, publish['kind'], publish['previous_owner_before'],
                publish['previous_owner_after'], begin['frontier'], len(events), *publish['before_repeats'],
                *publish['candidate_repeats'], *(v for owner in publish['owners'] for v in owner), publish['tail']))
            sequences = []
            for index, e in enumerate(events):
                require(e['index'] == index, 'ordered event indices')
                pack.extend(words(STAGES.index(e['stage']), int(e['phase'] == 'reverse'), e['ip'], e['anchor'],
                    *(NONE if e[k] is None else e[k] for k in ['cur','last','tail']), *e['repeats']))
                for key in ['stretch','reverse','query']:
                    pack.extend(words(int(e[key] is not None)))
                    if e[key] is not None: pack.extend(words(*e[key]))
                if e['query'] is not None:
                    pack.extend(words(len(e['matches'])))
                    for pair in e['matches']: pack.extend(words(*pair))
                observed_delta(pack,e['cells'],paths,7)
                current = [v for k in KEYS for v in e['frequencies'][k]]+[e['sums'][k] for k in KEYS]+[e['base_prices'][k] for k in KEYS]+[int(e['predefined'])]
                frequencies = delta(pack, current, frequencies, 1)
                require(e['sequences'][:len(sequences)] == sequences, 'append-only block sequences')
                added = e['sequences'][len(sequences):]
                pack.extend(words(len(e['sequences']), len(added)))
                for row in added: pack.extend(words(*row))
                sequences = e['sequences']
                if e['query'] is not None:
                    require(e['search_before']==dict(length=size,changes=[]), 'exact prior complete pre-query state')
                require(index == 0 or e['query'] is not None or not e['search']['changes'], 'only queries mutate search')
                observed_delta(pack,e['search'],search,1)
            block_catalog.append(dict(start=begin['start'], length=length, kind=publish['kind'], events=len(events),
                queries=sum(e['query'] is not None for e in events), sequences=len(sequences)))
        catalog.append(dict(name=name, offset=start, bytes=len(pack)-start, blocks=block_catalog,
            frame_bytes=len(frame), frame_sha256=sha(frame), packet_sha256=sha(request.packet()), transcript_sha256=sha(raw)))
    require(len(pack) <= 16 << 20 and sum(b['events'] for c in catalog for b in c['blocks']) == 67321, 'bounded complete projection')
    predecessor_path=ROOT/'tests/fixtures/reference-optimal-retained-wide/cases.bin'
    predecessor=predecessor_path.read_bytes()
    predecessor_meta=parse((predecessor_path.parent/'provenance.json').read_bytes())
    require(len(predecessor)==predecessor_meta['pack_bytes'] and sha(predecessor)==predecessor_meta['pack_sha256'],'immutable predecessor pack')
    require(pack[12:len(predecessor)]==predecessor[12:],'all seven predecessor case bytes preserved')
    provenance = dict(schema_version=1, archive_sha256=digest, pack_bytes=len(pack), pack_sha256=sha(pack), cases=catalog, predecessor_pack_sha256=sha(predecessor), predecessor_case_bytes=len(predecessor)-12,
        qualification='Independently audited larger sparse retained ordinary unchanged-provider C observations. Persistent path/frequency/search deltas and committed/pending publication owners; no public retained Optimal admission.')
    return bytes(pack), enc(provenance)


def main(write):
    pack, provenance = project()
    if write:
        require(not DEST.exists(), 'fresh immutable fixture directory'); DEST.mkdir(parents=True)
        (DEST/'cases.bin').write_bytes(pack); (DEST/'provenance.json').write_bytes(provenance)
    require((DEST/'cases.bin').read_bytes() == pack and (DEST/'provenance.json').read_bytes() == provenance, 'exact fixture reproduction')
    print(json.dumps(dict(status='passed', cases=19, events=67321, pack_bytes=len(pack), pack_sha256=sha(pack)), sort_keys=True))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--write', action='store_true')
    main(parser.parse_args().write)
