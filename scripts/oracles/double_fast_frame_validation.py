"""Independent ordinary DoubleFast observation replay; no codec/subprocess imports."""

from pathlib import Path
import hashlib
import importlib.util

SUPPORT = Path(__file__).with_name('fast_frame_publication_support.py')
SUPPORT_SHA256 = 'c1a205a9b7f897232dd7eb1e177f2a318c42da77ac8348226825732b44b0857a'
if hashlib.sha256(SUPPORT.read_bytes()).hexdigest() != SUPPORT_SHA256:
    raise ValueError('frozen shared frame-inspection identity')
_spec = importlib.util.spec_from_file_location('zstdoxide_frozen_frame_inspection', SUPPORT)
base = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(base)
need = base.need
shape = base.shape
fingerprint = base.fingerprint
parse_json = base.parse_json

KEYS = dict(base.RECORD_KEYS)
KEYS['block'] = (KEYS['block'] - {'hash'}) | {'long_table', 'short_table'}


def table_shape(value):
    shape(value, {'log', 'width', 'entries', 'before_fnv64', 'after_fnv64', 'changed'}, 'table')
    need(type(value['changed']) is list and len(value['changed']) <= 1 << 18, 'delta count')
    need(all(type(t) is list and len(t) == 3 for t in value['changed']), 'delta shape')


def record_shapes(records):
    need(type(records) is list and 2 <= len(records) <= 70, 'bounded records')
    base.scalar_types(records)
    for r in records:
        need(type(r) is dict and r.get('type') in KEYS, 'record discriminator')
        shape(r, KEYS[r['type']], r['type'])
        if r['type'] == 'begin':
            base.state_shape(r['initial_state'])
        if r['type'] == 'block':
            need(r['first'] in (0, 1) and r['prev_owner_before'] in (0, 1)
                 and r['prev_owner_after'] in (0, 1), 'owner/first fields')
            base.state_shape(r['before']); base.state_shape(r['after'])
            base.repeats_shape(r['semantic_before']); base.repeats_shape(r['semantic_after'])
            c = r['candidate']
            need(type(c) is dict and type(c.get('fresh')) is bool, 'candidate tag')
            if c['fresh']:
                shape(c, {'fresh', 'sequence_count', 'literal_count', 'sequences', 'trailing',
                          'literal_bytes', 'semantic_repeats'}, 'fresh candidate')
                base.state_shape(r['candidate_state']); base.repeats_shape(c['semantic_repeats'])
                need(type(c['sequences']) is list and len(c['sequences']) <= 131072 // 3 + 1
                     and all(type(s) is list and len(s) == 4 for s in c['sequences']), 'sequence shapes/count')
            else:
                shape(c, {'fresh', 'previous_generation'}, 'stale candidate')
                need(r['candidate_state'] is None, 'unavailable candidate state')
            shape(r['window'], {'base_index_bias', 'low_limit', 'dict_limit', 'next_to_update', 'consumed'}, 'window')
            need(r['window']['base_index_bias'] == 2 and r['window']['low_limit'] >= 2
                 and r['window']['dict_limit'] >= 2, 'window raw indices')
            for field in ('long_table', 'short_table'):
                table_shape(r[field])


def key_bucket(source, position, width, log):
    need(4 <= width <= 8 and 6 <= log <= 18 and 0 <= position <= len(source) - width, 'key domain')
    word = int.from_bytes(source[position:position + width], 'little')
    if width == 4:
        return ((word * 2654435761) & 0xffffffff) >> (32 - log)
    prime = {5: 889523592379, 6: 227718039650203, 7: 58295818150454627, 8: 0xcf1bbcdcb7a56463}[width]
    return (((word << (64 - 8 * width)) * prime) & ((1 << 64) - 1)) >> (64 - log)


def table_update(observed, previous, source, consumed, log, width, fresh):
    need((observed['log'], observed['width'], observed['entries']) == (log, width, 1 << log), 'independent table dimensions')
    need(len(previous) == 1 << log, 'previous table count')
    encode = lambda values: b''.join(v.to_bytes(4, 'little') for v in values)
    need(observed['before_fnv64'] == fingerprint(encode(previous)), 'table before fingerprint')
    seen = -1
    for slot, old, new in observed['changed']:
        need(seen < slot < len(previous) and 0 <= old < 1 << 32 and 2 <= new < 1 << 32
             and old != new and previous[slot] == old, 'ordered table changes')
        previous[slot] = new
        seen = slot
    if not fresh:
        need(not observed['changed'], 'tiny bypass changed sampled table')
    for slot, value in enumerate(previous):
        if value:
            need(2 <= value and value - 2 + width <= consumed, 'consumed table key extent')
            need(key_bucket(source, value - 2, width, log) == slot, 'complete table key bucket')
    need(observed['after_fnv64'] == fingerprint(encode(previous)), 'table after fingerprint')


def resolve(repeats, off, ll):
    a, b, c = repeats
    if off == 1:
        return (a, repeats) if ll else (b, [b, a, c])
    need(off >= 4, 'DoubleFast offset intent')
    distance = off - 3
    return distance, [distance, a, b]


def validate_case(recipe, records, frame, whole=None):
    recipe.metadata()  # Bounded inputs and complete explicit parameters.
    source, schedule = recipe.source, recipe.schedule
    record_shapes(records)
    expected_types = ['begin']
    for index, length in enumerate(schedule):
        if index == 0:
            expected_types.append('header')
        expected_types.append('block' if length else 'zero_continue')
    if not schedule:
        expected_types.append('header')
    expected_types.append('complete')
    if recipe.whole_compare:
        expected_types.append('whole_compare')
    need([r['type'] for r in records] == expected_types and all(r.get('case') == recipe.name for r in records), 'complete chronological record identity')
    begin, complete = records[0], next(r for r in records if r['type'] == 'complete')
    blocks = [r for r in records if r['type'] == 'block']
    zeros = [r for r in records if r['type'] == 'zero_continue']
    need([r['call'] for r in blocks] == [i for i, n in enumerate(schedule) if n]
         and [r['call'] for r in zeros] == [i for i, n in enumerate(schedule) if not n], 'call ordinals')
    initial = {'raw_repeats': [1, 4, 8], 'tags': {'huf': 0, 'll': 0, 'of': 0, 'ml': 0}, 'huf_table': None}
    need(begin['applied'] == recipe.metadata()['parameters'] and begin['source_bytes'] == len(source)
         and begin['source_fnv64'] == fingerprint(source) and begin['initial_state'] == initial
         and begin['pending_initialized'] is False, 'source/applied/initial metadata')
    need(begin['block_limit'] == min(131072, 1 << recipe.window_log, max(1, len(source))), 'effective block limit')
    header = next(r for r in records if r['type'] == 'header')
    header_bytes = base.validate_header(header, len(source), recipe.window_log)
    need(type(frame) is bytes and len(frame) <= len(source) + 3 * (len(blocks) + 1) + 22
         and frame.startswith(header_bytes), 'complete frame extent/header')
    for z in zeros:
        need(z['candidate_fresh'] is False and z['produced'] == (len(header_bytes) if z['call'] == 0 else 0), 'zero call publication')
    wire_blocks = base.frame_blocks(frame)
    need(len(wire_blocks) == len(blocks) + 1 and wire_blocks[-1] == {'kind': 0, 'field': 0, 'last': 1, 'payload': b''}, 'scheduled final empty block')
    semantic = [1, 4, 8]
    previous_state, previous_owner, initialized = initial, 0, [True, False]
    replay = bytearray()
    tables = [[0] * (1 << recipe.long_log), [0] * (1 << recipe.short_log)]
    cursor, generation = 0, 0
    rollback, kinds = [], []
    for r, actual in zip(blocks, wire_blocks):
        length = schedule[r['call']]
        kinds.append(r['kind'])
        need(r['source_start'] == cursor and r['source_bytes'] == length and r['kind'] == actual['kind']
             and not actual['last'] and r['payload_bytes'] == len(actual['payload'])
             and r['payload_fnv64'] == fingerprint(actual['payload']), 'source/wire block mapping')
        need(r['before'] == previous_state and r['prev_owner_before'] == previous_owner
             and r['pending_was_initialized'] is initialized[1 - previous_owner], 'context owner/state continuity')
        need(r['semantic_before'] == semantic and r['before']['raw_repeats'][:2] == semantic[:2], 'incoming repeats')
        need(r['first'] == int(cursor == 0) and (cursor != 0 or r['kind'] != 1), 'first uniform block rule')
        window = r['window']
        need(window['consumed'] == cursor + length and 2 <= window['dict_limit'] <= cursor + 2
             and 2 <= window['low_limit'] <= cursor + 2 and window['next_to_update'] <= cursor + length + 2, 'window progress bounds')
        trial = semantic.copy()
        c = r['candidate']
        if length >= 7:
            generation += 1
            initialized[1 - previous_owner] = True
            need(c['fresh'] is True, 'fresh candidate')
            literals = bytes.fromhex(c['literal_bytes'])
            need(c['literal_count'] == len(literals) <= length and c['sequence_count'] == len(c['sequences']), 'candidate counts')
            lit = 0
            for ll, ml, off, distance in c['sequences']:
                need(ml >= 4 and ll <= len(literals) - lit and len(replay) + ll + ml <= cursor + length, 'expanded sequence extent')
                replay.extend(literals[lit:lit + ll]); lit += ll
                decoded_distance, trial = resolve(trial, off, ll)
                need(distance == decoded_distance and 0 < distance <= len(replay) and distance <= 1 << recipe.window_log
                     and len(replay) - distance >= window['dict_limit'] - 2, 'sequence distance/window')
                for _ in range(ml):
                    replay.append(replay[-distance])
            need(len(literals) - lit == c['trailing'], 'trailing literal count')
            replay.extend(literals[lit:])
            need(c['semantic_repeats'] == trial and r['candidate_state']['raw_repeats'][:2] == trial[:2], 'candidate repeat replay')
        else:
            need(c == {'fresh': False, 'previous_generation': generation} and r['candidate_state'] is None
                 and r['kind'] == 0, 'stale tiny candidate')
            replay.extend(source[cursor:cursor + length])
        need(len(replay) == cursor + length and replay[cursor:] == source[cursor:cursor + length], 'complete candidate byte replay')
        need((r['prev_owner_before'] != r['prev_owner_after']) == (r['kind'] == 2), 'compressed-only owner swap')
        need(r['changed_candidate_first_two'] is (length >= 7 and trial[:2] != semantic[:2]), 'discarded repeat flag')
        if r['kind'] == 2:
            semantic = trial
            need(r['after'] == r['candidate_state'], 'published candidate state')
        else:
            before, after = r['before'], r['after']
            need(before['raw_repeats'] == after['raw_repeats'] and before['huf_table'] == after['huf_table'], 'discarded repeats/HUF')
            for tag in ('huf', 'll', 'ml'):
                need(before['tags'][tag] == after['tags'][tag], 'discarded entropy tags')
            need(after['tags']['of'] == before['tags']['of'] or (before['tags']['of'] == 2 and after['tags']['of'] == 1), 'offset tag demotion')
            if r['changed_candidate_first_two']:
                rollback.append({'call': r['call'], 'kind': r['kind']})
        need(r['semantic_after'] == semantic and r['after']['raw_repeats'][:2] == semantic[:2], 'committed repeat state')
        if r['kind'] == 0:
            need(actual['field'] == length and actual['payload'] == source[cursor:cursor + length], 'raw block bytes')
        elif r['kind'] == 1:
            need(actual['field'] == length and actual['payload'] * length == source[cursor:cursor + length], 'RLE block bytes')
        for observed, table, log, width in [(r['long_table'], tables[0], recipe.long_log, 8),
                                            (r['short_table'], tables[1], recipe.short_log, max(4, recipe.min_match))]:
            table_update(observed, table, source, cursor + length, log, width, length >= 7)
        previous_owner, previous_state = r['prev_owner_after'], r['after']
        cursor += length
    need(cursor == len(source) and replay == source, 'exhausted full replay')
    need(complete == {'type': 'complete', 'case': recipe.name, 'calls': len(schedule), 'blocks': len(blocks),
                     'candidate_generations': generation, 'decoded_bytes': len(source), 'frame_bytes': len(frame),
                     'frame_fnv64': fingerprint(frame), 'terminal_empty_raw': True}, 'complete record')
    if recipe.whole_compare:
        need(type(whole) is bytes and len(whole) <= len(source) + 3 * len(blocks) + 22, 'whole frame bound')
        whole_blocks = base.frame_blocks(whole)
        need(len(whole_blocks) == len(blocks) and base.header_info(whole) == base.header_info(frame), 'whole block/header count')
        for a, b in zip(wire_blocks, whole_blocks):
            need({k: v for k, v in a.items() if k != 'last'} == {k: v for k, v in b.items() if k != 'last'}, 'whole block payload')
        need(records[-1] == {'type': 'whole_compare', 'case': recipe.name, 'blocks': len(blocks),
                            'decoded_bytes': len(source), 'frame_bytes': len(whole), 'frame_fnv64': fingerprint(whole)}, 'whole observation')
    else:
        need(whole is None, 'no unspecified whole comparison')
    return {'id': recipe.name, 'block_kinds': kinds, 'rollback_witnesses': rollback,
            'candidate_generations': generation, 'source_bytes': len(source)}


def validate_partial(records, pledge, window):
    record_shapes(records)
    need([r['type'] for r in records] == ['header', 'partial'], 'partial order')
    base.validate_header(records[0], pledge, window)
    need(records[0]['case'] == 'header_only' and records[1]['completed_frame'] is False
         and records[1]['decoded'] is False and records[1]['pointer_bits'] in (32, 64)
         and records[1]['int_bits'] == 32, 'incomplete/undecoded header qualification')
