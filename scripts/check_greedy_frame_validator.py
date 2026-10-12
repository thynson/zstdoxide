#!/usr/bin/env python3
"""Test offline Greedy record checks using frozen hashes and synthetic records.

Synthetic records test structural replay, not Greedy parsing or observed C
publication. They are never stored as oracle expectations. No codec executes.
"""

from pathlib import Path
import argparse
import copy
import hashlib
import json

import check_greedy_frame_recipes as inputs
from oracles import greedy_frame_validation as v

ROOT = Path(__file__).resolve().parents[1]


def synthetic(row, length):
    module = inputs.load_generator()
    source = b'x' if length == 1 else (b'abcdefg' if length == 7 else b'abcd' * 8)
    recipe = module.Recipe(('row' if row else 'chain') + '_synthetic', source, (0, length, 0),
                           'Synthetic structural record only; not observed Greedy execution.',
                           window_log=15 if row else 14, hash_log=6, chain_log=6)
    initial = {'raw_repeats': [1, 4, 8], 'tags': {'huf': 0, 'll': 0, 'of': 0, 'ml': 0}, 'huf_table': None}
    sampling = {'next_to_update': 2, 'skipping': 0, 'row_hash_log': 2 if row else 0,
                'salt': v.FRESH_SALT if row else '0000000000000000', 'salt_entropy': 0, 'cache': [0] * 8}
    def table(width):
        digest = v.fingerprint(bytes(64 * width))
        return {'log': 6, 'element_bytes': width, 'entries': 64, 'before_fnv64': digest,
                'after_fnv64': digest, 'changed': []}
    first, second = table(4), table(1 if row else 4)
    header = bytes.fromhex('28b52ffd20') + bytes([length])
    frame = header + (length << 3).to_bytes(3, 'little') + source + bytes([1, 0, 0])
    candidate_state = None
    candidate = {'fresh': False, 'previous_generation': 0}
    if length >= 7:
        candidate_state = copy.deepcopy(initial)
        if length == 32:
            candidate_state['raw_repeats'] = [4, 1, 8]
            sequences, literals, trailing, semantic = [[4, 28, 7, 4, 4, 25]], b'abcd', 0, [4, 1, 4]
        else:
            sequences, literals, trailing, semantic = [], source, length, [1, 4, 8]
        candidate = {'fresh': True, 'sequence_count': len(sequences), 'literal_count': len(literals),
                     'sequences': sequences, 'trailing': trailing, 'literal_bytes': literals.hex(),
                     'semantic_repeats': semantic, 'long_length_type': 0, 'long_length_position': 0}
    block = {'type': 'block', 'case': recipe.name, 'call': 1, 'source_start': 0, 'source_bytes': length,
             'kind': 0, 'payload_bytes': length, 'payload_fnv64': v.fingerprint(source), 'first': 1,
             'prev_owner_before': 0, 'prev_owner_after': 0, 'pending_was_initialized': False,
             'before': copy.deepcopy(initial), 'semantic_before': [1, 4, 8], 'candidate': candidate,
             'candidate_state': candidate_state, 'after': copy.deepcopy(initial), 'semantic_after': [1, 4, 8],
             'changed_candidate_first_two': length == 32,
             'window': {'base_index_bias': 2, 'low_limit': 2, 'dict_limit': 2, 'next_to_update': 2, 'consumed': length},
             'first_table': copy.deepcopy(first), 'second_table': copy.deepcopy(second),
             'sampling_before': copy.deepcopy(sampling), 'sampling_after': copy.deepcopy(sampling)}
    records = [
        {'type': 'begin', 'case': recipe.name, 'source_bytes': length, 'source_fnv64': v.fingerprint(source),
         'applied': recipe.metadata()['parameters'], 'block_limit': length, 'pending_initialized': False,
         'initial_state': copy.deepcopy(initial), 'branch': 'row' if row else 'chain', 'sampling': copy.deepcopy(sampling)},
        {'type': 'header', 'case': recipe.name, 'pledge': length, 'window_log': recipe.window_log, 'window': length,
         'single_segment': 1, 'fcs_bytes': 1, 'bytes': header.hex()},
        {'type': 'zero_continue', 'case': recipe.name, 'call': 0, 'produced': len(header), 'candidate_fresh': False,
         'sampling': copy.deepcopy(sampling), 'first_table': copy.deepcopy(first), 'second_table': copy.deepcopy(second)},
        block,
        {'type': 'zero_continue', 'case': recipe.name, 'call': 2, 'produced': 0, 'candidate_fresh': False,
         'sampling': copy.deepcopy(sampling), 'first_table': copy.deepcopy(first), 'second_table': copy.deepcopy(second)},
        {'type': 'complete', 'case': recipe.name, 'calls': 3, 'blocks': 1, 'candidate_generations': int(length >= 7),
         'decoded_bytes': length, 'frame_bytes': len(frame), 'frame_fnv64': v.fingerprint(frame), 'terminal_empty_raw': True},
    ]
    return recipe, records, frame


def check():
    path = ROOT / 'tests/upstream/match-search/vectors.tsv'
    v.need(hashlib.sha256(path.read_bytes()).hexdigest() == '628ff9df0d3b6c972233c43e5ffbc4aa11a1445908054c5d30da47b3afd078ce', 'frozen C vector identity')
    hashes = 0
    for line in path.read_text().splitlines()[1:]:
        width, bits, _, data, expected = line.split('\t')
        width, bits, expected = int(width), int(bits), int(expected)
        if width > 6:
            continue
        for salt in [0, int(v.FRESH_SALT, 16), (1 << 64) - 1]:
            high = ((salt & 0xffffffff) >> (32 - bits)) if width == 4 else salt >> (64 - bits)
            v.need(v.key_hash(bytes.fromhex(data), 0, width, bits, salt) == expected ^ high, 'C vector and algebraic salted high bits')
            hashes += 1
    v.need(hashes == 5184, 'fixed primitive control count')
    for row in [False, True]:
        for length in [1, 7, 32]:
            v.validate_case(*synthetic(row, length))
    rejected = 0
    def reject(action):
        nonlocal rejected
        try:
            action()
        except ValueError:
            rejected += 1
        else:
            raise ValueError('forged control accepted')
    for row in [False, True]:
        recipe, records, frame = synthetic(row, 32)
        changes = [
            ([0, 'branch'], 'chain' if row else 'row'), ([0, 'applied', 6], 2),
            ([0, 'source_bytes'], True), ([0, 'sampling', 'next_to_update'], 3),
            ([2, 'sampling', 'skipping'], 1), ([2, 'produced'], 0),
            ([3, 'sampling_before', 'next_to_update'], 3), ([3, 'sampling_after', 'skipping'], 2),
            ([3, 'sampling_after', 'salt_entropy'], 1 << 32), ([3, 'sampling_after', 'cache', 0], 1 << 32),
            ([3, 'window', 'dict_limit'], 3), ([3, 'window', 'next_to_update'], 3),
            ([3, 'candidate', 'sequences', 0, 0], True), ([3, 'candidate', 'sequences', 0, 1], 131072),
            ([3, 'candidate', 'sequences', 0, 2], 2), ([3, 'candidate', 'sequences', 0, 3], 5),
            ([3, 'candidate', 'sequences', 0, 4], 5), ([3, 'candidate', 'sequences', 0, 5], 24),
            ([3, 'candidate', 'long_length_type'], 2), ([3, 'candidate', 'long_length_position'], 1),
            ([3, 'candidate', 'literal_bytes'], '01020304'), ([3, 'candidate', 'trailing'], 1),
            ([3, 'prev_owner_after'], 1), ([3, 'after', 'raw_repeats', 1], 1),
            ([3, 'changed_candidate_first_two'], False), ([3, 'first_table', 'log'], 20),
            ([3, 'second_table', 'element_bytes'], 4 if row else 1),
            ([3, 'first_table', 'changed'], [[0, 1, 2]]), ([3, 'first_table', 'after_fnv64'], '0' * 16),
            ([4, 'sampling', 'next_to_update'], 3), ([5, 'candidate_generations'], 0),
        ]
        for keys, value in changes:
            forged = copy.deepcopy(records); target = forged
            for key in keys[:-1]: target = target[key]
            target[keys[-1]] = value
            reject(lambda forged=forged: v.validate_case(recipe, forged, frame))
        for length in [0, 5, len(frame) - 1]:
            reject(lambda length=length: v.validate_case(recipe, records, frame[:length]))
        reject(lambda: v.validate_case(recipe, records, frame + b'x'))
        tiny_recipe, tiny_records, tiny_frame = synthetic(row, 1)
        tiny_records[3]['sampling_after']['next_to_update'] = 3
        reject(lambda: v.validate_case(tiny_recipe, tiny_records, tiny_frame))
        forged = copy.deepcopy(records)
        values = [0] * 64; values[0] = 2 if row else 1
        forged[3]['first_table']['changed'] = [[0, 0, values[0]]]
        forged[3]['first_table']['after_fnv64'] = v.fingerprint(b''.join(x.to_bytes(4, 'little') for x in values))
        reject(lambda: v.validate_case(recipe, forged, frame))
    metadata = {'type': 'metadata', 'revision': v.base.REVISION, 'strategy': 3, 'schema_version': 2,
                'pointer_bits': 64, 'int_bits': 32, 'u32_bytes': 4, 'source_cap': 4194304, 'block_cap': 131072,
                'call_cap': 64, 'leading_bytes': 64, 'suffix_bytes': 262656, 'guard_bytes': 64, 'scratch_bytes': 2097152}
    v.validate_metadata(metadata)
    for key, value in [('schema_version', 1), ('strategy', 2), ('pointer_bits', True), ('source_cap', 1 << 30)]:
        reject(lambda key=key, value=value: v.validate_metadata(metadata | {key: value}))
    for data in [b'{"x":1,"x":2}', b'{"x":NaN}', b'{} trailing']:
        reject(lambda data=data: v.parse_json(data))
    return {'status': 'passed', 'frozen_hash_checks': hashes, 'synthetic_case_positives': 6,
            'metadata_positives': 1, 'rejected_controls': rejected, 'codec_executions': 0,
            'qualification': 'Synthetic structural replay and primitive checks only; no observed Greedy frame or intermediate sampling claim.'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true', required=True)
    parser.parse_args()
    print(json.dumps(check(), sort_keys=True))


if __name__ == '__main__':
    main()
