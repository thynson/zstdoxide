#!/usr/bin/env python3
"""Independent path replay, hand-derived publication and forged trace controls."""
from copy import deepcopy
import json
from oracles.optimal_paths import Request, MAX_EVENTS, require
from oracles.optimal_path_recipes import recipes
from oracles.optimal_path_validation import Replay, observations, validate_output


def encoded(rows):
    return b''.join(json.dumps(row, sort_keys=True, separators=(',', ':')).encode() + b'\n' for row in rows)


def check():
    cases = recipes()
    coverage = set()
    totals = dict(events=0, queries=0, sequences=0)
    for name, request in cases.items():
        request.validate()
        state = Replay(request)
        state.run()
        coverage |= state.coverage
        totals['events'] += len(state.events)
        totals['queries'] += sum(e['stage'] == 'query' for e in state.events)
        totals['sequences'] += len(state.sequences)
        validate_output(encoded(observations(request)), request)
        if name == 'path_no_query_minimum_source':
            require(len(state.events) == 2 and state.sequences == [] and state.repeats == (1, 4, 8), 'no-query hand trace')
        if name == 'path_immediate':
            require(state.sequences == [[0, 1, 3, 4]] and state.repeats == (1, 1, 4) and state.events[-1]['tail'] == 12, 'immediate hand trace')
        if name == 'path_all_literals':
            require(state.sequences == [] and state.events[-1]['tail'] == 64, 'all-literal hand trace')
        if name == 'path_deferred_repeat':
            require(state.sequences == [[0, 1, 8, 4]] and state.repeats == (1, 1, 4), 'deferred hand trace')
        if name == 'path_repeat_publication':
            require(state.sequences == [[0, 4, 4, 2], [8, 0, 3, 3], [11, 0, 3, 2]] and state.repeats == (1, 3, 4), 'repeat publication hand trace')
        if name == 'path_equal_sufficient':
            require('initial_sufficient' not in state.coverage, 'strict sufficient threshold')
    required = {'all_literals', 'deferred_repeat_publication', 'initial_no_match', 'initial_sufficient',
        'later_source_end', 'later_sufficient', 'literal_improvement', 'literal_rejected', 'literal_tie',
        'match_extend', 'match_improvement', 'match_tie_abort', 'match_worse_abort', 'multiple_sequences',
        'tail_no_query', 'trailing_backtrack', 'unpromising_skip'}
    require(required <= coverage, 'required branch coverage')
    request = cases['path_script_seed_0']
    original = list(observations(request))
    forged = []
    def change(index, field, value):
        rows = deepcopy(original)
        rows[index][field] = value
        forged.append(encoded(rows))
    change(0, 'revision', '0' * 40)
    change(0, 'path_cells', 4099)
    change(-1, 'events', 1)
    change(-1, 'guards_unchanged', False)
    event = next(i for i, e in enumerate(original) if e.get('stage') == 'literal')
    for field, value in [('phase', 'reverse'), ('cur', -1), ('last', 4096), ('repeats', [0, 0, 0]),
                         ('source_unchanged', False), ('path_tail_zero', False), ('index', True), ('extra', 0)]:
        change(event, field, value)
    for column in range(7):
        cells = deepcopy(original[event]['cells'])
        cells[original[event]['cur']][column] += 1
        change(event, 'cells', cells)
    for field in ('frequencies', 'sums', 'base_prices'):
        values = deepcopy(original[event][field])
        if field == 'frequencies': values['ll'][0] += 1
        else: values['ll'] += 1
        change(event, field, values)
    reverse = next(i for i, e in enumerate(original) if e.get('stage') == 'reverse')
    change(reverse, 'reverse', [0, 0, 0])
    query = next(i for i, e in enumerate(original) if e.get('stage') == 'query' and e['matches'])
    matches = deepcopy(original[query]['matches'])
    matches[0][0] += 1
    change(query, 'matches', matches)
    change(query, 'query', [0, 1, 4, 8, 0, 3])
    sequence = next(i for i, e in enumerate(original) if e.get('stage') == 'sequence')
    change(sequence, 'sequences', [])
    forged += [encoded(original[:-1]), encoded(original) + b'{}\n', encoded(original)[:-1], encoded(original[1:])]
    for data in forged:
        try: validate_output(data, request)
        except ValueError: pass
        else: raise ValueError('forged path trace accepted')
    invalid = Replay(Request(b'a' * 16, ((1, ((2, 3),)),)))
    try: invalid.run()
    except ValueError: pass
    else: raise ValueError('unavailable dynamic repeat accepted')
    state = Replay(cases['path_immediate'])
    state.events = [{}] * MAX_EVENTS
    try: state.emit('begin')
    except ValueError: pass
    else: raise ValueError('event overflow accepted')
    return dict(status='passed', cases=len(cases), coverage=sorted(coverage),
                forged_rejected=len(forged), dynamic_rejections=2, child_processes=0, **totals)


if __name__ == '__main__':
    print(json.dumps(check(), sort_keys=True))
