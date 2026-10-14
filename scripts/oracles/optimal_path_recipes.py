"""Frozen source-only synthetic callback recipes, before upstream path capture."""
import json
from pathlib import Path
from .optimal_paths import Request


def recipes():
    result = {
        'path_no_query_minimum_source': Request(bytes(range(9)), ()),
        'path_no_match_maximum_source': Request(bytes(range(256)) * 2, ()),
        'path_immediate': Request(b'a' * 16, ((1, ((4, 3),)),), target=0),
        'path_all_literals': Request(b'a' * 64, ((1, ((4, 8),)),), target=64),
        'path_deferred_repeat': Request(b'a' * 64, ((1, ((4, 8),)),), target=64, literal_mode=1),
        'path_repeat_publication': Request(b'a' * 32, ((4, ((2, 4),)), (8, ((3, 3),)), (11, ((2, 3),))), target=0),
        'path_equal_sufficient': Request(b'a' * 32, ((1, ((4, 8),)),), target=8, literal_mode=1),
    }
    data = json.loads(Path(__file__).with_name('optimal_path_script_inputs.json').read_text())
    for case in data['cases']:
        entries = tuple((p, tuple(tuple(pair) for pair in pairs)) for p, pairs in case['scripts'])
        result[case['name']] = Request(b'a' * case['source_bytes'], entries,
            target=case['target'], literal_mode=case['literal_mode'])
    for minimum in (4, 5, 6, 7):
        result['path_minimum_' + str(minimum)] = Request(b'a' * 32, ((1, ((4, 4), (4, 12))),), min_match=minimum, target=12, literal_mode=1)
    return result
