"""Broader lazy-family source hypotheses; no parser constructs expected results."""
from dataclasses import fields

from . import greedy_direct, greedy_families, lazy_tiny
from .lazy_direct import Recipe, require

PREFIX = 4096
BODY = bytes(range(32, 128))


def constructed(branch, name, explicit, repeat=None):
    """Place disjoint known byte prefixes at source coordinates, not parser hits."""
    prefix = bytearray([241] * PREFIX)
    positions = []
    occupied = set()
    for is_explicit, (query, distance, length) in (
            [(True, v) for v in explicit] + ([(False, repeat)] if repeat else [])):
        position = PREFIX + query - distance
        region = set(range(position - 1, position + length + 1))
        require(position > 0 and position + length < PREFIX and not occupied & region,
                'disjoint constructed prefix copies')
        occupied |= region
        prefix[position - 1] = 243
        prefix[position:position + length] = BODY[query:query + length]
        prefix[position + length] = 242
        if is_explicit:
            positions.append(position)
    repeats = ((repeat[1], *greedy_families.UNAVAILABLE[1:]) if repeat
               else greedy_families.UNAVAILABLE)
    return Recipe(('row-' if branch else 'chain-') + name, bytes(prefix) + BODY,
                  branch=branch, prefix=PREFIX, next=PREFIX, window_log=13,
                  hash_log=12, chain_log=12, search_log=4, repeats=repeats,
                  seeds=tuple(sorted(positions)), salt=greedy_families.FRESH_SALT if branch else 0)


def threshold_hypotheses(branch):
    result = []
    # OffBase = distance + 3. Chosen exact powers of two make highbit costs
    # closed-form. Scores describe placed byte copies, never observed matches.
    for depth, old_bits, new_bits, lengths, penalty in (
            (1, 11, 7, (7, 8, 9), 4), (2, 11, 8, (8, 9, 10), 7)):
        for length, relation in zip(lengths, ('below', 'equal', 'above'), strict=True):
            old_distance, new_distance = (1 << old_bits) - 3, (1 << new_bits) - 3
            case = constructed(branch, f'explicit-depth-{depth}-{relation}',
                               [(0, old_distance, 8), (depth, new_distance, length)])
            result.append((case, {'purpose': f'explicit-depth-{depth}-threshold',
                'query_delta': depth, 'baseline_length': 8, 'baseline_offbase': 1 << old_bits,
                'candidate_length': length, 'candidate_offbase': 1 << new_bits,
                'baseline_score': 32 - old_bits + penalty,
                'candidate_score': 4 * length - new_bits, 'arithmetic_relation': relation}))
    for depth, old_bits, lengths, multiplier in (
            (1, 10, (4, 5, 6), 3), (2, 9, (5, 6, 7), 4)):
        for length, relation in zip(lengths, ('below', 'equal', 'above'), strict=True):
            case = constructed(branch, f'repeat-depth-{depth}-{relation}',
                               [(0, (1 << old_bits) - 3, 8)], repeat=(depth, 253, length))
            result.append((case, {'purpose': f'repeat-depth-{depth}-threshold',
                'query_delta': depth, 'baseline_length': 8, 'baseline_offbase': 1 << old_bits,
                'candidate_length': length, 'candidate_offbase': 1,
                'baseline_score': multiplier * 8 - old_bits + 1,
                'candidate_score': multiplier * length, 'arithmetic_relation': relation}))
    for length, relation in zip((7, 8, 9), ('shorter', 'equal', 'longer'), strict=True):
        case = constructed(branch, 'baseline-repeat-' + relation,
                           [(0, 2045, 8)], repeat=(1, 253, length))
        result.append((case, {'purpose': 'baseline-repeat-and-table-lengths',
            'table_length': 8, 'repeat_length': length, 'length_relation': relation}))
    case = constructed(branch, 'successive-explicit-improvements',
                       [(0, 2045, 4), (1, 1021, 8), (2, 509, 12), (3, 253, 16)])
    result.append((case, {'purpose': 'successive-explicit-improvements',
        'query_deltas': [0, 1, 2, 3], 'placed_lengths': [4, 8, 12, 16],
        'placed_offbases': [2048, 1024, 512, 256]}))
    require(len(result) == 16, 'constructed score families')
    return result


def extended(case, depth):
    values = {f.name: getattr(case, f.name) for f in fields(greedy_direct.Recipe)}
    values['name'] = f'd{depth}-' + case.name
    return Recipe(**values, depth=depth)


def recipes():
    result = lazy_tiny.recipes()
    historical = greedy_families.recipes()
    constructed_cases = [case for branch in (0, 1) for case, _ in threshold_hypotheses(branch)]
    for depth in range(3):
        result.extend(extended(c, depth) for c in historical + constructed_cases)
    require(len(result) == len({c.name for c in result}) == 645, 'broader lazy inventory')
    for case in result:
        case.packet()
    return result


def score_hypotheses():
    return [{'id': f'd{depth}-' + c.name, 'depth': depth, 'branch': branch, **hypothesis}
            for depth in range(3) for branch in (0, 1)
            for c, hypothesis in threshold_hypotheses(branch)]


def predecessor_controls():
    return lazy_tiny.predecessor_controls() + [
        {'id': 'd0-' + c.name, 'predecessor_id': c.name} for c in greedy_families.recipes()]
