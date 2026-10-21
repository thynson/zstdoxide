#!/usr/bin/env python3
"""Reconcile every prior larger retained native state; test additive model boundaries.

No child process or codec execution is permitted. Policy-2 states are hypotheses
until a separately frozen observer campaign qualifies them.
"""
from pathlib import Path
import copy
import dataclasses
import hashlib
import json
import struct
import sys
import tarfile
sys.dont_write_bytecode = True

from oracles.ultra_retained import Request, Original, MAX_PACKET, parse_packet, require
from oracles.ultra_retained_validation import observations, records, validate_output, MAX_LINE
from oracles.optimal_retained_frames import balanced_blocks
from oracles.optimal_retained_frames_recipes import recipes
from oracles.optimal_retained_validation_v2 import SearchPrefix
from oracles.tree_validation import parse, same

ROOT = Path(__file__).resolve().parents[1]
ARCHIVE_SHA = '750e3a3624c78b9d189ab93249085f96736667d25f857156ae1e065e05e4e985'


def sha_file(path):
    h = hashlib.sha256()
    with path.open('rb') as stream:
        while part := stream.read(65536):
            h.update(part)
    return h.hexdigest()


def encoded(rows):
    return b''.join((json.dumps(r, sort_keys=True, separators=(',', ':'))+'\n').encode()
                    for r in rows)


def predecessor_projection(row):
    row = dict(row)
    require(row.pop('price_policy') == 0, 'project only policy zero')
    if row['record'] == 'metadata':
        require(row['protocol'] == 'ZURTS01', 'successor metadata')
        row['protocol'] = 'ZOPRS03'
    elif row['record'] == 'event':
        require(row.pop('alternative') is None, 'no policy-zero alternative')
    return row


def native_predecessors():
    report_path = ROOT/'docs/evidence/2026-10-20-optimal-retained-frames-capture.json'
    report = parse(report_path.read_bytes())
    archive = report_path.parent/report['archive']
    require(report['status'] == 'passed' and report['archive_sha256'] == ARCHIVE_SHA
            and archive.stat().st_size == report['archive_bytes']
            and sha_file(archive) == ARCHIVE_SHA, 'pinned native predecessor archive')
    inventory = {r['path']: r for r in report['files']}
    require(len(inventory) == len(report['files']), 'distinct predecessor members')
    cases = recipes()
    wanted = {'logs/normal-run-'+name+'.stdout': name for name in cases}
    seen = set()
    results = []
    with tarfile.open(archive, 'r|gz') as tar:
        for member in tar:
            require(member.isfile() and member.name in inventory and member.name not in seen,
                    'exact regular predecessor member')
            seen.add(member.name)
            identity = inventory[member.name]
            require(member.size == identity['bytes'] and 0 <= member.size <= 256 << 20,
                    'bounded predecessor member')
            stream = tar.extractfile(member)
            digest = hashlib.sha256()
            if member.name in wanted:
                name = wanted[member.name]
                recipe = cases[name]
                request = Request(0, recipe.request)
                packet = request.packet()
                require(parse_packet(packet) == request and packet[12:] == recipe.request.packet(),
                        'unchanged nested predecessor packet')
                events = total = 0
                for expected in observations(request, recipe.kinds):
                    line = stream.readline(MAX_LINE + 1)
                    require(0 < len(line) <= MAX_LINE and line.endswith(b'\n'),
                            'bounded complete native row')
                    digest.update(line)
                    same(parse(line), predecessor_projection(expected),
                         'unchanged native policy-zero state after declared tags only')
                    events += expected['record'] == 'event'
                    total += 1
                require(stream.read(1) == b'', 'complete native predecessor EOF')
                peer = inventory['logs/sanitized-run-'+name+'.stdout']
                require(peer['bytes'] == member.size and peer['sha256'] == identity['sha256'],
                        'both native builds have identical complete states')
                results.append(dict(name=name, events=events, records=total,
                                    native_sha256=identity['sha256'],
                                    packet_sha256=hashlib.sha256(packet).hexdigest()))
            else:
                while part := stream.read(65536):
                    digest.update(part)
            require(digest.hexdigest() == identity['sha256'], 'exact native member hash')
    require(seen == set(inventory) and {r['name'] for r in results} == set(cases)
            and len(results) == 19, 'all nineteen native predecessors')
    return dict(archive_sha256=ARCHIVE_SHA, cases=results,
                events=sum(r['events'] for r in results),
                identical_build_transcripts=2*len(results))


def check():
    children = []
    def guard(event, args):
        if event in ('subprocess.Popen', 'os.system', 'os.posix_spawn', 'os.fork'):
            children.append(event)
            raise ValueError('child execution forbidden')
    sys.addaudithook(guard)
    native = native_predecessors()
    rejected = []
    def reject(name, call):
        require(name not in rejected, 'distinct negative control')
        try:
            call()
        except (ValueError, TypeError):
            rejected.append(name)
        else:
            raise ValueError('accepted malformed/forged control: '+name)

    original = Original(b'a'*128, (64, 64))
    for policy in (0, 2):
        request = Request(policy, original)
        packet = request.packet()
        require(parse_packet(packet) == request, 'both policies roundtrip')
        for name, data in [('empty', b''), ('header', packet[:11]),
                           ('truncated', packet[:-1]), ('trailing', packet+b'!'),
                           ('outer-magic', b'!'+packet[1:]),
                           ('inner-magic', packet[:12]+b'!'+packet[13:]),
                           ('mutable', bytearray(packet))]:
            reject(f'p{policy}-{name}', lambda d=data: parse_packet(d))
        for offset, value in [(8, 1), (8, 3), (8, 0xffffffff), (20, 32769),
                              (24, 1), (24, 5), (28, 9), (28, 17), (32, 5),
                              (32, 13), (36, 5), (36, 13), (40, 0), (40, 7),
                              (44, 2), (44, 8), (48, 4096), (52, 2),
                              (56, 63), (56, 8193)]:
            data = packet[:offset]+struct.pack('<I', value)+packet[offset+4:]
            reject(f'p{policy}-packet-{offset}-{value}', lambda d=data: parse_packet(d))
    for value in (True, False, 0.0, 2.0, -1, 1, 3, None, '2'):
        reject('policy-'+repr(value), lambda v=value: Request(v, original).validate())
    reject('wrong-nested-type', lambda: Request(2, dataclasses.asdict(original)).validate())
    for field, value in [('source', bytearray(original.source)), ('blocks', [64, 64]),
                         ('blocks', (64, True)), ('target', True), ('literal_mode', False)]:
        reject('nested-'+field+'-'+type(value).__name__,
               lambda f=field, v=value: Request(2, dataclasses.replace(original, **{f:v})).validate())
    request = Request(2, original)
    kinds = (2, 1)
    for value in ([2, 1], (2,), (2, True), (2, 3)):
        reject('kinds-'+repr(value), lambda v=value: records(request, v))
    good_rows = list(observations(request, kinds))
    good = encoded(good_rows)
    validate_output(good, request, kinds)
    event_index = next(i for i, r in enumerate(good_rows)
                       if r.get('stage') == 'query' and r['block'] == 1)
    pub_index = next(i for i, r in enumerate(good_rows)
                     if r['record'] == 'block_publish' and r['block'] == 1)
    origin_index = next(i for i, r in enumerate(good_rows) if r.get('stage') == 'origin')
    controls = {
        'metadata-policy': lambda r: r[0].update(price_policy=0),
        'event-policy': lambda r: r[event_index].update(price_policy=0),
        'publication-policy': lambda r: r[pub_index].update(price_policy=0),
        'complete-policy': lambda r: r[-1].update(price_policy=0),
        'policy-bool': lambda r: r[event_index].update(price_policy=True),
        'invented-alternative': lambda r: r[event_index].update(alternative=[0]*12),
        'query': lambda r: r[event_index]['query'].__setitem__(0, 0),
        'query-bool': lambda r: r[event_index]['query'].__setitem__(0, True),
        'frequency': lambda r: r[event_index]['frequencies']['ll'].__setitem__(0, 0),
        'search-length': lambda r: r[event_index]['search'].update(length=1),
        'search-before': lambda r: r[event_index]['search_before'].update(changes=[[0, 2]]),
        'path-length': lambda r: r[origin_index]['cells'].update(length=515),
        'path-duplicate': lambda r: r[origin_index]['cells']['changes'].extend(
            copy.deepcopy(r[origin_index]['cells']['changes'])),
        'owner': lambda r: r[pub_index].update(previous_owner_after=0),
        'candidate': lambda r: r[pub_index].update(candidate_repeats=[0, 0, 0]),
        'wire-kind': lambda r: r[pub_index].update(kind=2),
        'missing': lambda r: r.pop(event_index),
        'extra': lambda r: r.append(copy.deepcopy(r[-1])),
    }
    for name, edit in controls.items():
        changed = copy.deepcopy(good_rows)
        edit(changed)
        data = encoded(changed)
        require(data != good, 'control must actually change bytes: '+name)
        reject('trace-'+name, lambda d=data: validate_output(d, request, kinds))
    for name, data in [('newline', good[:-1]), ('trailing', good+b'{}\n'),
                       ('duplicate-key', good.replace(b'"price_policy":2',
                           b'"price_policy":2,"price_policy":2', 1)),
                       ('nonfinite', good.replace(b'"price_policy":2', b'"price_policy":NaN', 1))]:
        reject('trace-'+name, lambda d=data: validate_output(d, request, kinds))

    hypotheses = []
    coverage = set()
    for n in (128, 16385, 24576, 24577, 32768):
        blocks = (64, 64) if n == 128 else balanced_blocks(n)
        for policy in (0, 2):
            r = Original(b'a'*n, blocks, window_log=15)
            tagged = Request(policy, r)
            require(parse_packet(tagged.packet()) == tagged, 'all size boundaries roundtrip')
            rows, hit = records(tagged, (2,)+(1,)*(len(blocks)-1))
            coverage.update(hit)
            require(rows[-1]['record'] == 'block_publish' and rows[-1]['block'] == len(blocks)-1,
                    'nonzero retained block initialization')
            hypotheses.append(dict(policy=policy, source_bytes=n, records=len(rows), coverage=sorted(hit)))
        if n > 16384:
            reject('window-'+str(n), lambda r=r: Request(2, dataclasses.replace(r, window_log=14)).validate())
        reject('future-prefix-'+str(n), lambda r=r: SearchPrefix(r, r.source[:r.blocks[0]+1]).validate())
    raw_rows, raw_coverage = records(request, (0, 2))
    require(raw_rows[1]['record'] == 'event' and 'raw_rollback' in raw_coverage,
            'source-only raw rollback hypothesis')
    require({'compressed_commit', 'rle_rollback'} <= coverage and not children,
            'basic retained composition without children')
    sources = {str(Path(m.__file__).resolve().relative_to(ROOT)): sha_file(Path(m.__file__).resolve())
               for m in list(sys.modules.values()) if getattr(m, '__file__', None)
               and Path(m.__file__).resolve().is_relative_to(ROOT/'scripts')
               and Path(m.__file__).suffix == '.py'}
    return dict(status='passed', optimized=not __debug__, native_predecessors=native,
                rejected=len(rejected), controls=rejected, boundary_hypotheses=hypotheses,
                source_sha256=sources, child_processes=0,
                qualification='All nineteen retained Optimal native captures replay under additive policy 0; both padding passes and identical normal/sanitized transcripts are bound. Policy-2 boundary and publication states are independent hypotheses only. No successor native execution, production implementation or public Ultra admission is claimed.')


if __name__ == '__main__':
    print(json.dumps(check(), sort_keys=True, indent=2))
