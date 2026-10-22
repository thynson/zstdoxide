#!/usr/bin/env python3
"""Pure phase/block controls and complete immutable predecessor reconciliation."""
from dataclasses import asdict, replace
from pathlib import Path
import argparse
import copy
import hashlib
import json
import struct
import sys
import tarfile
sys.dont_write_bytecode = True

from oracles.ultra2_retained import Request, SourceRequest, parse_packet, MAX_PACKET, need
from oracles.ultra2_retained_validation import records, observations, validate_output, Search, Prefix
from oracles.ultra2_protocol import canonical
from oracles.ultra_scalar import load

ROOT = Path(__file__).resolve().parents[1]


def digest(path):
    h = hashlib.sha256()
    with path.open('rb') as stream:
        for part in iter(lambda: stream.read(65536), b''):
            h.update(part)
    return h.hexdigest()


def encoded(rows):
    return b''.join((canonical(row) + '\n').encode() for row in rows)


def controls():
    rejected = []
    def reject(name, call):
        need(name not in rejected, 'distinct control')
        try:
            call()
        except (ValueError, TypeError):
            rejected.append(name)
        else:
            raise ValueError('accepted negative control: ' + name)
    original = SourceRequest(b'abcd' * 48, (64, 64, 64), min_match=3, target=128)
    request = Request(original)
    packet = request.packet()
    need(parse_packet(packet) == request, 'positive complete packet')
    for name, value in [('empty', b''), ('short', packet[:60]), ('truncated', packet[:-1]),
                        ('trailing', packet + b'!'), ('magic', b'!' + packet[1:]),
                        ('mutable', bytearray(packet)), ('oversize', b'0' * (MAX_PACKET + 1))]:
        reject(name, lambda v=value: parse_packet(v))
    for offset, value in [(8, 32769), (12, 0), (12, 5), (16, 2), (20, 1), (20, 0),
                          (24, 9), (24, 17), (28, 5), (28, 13), (32, 5), (32, 13),
                          (36, 0), (36, 7), (40, 2), (40, 8), (44, 4096), (48, 2),
                          (52, 6), (52, 8193)]:
        bad = packet[:offset] + struct.pack('<I', value) + packet[offset + 4:]
        reject(f'field-{offset}-{value}', lambda v=bad: parse_packet(v))
    for field in ('window_log', 'hash_log', 'chain_log', 'search_log', 'min_match', 'target', 'literal_mode'):
        reject('boolean-' + field, lambda f=field: replace(original, **{f: True}).validate())
    for value in (0, 1, None, 'true'):
        reject('bootstrap-' + repr(value), lambda v=value: replace(request, bootstrap=v).validate())
    reject('policy-bool', lambda: replace(request, policy=True).validate())
    reject('blocks-list', lambda: replace(original, blocks=[64, 64, 64]).validate())
    reject('block-bool', lambda: replace(original, blocks=(True, 127, 64)).validate())
    reject('source-mutable', lambda: replace(original, source=bytearray(original.source)).validate())
    kinds = (0, 2, 1)
    for bad in ([0, 2, 1], (0, 2), (0, True, 1), (0, 2, 3)):
        reject('publication-' + repr(bad), lambda v=bad: records(request, v))
    good = list(observations(request, kinds))
    raw = encoded(good)
    validate_output(raw, request, kinds)
    reset_index = next(i for i, row in enumerate(good) if row['record'] == 'reset')
    later = next(i for i, row in enumerate(good) if row.get('stage') == 'query' and row['block'] == 1)
    publication = next(i for i, row in enumerate(good) if row['record'] == 'block_publish' and row['block'] == 0)
    for name in ('missing-reset', 'extra-reset', 'full-source-rebase', 'later-phase', 'later-block',
                 'caller-repeats', 'discarded-count', 'frontier', 'table-preservation', 'frequency',
                 'later-query', 'later-query-bool', 'later-search', 'later-path', 'later-cache',
                 'publication-kind', 'publication-owner', 'candidate-repeats', 'padding',
                 'event-count', 'pass-count', 'coherent-frequency', 'coherent-origin', 'missing-row'):
        rows = copy.deepcopy(good)
        reset, event, pub = rows[reset_index], rows[later], rows[publication]
        if name == 'missing-reset':rows.pop(reset_index)
        elif name == 'extra-reset':rows.insert(reset_index, copy.deepcopy(reset))
        elif name == 'full-source-rebase':reset['origin_after'] = 2 + len(original.source)
        elif name == 'later-phase':event['pass_name'] = 'seeding'
        elif name == 'later-block':event['block'] = 0
        elif name == 'caller-repeats':reset['caller_repeats'][0] += 1
        elif name == 'discarded-count':reset['discarded_sequences'] += 1
        elif name == 'frontier':reset['next_to_update'] -= 1
        elif name == 'table-preservation':reset['tables_preserved'] = False
        elif name == 'frequency':reset['frequency']['tables']['ll'][0] += 1
        elif name == 'later-query':event['query'][0] += original.blocks[0]
        elif name == 'later-query-bool':event['query'][0] = True
        elif name == 'later-search':event['search']['length'] -= 1
        elif name == 'later-path':event['cells']['changes'].append([4098, [0]*7])
        elif name == 'later-cache':event['base_prices']['ll'] += 1
        elif name == 'publication-kind':pub['kind'] = 2
        elif name == 'publication-owner':pub['previous_owner_after'] = 1
        elif name == 'candidate-repeats':pub['candidate_repeats'][0] += 1
        elif name == 'padding':event['padding_pass'] = 1
        elif name == 'event-count':rows[-1]['events'] += 1
        elif name == 'pass-count':rows[-1]['algorithm_passes'] += 1
        elif name == 'coherent-frequency':
            for row in rows:
                if 'frequency' in row:
                    row['frequency']['tables']['ll'][0] += 1
                    row['frequency']['sums']['ll'] += 1
                if 'frequencies' in row:
                    row['frequencies']['ll'][0] += 1
                    row['sums']['ll'] += 1
        elif name == 'coherent-origin':
            for row in rows:
                for key in ('origin', 'origin_after', 'dictionary_limit', 'low_limit', 'next_to_update'):
                    if key in row and row[key] > 2:row[key] += 1
        else:rows.pop(later)
        changed = encoded(rows)
        need(changed != raw, 'effective forgery: ' + name)
        reject('trace-' + name, lambda data=changed: validate_output(data, request, kinds))
    for name, data in [('newline', raw[:-1]), ('extra', raw+b'{}\n'),
                       ('duplicate-key', raw.replace(b'"protocol":', b'"protocol":"x","protocol":', 1)),
                       ('nonfinite', raw.replace(b'"bias":2', b'"bias":NaN', 1))]:
        reject('trace-' + name, lambda v=data: validate_output(v, request, kinds))
    search = Search(original)
    search.rebase_first()
    view = search.request.source
    need(view.coordinates.origin == 66 and view[64] == original.source[0], 'first-block biased read')
    reject('stale-byte', lambda: view[0])
    reject('future-byte', lambda: view[128])
    reject('stale-slice', lambda: view[0:1])
    reject('repeated-rebase', search.rebase_first)
    reject('partial-prefix', lambda: Prefix(original, 65, 66))
    before = [r for r in good if r.get('block') == 0]
    changed_source = replace(original, source=original.source[:64] + b'z' * 128)
    after = [r for r in observations(Request(changed_source), kinds) if r.get('block') == 0]
    need(canonical(before) == canonical(after), 'future bytes cannot change first-block states')
    boundaries = []
    for first in (7, 8, 9, 64, 65):
        r = SourceRequest(b'a' * (first + 64), (first, 64), min_match=3)
        rows, coverage = records(Request(r), (0, 2))
        gates = [row for row in rows if row['record'] == 'eligibility']
        resets = [row for row in rows if row['record'] == 'reset']
        need([row['seed'] for row in gates] == [first > 8, False], 'first-only threshold')
        need(len(resets) == int(first > 8), 'one optional rebase')
        if resets:need(resets[0]['origin_after'] == first + 2, 'first-length bias')
        boundaries.append(dict(first_bytes=first, seeds=len(resets), records=len(rows), coverage=sorted(coverage)))
    from oracles.optimal_retained_frames import balanced_blocks
    for size in (16385, 24576, 24577, 32768):
        r = SourceRequest(b'a' * size, balanced_blocks(size), window_log=15)
        need(parse_packet(Request(r).packet()) == Request(r), 'public extent packet')
        rows, _ = records(Request(r), (2,) + (1,) * (len(r.blocks)-1))
        need(len([row for row in rows if row['record'] == 'reset']) == 1,
             'single rebase through maximum complete source')
        reject('window-' + str(size), lambda r=r: replace(r, window_log=14).validate())
        boundaries.append(dict(source_bytes=size, blocks=list(r.blocks), records=len(rows)))
    return dict(rejected=len(rejected), controls=rejected, boundaries=boundaries,
                positive_transcripts=2, qualification='Predictions only; no successor native execution.')


def projected(request, kinds, fresh):
    for row in observations(request, kinds):
        row = dict(row)
        record = row['record']
        if fresh:
            if record in ('block_begin', 'block_publish'):continue
            if record == 'metadata':
                row.update(protocol='ZU2PS01', seed=row.pop('bootstrap'))
                row.pop('blocks')
            elif record == 'complete':row.pop('blocks')
            elif record != 'event':row.pop('block')
        else:
            if record in ('eligibility', 'pass_begin', 'pass_end', 'reset'):continue
            if record == 'metadata':
                row.update(protocol='ZURTS01', passes=row.pop('padding_passes'))
                row.pop('bootstrap')
            elif record == 'complete':
                row.update(passes=row.pop('padding_passes'), price_policy=request.policy)
                row.pop('algorithm_passes')
            elif record == 'event':row.pop('pass_name')
        yield row


def predecessors():
    from oracles.ultra2_recipes import recipes as fresh_recipes
    from oracles.ultra_retained_recipes import recipes as retained_recipes
    results = []
    for fresh, name, capture in (
        (True, '2026-10-20-ultra2-native', Path('/private/tmp/zstdoxide-ultra2-capture-1020')),
        (False, '2026-10-21-ultra-retained-native', Path('/private/tmp/ultra-retained-capture'))):
        report = load((ROOT/'docs/evidence'/(name+'.json')).read_bytes())
        archive = ROOT/'docs/evidence'/report['archive']
        need(report['status'] == 'passed' and digest(archive) == report['archive_sha256'] and
             archive.stat().st_size == report['archive_bytes'], 'exact committed predecessor archive')
        mapping = None
        with tarfile.open(archive, 'r|gz') as tar:
            for member in tar:
                if member.name == 'file-map.json':
                    data = tar.extractfile(member).read()
                    need(hashlib.sha256(data).hexdigest() == report['file_map_sha256'], 'bound file map')
                    mapping = load(data)
                    break
        need(mapping is not None, 'complete predecessor map')
        need(digest(capture/'evidence.json') == mapping['capture/evidence.json']['sha256'],
             'local receipt bound to archived capture')
        old = load((capture/'evidence.json').read_bytes())
        need(old['status'] == 'passed' and old['source_unchanged'], 'qualified predecessor')
        cases = fresh_recipes() if fresh else retained_recipes()
        for case, recipe in cases.items():
            if fresh:
                request = Request(SourceRequest(**asdict(recipe.request)), recipe.seed)
                kinds = (2,)
            else:
                request = Request(SourceRequest(**asdict(recipe.request.original)), False, recipe.request.policy)
                kinds = recipe.kinds
            expected = projected(request, kinds, fresh)
            path = capture/'logs'/('normal-run-'+case+'.stdout')
            key = 'capture/' + str(path.relative_to(capture))
            identity = mapping[key]
            need(path.stat().st_size == identity['bytes'] and digest(path) == identity['sha256'],
                 'exact archived native transcript')
            other = key.replace('/normal-run-', '/sanitized-run-')
            need(mapping[other] == identity, 'identical native/sanitized transcripts')
            count = events = 0
            with path.open('rb') as stream:
                for row in expected:
                    line = stream.readline((2 << 20) + 1)
                    need(0 < len(line) <= 2 << 20 and line.endswith(b'\n'), 'bounded complete native row')
                    actual = load(line)
                    need(canonical(actual) == canonical(row),
                         f'exact predecessor {case} row {count} {row.get("record")} {row.get("stage")}')
                    count += 1
                    events += row['record'] == 'event'
                need(stream.read(1) == b'', 'complete native EOF')
            results.append(dict(case=case, fresh=fresh, records=count, events=events,
                                transcript_sha256=identity['sha256'], packet_sha256=hashlib.sha256(request.packet()).hexdigest()))
            print('Reconciled ' + case, file=sys.stderr, flush=True)
    need(len(results) == 95, 'all 53 fresh and 42 retained native predecessors')
    return dict(cases=results, events=sum(r['events'] for r in results),
                identical_native_build_transcripts=190)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--predecessors', action='store_true')
    args = parser.parse_args()
    children = []
    def guard(event, arguments):
        if event in ('subprocess.Popen', 'os.system', 'os.fork', 'os.posix_spawn', 'os.exec', 'os.spawn'):
            children.append(event)
            raise ValueError('child execution forbidden')
    sys.addaudithook(guard)
    result = predecessors() if args.predecessors else controls()
    need(not children, 'pure independent checks')
    source = {str(Path(m.__file__).resolve().relative_to(ROOT)): digest(Path(m.__file__).resolve())
              for m in list(sys.modules.values()) if getattr(m, '__file__', None)
              and Path(m.__file__).resolve().is_relative_to(ROOT/'scripts')
              and Path(m.__file__).suffix == '.py'}
    print(json.dumps(dict(status='passed', optimized=not __debug__, child_processes=0,
                         source=source, result=result), sort_keys=True, indent=2))


if __name__ == '__main__':main()
