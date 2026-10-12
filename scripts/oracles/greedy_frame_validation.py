"""Independent ordinary Greedy observation replay; no codec/subprocess imports."""

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

KEYS = {name: set(fields) for name, fields in base.RECORD_KEYS.items()}
KEYS['begin'] |= {'branch', 'sampling'}
KEYS['zero_continue'] |= {'sampling', 'first_table', 'second_table'}
KEYS['block'] = (KEYS['block'] - {'hash'}) | {'first_table', 'second_table', 'sampling_before', 'sampling_after'}
FRESH_SALT = '835892b4bb2cae74'


def integer(value, lower, upper, label):
    need(type(value) is int and lower <= value <= upper, label)


def scalar_types(value, key=None, depth=0):
    need(depth <= 40, 'JSON nesting cap')
    if type(value) is dict:
        for name, item in value.items():
            need(type(name) is str, 'object key type')
            scalar_types(item, name, depth + 1)
    elif type(value) is list:
        need(len(value) <= 1 << 19, 'JSON array cap')
        for item in value:
            scalar_types(item, None, depth + 1)
    elif type(value) is bool:
        need(key in base.BOOL_FIELDS, 'boolean field domain')
    elif type(value) is int:
        need(key not in base.BOOL_FIELDS and 0 <= value < 1 << 64, 'integer field domain')
    elif type(value) is str:
        need(key in {'type', 'case', 'source_fnv64', 'payload_fnv64', 'before_fnv64',
                     'after_fnv64', 'frame_fnv64', 'bytes', 'literal_bytes', 'branch', 'salt'}
             and len(value) <= 1 << 20, 'string field domain')
    else:
        need(value is None and key in {'huf_table', 'candidate_state'}, 'nullable field domain')


def digest(value):
    need(type(value) is str and len(value) == 16 and all(c in '0123456789abcdef' for c in value), 'canonical hex64')


def sampling_shape(value):
    shape(value, {'next_to_update', 'skipping', 'row_hash_log', 'salt', 'salt_entropy', 'cache'}, 'sampling')
    integer(value['next_to_update'], 2, (4 << 20) + 2, 'frontier domain')
    integer(value['skipping'], 0, 1, 'skipping domain')
    integer(value['row_hash_log'], 0, 15, 'row hash log domain')
    integer(value['salt_entropy'], 0, (1 << 32) - 1, 'salt entropy domain')
    digest(value['salt'])
    need(type(value['cache']) is list and len(value['cache']) == 8, 'cache count')
    for entry in value['cache']:
        integer(entry, 0, (1 << 32) - 1, 'cache scalar domain')


def table_shape(value):
    shape(value, {'log', 'element_bytes', 'entries', 'before_fnv64', 'after_fnv64', 'changed'}, 'table')
    integer(value['log'], 6, 19, 'table log domain')
    need(value['element_bytes'] in (1, 4) and value['entries'] == 1 << value['log'], 'table element dimensions')
    digest(value['before_fnv64']); digest(value['after_fnv64'])
    need(type(value['changed']) is list and len(value['changed']) <= value['entries'], 'delta count')
    need(all(type(t) is list and len(t) == 3 for t in value['changed']), 'delta shape')


def record_shapes(records):
    need(type(records) is list and 2 <= len(records) <= 70, 'bounded records')
    scalar_types(records)
    for r in records:
        need(type(r) is dict and r.get('type') in KEYS, 'record discriminator')
        shape(r, KEYS[r['type']], r['type'])
        if r['type'] == 'begin':
            base.state_shape(r['initial_state']); sampling_shape(r['sampling'])
            need(r['branch'] in ('chain', 'row'), 'branch name')
        if r['type'] == 'zero_continue':
            sampling_shape(r['sampling'])
        if r['type'] == 'block':
            need(r['first'] in (0, 1) and r['prev_owner_before'] in (0, 1)
                 and r['prev_owner_after'] in (0, 1), 'owner/first fields')
            base.state_shape(r['before']); base.state_shape(r['after'])
            base.repeats_shape(r['semantic_before']); base.repeats_shape(r['semantic_after'])
            sampling_shape(r['sampling_before']); sampling_shape(r['sampling_after'])
            c = r['candidate']
            need(type(c) is dict and type(c.get('fresh')) is bool, 'candidate tag')
            if c['fresh']:
                shape(c, {'fresh', 'sequence_count', 'literal_count', 'sequences', 'trailing',
                          'literal_bytes', 'semantic_repeats', 'long_length_type', 'long_length_position'}, 'fresh candidate')
                base.state_shape(r['candidate_state']); base.repeats_shape(c['semantic_repeats'])
                need(type(c['sequences']) is list and len(c['sequences']) <= 131072 // 4
                     and all(type(s) is list and len(s) == 6 for s in c['sequences']), 'sequence shapes/count')
                integer(c['long_length_type'], 0, 2, 'long length type')
            else:
                shape(c, {'fresh', 'previous_generation'}, 'stale candidate')
                need(r['candidate_state'] is None, 'unavailable candidate state')
            shape(r['window'], {'base_index_bias', 'low_limit', 'dict_limit', 'next_to_update', 'consumed'}, 'window')
            need(r['window']['base_index_bias'] == 2 and r['window']['low_limit'] >= 2
                 and r['window']['dict_limit'] >= 2, 'window raw indices')
        if r['type'] in ('block', 'zero_continue'):
            table_shape(r['first_table']); table_shape(r['second_table'])


def key_hash(source, position, width, bits, salt):
    need(4 <= width <= 6 and 1 <= bits <= 23 and 0 <= position <= len(source) - width, 'key domain')
    word = int.from_bytes(source[position:position + width], 'little')
    if width == 4:
        return (((word * 2654435761) & 0xffffffff) ^ (salt & 0xffffffff)) >> (32 - bits)
    prime = {5: 889523592379, 6: 227718039650203}[width]
    return ((((word << (64 - 8 * width)) * prime) & ((1 << 64) - 1)) ^ salt) >> (64 - bits)


def table_update(observed, previous, log, width, fresh):
    need((observed['log'], observed['element_bytes'], observed['entries']) == (log, width, 1 << log), 'selected owner dimensions')
    need(len(previous) == 1 << log, 'previous table count')
    encode = lambda values: b''.join(v.to_bytes(width, 'little') for v in values)
    need(observed['before_fnv64'] == fingerprint(encode(previous)), 'table before fingerprint')
    seen = -1
    for slot, old, new in observed['changed']:
        need(seen < slot < len(previous) and 0 <= old < 1 << (8 * width)
             and 0 <= new < 1 << (8 * width) and old != new and previous[slot] == old, 'ordered table changes')
        previous[slot] = new
        seen = slot
    if not fresh:
        need(not observed['changed'], 'bypassed call changed sampled table')
    need(observed['after_fnv64'] == fingerprint(encode(previous)), 'table after fingerprint')


def sampling_domain(value, recipe, consumed):
    need(value['next_to_update'] <= consumed + 2, 'frontier consumed bound')
    row = recipe.window_log > 14
    if row:
        row_log = min(6, max(4, recipe.search_log))
        need(value['row_hash_log'] == recipe.hash_log - row_log and value['salt'] == FRESH_SALT,
             'fresh-context row geometry/salt')
        need(all(h < 1 << (value['row_hash_log'] + 8) for h in value['cache']), 'cache hash width')
    else:
        need(value['row_hash_log'] == value['salt_entropy'] == 0 and value['salt'] == '0000000000000000'
             and value['cache'] == [0] * 8, 'inactive row metadata')


def validate_tables(tables, recipe, consumed, state):
    width = min(6, max(4, recipe.min_match))
    def position(value):
        need(2 <= value < state['next_to_update'] and value - 2 + width <= consumed, 'stored key frontier/extent')
        return value - 2
    # Membership is an independent state check, not a reconstruction of C's
    # intermediate insertion schedule. Frozen artifact identities cover that.
    if recipe.window_log <= 14:
        for slot, value in enumerate(tables[0]):
            if value:
                need(key_hash(recipe.source, position(value), width, recipe.hash_log, 0) == slot, 'chain head hash membership')
        for value in tables[1]:
            if value:
                position(value)
    else:
        row_log = min(6, max(4, recipe.search_log)); row_width = 1 << row_log
        positions, tags = tables
        salt = int(state['salt'], 16)
        for start in range(0, len(positions), row_width):
            head = tags[start]
            need(positions[start] == 0 and head < row_width, 'reserved position/head bound')
            occupied = False
            for relative in range(1, row_width):
                value = positions[start + relative]
                if value:
                    hashed = key_hash(recipe.source, position(value), width, recipe.hash_log - row_log + 8, salt)
                    need((hashed >> 8) << row_log == start and hashed & 255 == tags[start + relative], 'row/tag hash membership')
                    occupied = True
                else:
                    need(tags[start + relative] == 0, 'empty position tag')
            need((head != 0) == occupied and (head == 0 or positions[start + head] != 0), 'row occupancy/head')


def resolve(repeats, off, ll):
    a, b, c = repeats
    if off == 1:
        return (a, repeats) if ll else (b, [b, a, c])
    need(off >= 4, 'Greedy offset intent')
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
    row = recipe.window_log > 14
    initial_sampling = {'next_to_update': 2, 'skipping': 0, 'row_hash_log': recipe.hash_log - min(6, max(4, recipe.search_log)) if row else 0,
                        'salt': FRESH_SALT if row else '0000000000000000', 'salt_entropy': 0, 'cache': [0] * 8}
    need(begin['branch'] == ('row' if row else 'chain') and begin['sampling'] == initial_sampling, 'initial selected sampling')
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
    tables = [[0] * (1 << recipe.hash_log), [0] * (1 << (recipe.hash_log if row else recipe.chain_log))]
    cursor, generation = 0, 0
    rollback, kinds = [], []
    previous_sampling = initial_sampling
    wire_index = 0
    owners = [(tables[0], recipe.hash_log, 4), (tables[1], recipe.hash_log if row else recipe.chain_log, 1 if row else 4)]
    for r in (r for r in records if r['type'] in ('block', 'zero_continue')):
        length = schedule[r['call']]
        if not length:
            need(r['sampling'] == previous_sampling, 'zero sampling continuity')
            for field, (table, log, width) in zip(('first_table', 'second_table'), owners):
                table_update(r[field], table, log, width, False)
            validate_tables(tables, recipe, cursor, previous_sampling)
            continue
        actual = wire_blocks[wire_index]; wire_index += 1
        need(r['sampling_before'] == previous_sampling, 'sampling continuity')
        state = r['sampling_after']
        sampling_domain(state, recipe, cursor + length)
        floor = max(2, cursor + 2 - (1 << recipe.window_log))
        entering = max(previous_sampling['next_to_update'], floor)
        if length >= 7:
            gap = cursor + 2 - entering
            if gap > 384:
                entering = cursor + 2 - min(192, gap - 384)
            need(state['next_to_update'] >= entering, 'fresh frontier after ordered wrapper adjustments')
        else:
            need(state == previous_sampling | {'next_to_update': entering}, 'tiny window-floor clamp only')
        kinds.append(r['kind'])
        need(r['source_start'] == cursor and r['source_bytes'] == length and r['kind'] == actual['kind']
             and not actual['last'] and r['payload_bytes'] == len(actual['payload'])
             and r['payload_fnv64'] == fingerprint(actual['payload']), 'source/wire block mapping')
        need(r['before'] == previous_state and r['prev_owner_before'] == previous_owner
             and r['pending_was_initialized'] is initialized[1 - previous_owner], 'context owner/state continuity')
        need(r['semantic_before'] == semantic and r['before']['raw_repeats'][:2] == semantic[:2], 'incoming repeats')
        need(r['first'] == int(cursor == 0) and (cursor != 0 or r['kind'] != 1), 'first uniform block rule')
        window = r['window']
        need(window['consumed'] == cursor + length and window['dict_limit'] == window['low_limit'] == floor
             and window['next_to_update'] == state['next_to_update'], 'ordinary block-start floor/progress')
        trial = semantic.copy()
        c = r['candidate']
        if length >= 7:
            generation += 1
            initialized[1 - previous_owner] = True
            need(c['fresh'] is True, 'fresh candidate')
            literals = bytes.fromhex(c['literal_bytes'])
            need(c['literal_count'] == len(literals) <= length and c['sequence_count'] == len(c['sequences']), 'candidate counts')
            lit = 0
            marker = None
            for index, (ll, ml, off, distance, raw_ll, raw_ml) in enumerate(c['sequences']):
                need(raw_ll == (ll & 65535) and ml >= 4 and raw_ml == ((ml - 3) & 65535), 'raw/expanded sequence lengths')
                for kind, value in [(1, ll), (2, ml - 3)]:
                    if value >= 65536:
                        need(marker is None, 'single long marker')
                        marker = (kind, index)
                need(ml >= 4 and ll <= len(literals) - lit and len(replay) + ll + ml <= cursor + length, 'expanded sequence extent')
                replay.extend(literals[lit:lit + ll]); lit += ll
                decoded_distance, trial = resolve(trial, off, ll)
                need(distance == decoded_distance and 0 < distance <= len(replay) and distance <= 1 << recipe.window_log
                     and len(replay) - distance >= window['dict_limit'] - 2, 'sequence distance/window')
                for _ in range(ml):
                    replay.append(replay[-distance])
            need((marker or (0, 0)) == (c['long_length_type'], c['long_length_position']), 'long marker identity')
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
        for field, (table, log, width) in zip(('first_table', 'second_table'), owners):
            table_update(r[field], table, log, width, length >= 7)
        validate_tables(tables, recipe, cursor + length, state)
        previous_sampling = state
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


def validate_metadata(value):
    expected = {'type': 'metadata', 'revision': base.REVISION, 'strategy': 3, 'schema_version': 2,
                'pointer_bits': value.get('pointer_bits') if type(value) is dict else None,
                'int_bits': 32, 'u32_bytes': 4, 'source_cap': 4194304, 'block_cap': 131072,
                'call_cap': 64, 'leading_bytes': 64, 'suffix_bytes': 262656,
                'guard_bytes': 64, 'scratch_bytes': 2097152}
    need(type(value) is dict and set(value) == set(expected), 'metadata shape')
    need(all(type(value[k]) is type(v) and value[k] == v for k, v in expected.items()), 'metadata dimensions/version')
    need(type(value['pointer_bits']) is int and value['pointer_bits'] in (32, 64), 'native pointer width')
