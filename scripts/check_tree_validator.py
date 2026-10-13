#!/usr/bin/env python3
"""Qualify tree observation envelope/replay using explicitly synthetic records.

No reference executable or real C result is used or claimed by these controls.
"""
import copy
import hashlib
import json
from pathlib import Path
import sys

sys.dont_write_bytecode = True
from oracles import tree_direct as wire, tree_validation as audit


def synthetic():
    request = wire.Request(b'a' * 128, (0, 1, 2, 3, 119, 120))
    head_cells = 1 << request.hash_log
    node_cells = 1 << request.chain_log
    source_arena = 64 + len(request.source) + 2048
    dynamic = source_arena + 191 + 2 * (4 * head_cells + 191) + 2 * (4 * node_cells + 191)
    metadata = dict(record='metadata', protocol='ZTREE01', revision=wire.REVISION,
                    source_bytes=len(request.source), source_fnv1a64=wire.fnv(request.source),
                    parameters=request.metadata()['parameters'], queries=list(request.queries),
                    bias=2, offset_seed=audit.OFFSET_SEED, head_cells=head_cells,
                    node_cells=node_cells, source_arena_bytes=source_arena,
                    fixed_bytes=470000, live_bytes_cap=4 << 20)
    records = [metadata]
    # Hand-written structural examples exercise non-matches, selected bytes and
    # skipped frontiers. They are checker inputs, not frozen oracle observations.
    plans = [(0, audit.OFFSET_SEED, 1), (0, audit.OFFSET_SEED, 2),
             (126, 4, 119), (0, audit.OFFSET_SEED, 119), (9, 120, 120), (8, 4, 121)]
    for pass_index in range(2):
        heads, nodes, frontier = [0] * head_cells, [[0, 0] for _ in range(node_cells // 2)], 0
        for ordinal, (query, (length, offset, after)) in enumerate(zip(request.queries, plans)):
            before = heads[:]
            if query >= frontier:
                heads[audit.key_bucket(request.source, query, request.hash_log, request.min_match)] = query + 2
            records.append(dict(record='operation', **{'pass': pass_index}, padding=(0x6a, 0xc3)[pass_index],
                               ordinal=ordinal, query=query, before_frontier=frontier, after_frontier=after,
                               window_floor_raw=max(2, query + 2 - (1 << request.window_log)),
                               tree_floor_raw=max(0, query + 2 - ((1 << (request.chain_log - 1)) - 1)),
                               length=length, offset_base=offset, before_heads=before, after_heads=heads[:],
                               before_nodes=copy.deepcopy(nodes), after_nodes=copy.deepcopy(nodes),
                               source_unchanged=True, guards_unchanged=True))
            frontier = after
    records.append(dict(record='complete', operations=len(request.queries), passes=2,
                        allocations=10, releases=10, live_bytes=0, peak_dynamic_bytes=dynamic))
    return request, records


def encode(records):
    return b''.join(audit.canonical(record) + b'\n' for record in records)


def check():
    request, original = synthetic()
    raw = encode(original)
    positive = audit.validate(request, raw)
    wire.require(positive['selected'] == 6 and positive['skipped'] == 2, 'synthetic branch coverage')
    results = []

    def reject(name, data, target=request):
        try:
            audit.validate(target, data)
        except (ValueError, TypeError, KeyError, IndexError, RecursionError):
            results.append(name)
        else:
            raise ValueError('accepted malformed tree record: ' + name)

    def change(name, edit):
        records = copy.deepcopy(original)
        edit(records)
        data = encode(records)
        wire.require(data != raw, 'effective mutation: ' + name)
        reject(name, data)

    for field, value in [('extra', 1), ('protocol', 'ZTREE02'), ('revision', '0' * 40),
                          ('source_bytes', 128.0), ('source_fnv1a64', '0' * 16),
                          ('fixed_bytes', 459775), ('bias', 0), ('offset_seed', 0),
                          ('head_cells', 32), ('node_cells', 32), ('live_bytes_cap', 1 << 30)]:
        change('metadata-' + field, lambda r, f=field, v=value: r[0].update({f: v}))
    change('parameter-bool', lambda r: r[0]['parameters'].__setitem__(3, True))
    change('query-metadata', lambda r: r[0]['queries'].__setitem__(0, 1))
    for field, value in [('record', 'other'), ('pass', 1), ('padding', 0), ('ordinal', True),
                          ('query', 1), ('before_frontier', 1), ('after_frontier', 0),
                          ('window_floor_raw', 0), ('tree_floor_raw', 1), ('length', 129),
                          ('offset_base', 4), ('source_unchanged', 1), ('guards_unchanged', False)]:
        change('operation-' + field, lambda r, f=field, v=value: r[1].update({f: v}))
    change('truncated-heads', lambda r: r[1]['after_heads'].pop())
    change('truncated-nodes', lambda r: r[1]['after_nodes'].pop())
    change('truncated-pair', lambda r: r[1]['after_nodes'][0].pop())
    change('node-float', lambda r: r[1]['after_nodes'][0].__setitem__(0, 0.0))
    change('node-marker-first', lambda r: r[1]['after_nodes'][0].__setitem__(0, 1))
    change('node-future', lambda r: r[1]['after_nodes'][0].__setitem__(1, 3))
    change('head-marker', lambda r: r[1]['after_heads'].__setitem__(0, 1))
    change('head-wrong-bucket', lambda r: r[1]['after_heads'].__setitem__(0, 2))
    bucket = audit.key_bucket(request.source, 0, request.hash_log, request.min_match)
    change('missing-head-insertion', lambda r: r[1]['after_heads'].__setitem__(bucket, 0))
    change('head-continuity', lambda r: r[2]['before_heads'].__setitem__(bucket, 0))
    change('node-continuity', lambda r: r[2]['before_nodes'][0].__setitem__(1, 1))
    change('selected-distance', lambda r: r[3].update(offset_base=100))
    change('selected-window-floor', lambda r: r[3].update(offset_base=5))
    change('skipped-frontier', lambda r: r[4].update(after_frontier=120))
    change('skipped-node-write', lambda r: r[4]['after_nodes'][0].__setitem__(1, 1))
    change('padding-only-map', lambda r: r[7]['after_nodes'][0].__setitem__(1, 1))
    for field, value in [('operations', True), ('passes', 1), ('allocations', 9),
                          ('releases', 9), ('live_bytes', 1), ('peak_dynamic_bytes', 1)]:
        change('complete-' + field, lambda r, f=field, v=value: r[-1].update({f: v}))
    different = wire.Request(b'a' * 127 + b'b', request.queries)
    changed = copy.deepcopy(original)
    changed[0]['source_fnv1a64'] = wire.fnv(different.source)
    reject('selected-byte-replay', encode(changed), different)
    reject('missing-trailer', encode(original[:-1]))
    reject('missing-operation', encode(original[:2] + original[3:]))
    reject('extra-record', raw + b'{}\n')
    reject('missing-final-newline', raw[:-1])
    reject('duplicate-json-key', raw.replace(b'{', b'{"record":"metadata",', 1))
    reject('non-finite-json', raw.replace(b'"fixed_bytes":470000', b'"fixed_bytes":NaN', 1))
    reject('oversize-record', b' ' * (audit.MAX_LINE + 1) + b'\n')
    restored = audit.validate(request, raw)
    wire.require(restored == positive, 'restored synthetic positive')
    return dict(status='passed',positive=2,rejected=len(results),cases=results,
                synthetic_result=positive,qualification='Synthetic observation-reader controls only; no C capture or tree-selection oracle result.')


def main():
    attempts = []

    def no_children(event, _args):
        if event in ('subprocess.Popen', 'os.system', 'os.fork', 'os.posix_spawn', 'os.exec', 'os.spawn'):
            attempts.append(event)
            raise RuntimeError('child process forbidden')

    sys.addaudithook(no_children)
    report = check()
    wire.require(not attempts, 'no child attempts')
    root = Path(__file__).resolve().parents[1]
    report.update(optimized=not __debug__, child_processes=len(attempts), source={
        n:hashlib.sha256((root/n).read_bytes()).hexdigest() for n in
        ['scripts/check_tree_validator.py','scripts/oracles/tree_direct.py','scripts/oracles/tree_validation.py']})
    print(json.dumps(report, sort_keys=True))


if __name__ == '__main__':
    main()
