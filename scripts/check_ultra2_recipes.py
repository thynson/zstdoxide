#!/usr/bin/env python3
"""Qualify one frozen source-only recipe without native or Rust execution."""
import argparse
import hashlib
import json
from pathlib import Path
import sys

sys.dont_write_bytecode = True
from oracles.ultra2_recipes import recipes
from oracles.ultra2_protocol import Request, canonical, parse_packet
from oracles.ultra2_validation import records
from oracles.ultra_scalar import need, load
from oracles.ultra_fresh_recipes import recipes as ordinary

ROOT = Path(__file__).resolve().parents[1]


def sha(data):
    return hashlib.sha256(data).hexdigest()


def check(name, capture):
    children = []
    def guard(event, args):
        if event in ('subprocess.Popen', 'os.system', 'os.fork', 'os.posix_spawn', 'os.exec', 'os.spawn'):
            children.append(event)
            raise RuntimeError('recipe worker cannot execute children')
    sys.addaudithook(guard)
    recipe = recipes()[name]
    request = Request(recipe.request, recipe.seed)
    packet = request.packet()
    need(parse_packet(packet) == request, 'exact complete packet round trip')
    rows, coverage = records(recipe.request, recipe.seed)
    events = [row for row in rows if row['record'] == 'event']
    ends = [row for row in rows if row['record'] == 'pass_end']
    begins = [row for row in rows if row['record'] == 'pass_begin']
    resets = [row for row in rows if row['record'] == 'reset']
    seeded = recipe.seed and len(recipe.request.source) > 8
    need(len(ends) == len(begins) == (2 if seeded else 1)
         and len(resets) == int(seeded), 'exact phase inventory')
    if seeded:
        need(ends[0]['frequency'] == resets[0]['frequency'] == begins[1]['frequency'],
             'retain complete frequency state across discarded output reset')
        need(begins[1]['repeats'] == resets[0]['caller_repeats'] == [1, 4, 8]
             and ends[0]['candidate_repeats'] == resets[0]['discarded_repeats'],
             'restore original repeats and retain discarded candidates as observations')
        need(resets[0]['origin_after'] == len(recipe.request.source) + 2
             and resets[0]['discarded_sequences'] == len(ends[0]['sequences']),
             'exact reset extent')
    predecessor = None
    if recipe.predecessor is not None:
        old = ordinary()[recipe.predecessor]
        need(recipe.request.source == old.request.original.source
             and recipe.request.packet() == old.request.original.packet(),
             'all original source/request bytes preserved')
        provenance = load((ROOT / 'tests/fixtures/reference-ultra-fresh/provenance.json').read_bytes())
        p = next(row for row in provenance['cases'] if row['name'] == recipe.predecessor)
        raw = (capture / 'logs' / ('normal-run-' + recipe.predecessor + '.stdout')).read_bytes()
        need(sha(raw) == p['transcript_sha256'], 'pinned ordinary native transcript')
        first_phase = 'seeding' if seeded else 'encoding'
        expected = iter(row for row in events if row['pass_name'] == first_phase)
        count = 0
        for line in raw.splitlines():
            row = load(line)
            if row.get('padding_pass') != 0 or row.get('record') != 'event':
                continue
            for key in ('padding_pass', 'source_unchanged', 'guards_unchanged', 'path_tail_zero'):
                row.pop(key, None)
            predicted = next(expected, None)
            need(predicted is not None, 'complete first-pass event inventory')
            predicted = {k: v for k, v in predicted.items() if k != 'pass_name'}
            need(canonical(row) == canonical(predicted), 'exact original native first-pass state')
            count += 1
        need(next(expected, None) is None, 'no additional first-pass events')
        predecessor = dict(name=recipe.predecessor, packet_sha256=sha(old.request.packet()),
                           transcript_sha256=sha(raw), events=count)
    h = hashlib.sha256()
    for row in rows:
        h.update((canonical(row) + '\n').encode())
    need(not children, 'no native/Rust/child execution inside recipe worker')
    return dict(name=name, seed=recipe.seed, source_sha256=sha(recipe.request.source),
                source_bytes=len(recipe.request.source), packet_sha256=sha(packet),
                packet_bytes=len(packet), rows=len(rows), events=len(events),
                rows_sha256=h.hexdigest(), passes=len(ends),
                changed_path=seeded and ends[0]['sequences'] != ends[1]['sequences'],
                sequence_counts=[len(row['sequences']) for row in ends],
                predecessor=predecessor, coverage=sorted(coverage), child_processes=0,
                optimized=not __debug__, status='passed')


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('name', choices=recipes())
    p.add_argument('ordinary_capture', type=Path)
    args = p.parse_args()
    print(json.dumps(check(args.name, args.ordinary_capture), sort_keys=True, indent=2))
