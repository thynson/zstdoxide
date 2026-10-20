#!/usr/bin/env python3
"""Check source-derived gates/coordinates and preserved skipped ordinary states."""
import argparse
from dataclasses import asdict, replace
import hashlib
import itertools
import json
from pathlib import Path
import sys

sys.dont_write_bytecode = True
from oracles.ultra2_lifecycle import Coordinates, Entry, MAX_BLOCK, U32_MAX, skipped_ordinary
from oracles.ultra_fresh_recipes import recipes
from oracles.ultra_scalar import need, load

ROOT = Path(__file__).resolve().parents[1]


def sha(data):
    return hashlib.sha256(data).hexdigest()


def check(capture):
    children = []
    def guard(event, args):
        if event in ('subprocess.Popen', 'os.system', 'os.fork', 'os.posix_spawn', 'os.exec', 'os.spawn'):
            children.append(event)
            raise RuntimeError('child execution forbidden')
    sys.addaudithook(guard)
    rejected = []
    def reject(name, call):
        try:
            call()
        except (ValueError, TypeError):
            rejected.append(name)
        else:
            raise ValueError('accepted malformed lifecycle: ' + name)

    gates = []
    for predicates in itertools.product((False, True), repeat=5):
        first, empty, no_dictionary, at_start, long = predicates
        entry = Entry(0 if first else 1, 0 if empty else 1, 2,
                      2 if no_dictionary else 1, 2 if at_start else 3,
                      9 if long else 8, 2)
        need(entry.predicates() == predicates, 'five independent predicates')
        need(entry.eligible() == all(predicates), 'exact native conjunction')
        gates.append(dict(entry=asdict(entry), predicates=predicates, eligible=entry.eligible()))
    base = Entry(0, 0, 2, 2, 2, 64, 2)
    for size in (0, 1, 7, 8, 9, 64, 16384, MAX_BLOCK):
        entry = replace(base, source_bytes=size)
        need(entry.eligible() == (size > 8), 'strict predefined threshold')
    for name in asdict(base):
        for bad in (-1, True, 1.0, U32_MAX + 1):
            reject(name + '-' + repr(bad), lambda n=name, v=bad: replace(base, **{n: v}).validate())
    reject('oversized-block', lambda: replace(base, source_bytes=MAX_BLOCK + 1).validate())
    reject('unordered-limits', lambda: replace(base, low_limit=3).validate())
    reject('current-before-dictionary', lambda: replace(base, current=1).validate())
    reject('source-overflow', lambda: replace(base, current=U32_MAX).validate())
    for next_update in (0, 3, U32_MAX):
        entry = replace(base, next_to_update=next_update)
        need(entry.eligible(), 'frontier is not a sixth predicate')
        reject('helper-frontier-' + str(next_update), entry.seeding_contract)
    for next_update in (1, 2):
        replace(base, next_to_update=next_update).seeding_contract()
    overflow = Entry(0, 0, U32_MAX - 64, U32_MAX - 64, U32_MAX - 64, 64, U32_MAX - 64)
    need(overflow.eligible(), 'input interval fits before rebase')
    reject('second-end-overflow', overflow.rebased)

    coordinates = []
    for size in (9, 64, 65, 1024, 16384, MAX_BLOCK):
        entry = replace(base, source_bytes=size)
        old = Coordinates(2, size)
        new = old.rebase()
        rebased = entry.rebased()
        need(new.origin == rebased.current == rebased.low_limit == rebased.dictionary_limit
             == rebased.next_to_update == 2 + size, 'all reset coordinates agree')
        # Exhaust every old byte, not just a chosen stale index. The old end is
        # excluded: it becomes the new start and was never an old readable byte.
        need(all(index < new.origin for index in range(old.origin, old.absolute(size))),
             'entire first-pass interval is stale')
        for index in (0, 1, 2, size + 1):
            reject(f'stale-{size}-{index}', lambda i=index: new.offset(i))
        for offset in (0, 1, size - 1):
            need(new.offset(new.absolute(offset)) == offset, 'coordinate round trip')
        need(new.offset(new.absolute(size), 0) == size, 'empty range at end')
        reject('end-read-' + str(size), lambda: new.offset(new.absolute(size)))
        reject('cross-end-' + str(size), lambda: new.offset(new.absolute(size - 1), 2))
        reject('forward-candidate-' + str(size), lambda: new.candidate(new.origin + 1, new.origin))
        reject('self-candidate-' + str(size), lambda: new.candidate(new.origin, new.origin))
        need(new.candidate(new.origin, new.origin + 1) == 0, 'strict predecessor')
        aliases = []
        for chain_log in range(6, 13):
            mask = (1 << (chain_log - 1)) - 1
            stale = new.origin - (mask + 1)
            if stale >= old.origin:
                need(stale & mask == new.origin & mask, 'physical ring alias')
                reject(f'ring-alias-{size}-{chain_log}', lambda i=stale: new.offset(i))
                aliases.append(chain_log)
        coordinates.append(dict(source_bytes=size, old=asdict(old), new=asdict(new), ring_alias_logs=aliases))
    for bad in (True, -1, 1.0, U32_MAX + 1):
        reject('coordinate-origin-' + repr(bad), lambda v=bad: Coordinates(v, 64).validate())
        reject('coordinate-offset-' + repr(bad), lambda v=bad: Coordinates(2, 64).absolute(v))
        reject('coordinate-width-' + repr(bad), lambda v=bad: Coordinates(2, 64).offset(2, v))

    provenance = load((ROOT / 'tests/fixtures/reference-ultra-fresh/provenance.json').read_bytes())
    need(sha((capture / 'evidence.json').read_bytes()) == provenance['capture_evidence_sha256'],
         'unchanged previously audited ordinary capture')
    preserved = {row['name']: row for row in provenance['cases']}
    cases = []
    for name, recipe in recipes().items():
        if recipe.request.policy != 2:
            continue
        packet = recipe.request.packet()
        need(sha(packet) == preserved[name]['packet_sha256']
             and packet == (capture / 'requests' / (name + '.bin')).read_bytes(),
             'unchanged ordinary request')
        entry = replace(base, sequence_count=1, source_bytes=len(recipe.request.original.source))
        rows, coverage = skipped_ordinary(recipe.request, recipe.kinds, entry)
        data = (capture / 'logs' / ('normal-run-' + name + '.stdout')).read_bytes()
        need(sha(data) == preserved[name]['transcript_sha256'], 'pinned native ordinary transcript')
        need(data == (capture / 'logs' / ('sanitized-run-' + name + '.stdout')).read_bytes(),
             'native normal/sanitized equality')
        observed = []
        for line in data.splitlines():
            row = load(line)
            if row.get('padding_pass') != 0:
                continue
            for key in ('padding_pass', 'source_unchanged', 'guards_unchanged', 'path_tail_zero'):
                row.pop(key, None)
            observed.append(row)
        need(json.dumps(rows, sort_keys=True) == json.dumps(observed, sort_keys=True),
             'complete unchanged ordinary append observations')
        cases.append(dict(name=name, packet_sha256=sha(packet), ordinary_transcript_sha256=sha(data),
                          rows=len(rows), events=sum(row['record'] == 'event' for row in rows),
                          coverage=sorted(coverage)))
        print('reconciled ' + name, file=sys.stderr, flush=True)
    need(len(cases) == 22 and not children, 'all ordinary Ultra cases without child execution')
    return dict(status='passed', optimized=not __debug__, gates=gates, coordinates=coordinates,
                rejected=rejected, skipped_cases=cases, child_processes=0,
                qualification='Source-derived five-predicate eligibility and checked integer rebasing. '
                'All 22 unchanged ordinary Ultra parser transcripts reconcile under a sequence-prefix-only '
                'skip hypothesis. No native Ultra2 wrapper, seeded second pass, existing prefix payload '
                'or public strategy-9 admission is claimed.')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('ordinary_capture', type=Path)
    args = parser.parse_args()
    print(json.dumps(check(args.ordinary_capture), sort_keys=True, indent=2))
