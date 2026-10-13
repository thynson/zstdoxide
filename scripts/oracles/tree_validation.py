"""Independent bounded structural replay of tree helper observations.

This is not a tree-selection implementation. Captured complete maps remain the
expectations; source replay and state continuity constrain their interpretation.
"""
import io
import json

from . import tree_direct as wire

MAX_OUTPUT = 128 << 20
MAX_LINE = 256 << 10
OFFSET_SEED = 999999999
require, integer = wire.require, wire.integer


def unique(pairs):
    result = {}
    for name, value in pairs:
        require(name not in result, 'duplicate JSON key')
        result[name] = value
    return result


def bad_constant(value):
    raise ValueError('non-finite JSON value: ' + value)


def parse(data):
    return json.loads(data, object_pairs_hook=unique, parse_constant=bad_constant)


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), allow_nan=False).encode()


def same(actual, expected, reason):
    require(canonical(actual) == canonical(expected), reason)


def keys(value, expected, reason):
    require(type(value) is dict and set(value) == set(expected.split()), reason)


def key_bucket(source, position, bits, minimum):
    """Pinned v1.5.7 hash4/5/6 formulas; no tree traversal or match selection."""
    width = min(6, max(4, minimum))
    require(0 <= position <= len(source) - 8, 'stored hash position footprint')
    word = int.from_bytes(source[position:position + width], 'little')
    if width == 4:
        return ((word * 2654435761) & 0xffffffff) >> (32 - bits)
    shift, prime = (24, 889523592379) if width == 5 else (16, 227718039650203)
    return (((word << shift) * prime) & 0xffffffffffffffff) >> (64 - bits)


def maps(request, heads, nodes, maximum):
    require(type(heads) is list and len(heads) == 1 << request.hash_log, 'complete head map')
    require(type(nodes) is list and len(nodes) == 1 << (request.chain_log - 1), 'complete paired node map')
    for bucket, value in enumerate(heads):
        integer(value, 0, maximum, 'raw head integer/domain')
        require(value != 1, 'no unsorted head marker')
        if value:
            require(key_bucket(request.source, value - 2, request.hash_log, request.min_match) == bucket,
                    'head source hash identity')
    for pair in nodes:
        require(type(pair) is list and len(pair) == 2, 'paired node extent')
        for side, value in enumerate(pair):
            integer(value, 0, maximum, 'raw edge integer/domain')
            require(value != 1 or side == 1, 'unsorted marker belongs to second cell')


def validate(request, data):
    request.packet()
    require(type(data) is bytes and 0 < len(data) <= MAX_OUTPUT, 'bounded complete stdout bytes')
    require(data.endswith(b'\n'), 'complete final newline')
    stream = io.BytesIO(data)

    def record():
        line = stream.readline(MAX_LINE + 1)
        require(0 < len(line) <= MAX_LINE and line.endswith(b'\n'), 'bounded complete record')
        return parse(line)

    metadata = record()
    keys(metadata, 'record protocol revision source_bytes source_fnv1a64 parameters queries bias '
         'offset_seed head_cells node_cells source_arena_bytes fixed_bytes live_bytes_cap', 'metadata schema')
    fixed = integer(metadata['fixed_bytes'], 459776, 525312, 'bounded native fixed-storage ledger')
    expected = dict(record='metadata', protocol='ZTREE01', revision=wire.REVISION,
                    source_bytes=len(request.source), source_fnv1a64=wire.fnv(request.source),
                    parameters=request.metadata()['parameters'], queries=list(request.queries),
                    bias=2, offset_seed=OFFSET_SEED, head_cells=1 << request.hash_log,
                    node_cells=1 << request.chain_log, source_arena_bytes=64 + len(request.source) + 2048,
                    fixed_bytes=fixed, live_bytes_cap=4 << 20)
    same(metadata, expected, 'request-bound metadata')
    dynamic = expected['source_arena_bytes'] + 191
    dynamic += 2 * ((4 << request.hash_log) + 191) + 2 * ((4 << request.chain_log) + 191)
    require(fixed + dynamic <= expected['live_bytes_cap'], 'complete live allocation bound')
    fingerprints = []
    counts = dict(selected=0, skipped=0, zero=0)
    for pass_index in range(2):
        heads = [0] * expected['head_cells']
        nodes = [[0, 0] for _ in range(expected['node_cells'] // 2)]
        frontier = 0
        for ordinal, query in enumerate(request.queries):
            value = record()
            keys(value, 'record pass padding ordinal query before_frontier after_frontier window_floor_raw '
                 'tree_floor_raw length offset_base before_heads after_heads before_nodes after_nodes '
                 'source_unchanged guards_unchanged', 'operation schema')
            same([value[k] for k in ('record', 'pass', 'padding', 'ordinal', 'query')],
                 ['operation', pass_index, (0x6a, 0xc3)[pass_index], ordinal, query], 'operation identity/order')
            before = integer(value['before_frontier'], 0, len(request.source), 'before frontier')
            after = integer(value['after_frontier'], 0, len(request.source), 'after frontier')
            require(before == frontier, 'frontier continuity')
            same(value['before_heads'], heads, 'complete head continuity')
            same(value['before_nodes'], nodes, 'complete node continuity')
            current = query + 2
            window = max(2, current - (1 << request.window_log))
            tree_floor = max(0, current - ((1 << (request.chain_log - 1)) - 1))
            same([value['window_floor_raw'], value['tree_floor_raw']], [window, tree_floor], 'independent raw floors')
            maps(request, value['before_heads'], value['before_nodes'], current)
            maps(request, value['after_heads'], value['after_nodes'], current)
            length = integer(value['length'], 0, len(request.source) - query, 'selected extent')
            offset = integer(value['offset_base'], 4, OFFSET_SEED, 'offset base/seed domain')
            require(value['source_unchanged'] is True and value['guards_unchanged'] is True,
                    'exact guard/source observations')
            if length == 0:
                require(offset == OFFSET_SEED, 'zero result retains seed')
                counts['zero'] += 1
            else:
                distance = offset - 3
                previous = query - distance
                require(0 <= previous < query and previous + 2 > window, 'selected source/window domain')
                require(request.source[previous:previous + length] == request.source[query:query + length],
                        'complete selected byte replay')
                counts['selected'] += 1
            if query < before:
                require(length == 0 and after == before, 'skipped result/frontier')
                same(value['after_heads'], heads, 'skipped head map')
                same(value['after_nodes'], nodes, 'skipped node map')
                counts['skipped'] += 1
            else:
                require(after >= query + 1, 'non-skipped frontier progress')
                bucket = key_bucket(request.source, query, request.hash_log, request.min_match)
                require(value['after_heads'][bucket] == current, 'searched position becomes hash head')
            fingerprint = wire.sha(canonical({k: v for k, v in value.items() if k not in ('pass', 'padding')}))
            if pass_index == 0:
                fingerprints.append(fingerprint)
            else:
                require(fingerprint == fingerprints[ordinal], 'exact padding-independent observation')
            heads, nodes, frontier = value['after_heads'], value['after_nodes'], after
    complete = record()
    same(complete, dict(record='complete', operations=len(request.queries), passes=2,
                        allocations=10, releases=10, live_bytes=0, peak_dynamic_bytes=dynamic),
         'complete exact-owner trailer')
    require(stream.read(1) == b'', 'exact record EOF')
    return dict(operations=len(request.queries), records=2 * len(request.queries) + 2,
                stdout_bytes=len(data), stdout_sha256=wire.sha(data), fixed_bytes=fixed,
                peak_dynamic_bytes=dynamic, **counts)
