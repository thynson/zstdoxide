"""Bounded ordinary lazy-family replay; no codec or child-process execution."""
from pathlib import Path
import hashlib

ROOT = Path(__file__).resolve().parents[2]
COMMON = ROOT / 'scripts/oracles/greedy_frame_validation.py'
COMMON_SHA = '8e400ccad307b562dd6a41ab02c5d00359d36431cd38a5c51040e08e3903c71f'
if not COMMON.is_file() or COMMON.is_symlink() or COMMON.stat().st_size > 64 << 10:
    raise ValueError('bounded regular common replay')
if hashlib.sha256(COMMON.read_bytes()).hexdigest() != COMMON_SHA:
    raise ValueError('frozen common replay identity')
from . import greedy_frame_validation as common

need, shape = common.need, common.shape
parse_json, fingerprint = common.parse_json, common.fingerprint
PROTOCOL = 'lazy-family-ordinary-v1'


def validate_metadata(value):
    expected = {'type':'metadata', 'revision':common.base.REVISION,
                'protocol':PROTOCOL, 'schema_version':1, 'depths':[0,1,2],
                'strategies':[3,4,5], 'pointer_bits':64, 'int_bits':32,
                'u32_bytes':4, 'source_cap':4<<20, 'block_cap':131072,
                'call_cap':64, 'leading_bytes':64, 'suffix_bytes':262656,
                'guard_bytes':64, 'scratch_bytes':2<<20}
    # JSON equality alone treats booleans as integers; canonical serialization
    # retains the scalar distinction in every nested field.
    import json
    need(type(value) is dict and json.dumps(value, sort_keys=True, allow_nan=False)
         == json.dumps(expected, sort_keys=True, allow_nan=False), 'complete lazy metadata')


def validate_case(recipe, records, frame, whole=None):
    metadata = recipe.metadata()
    need(type(metadata.get('depth')) is int and 0 <= metadata['depth'] <= 2,
         'explicit recipe depth')
    need(metadata['parameters'][-1] == 3 + metadata['depth']
         and recipe.name.startswith(f"d{metadata['depth']}_"), 'depth strategy/name binding')
    # Common replay already compares applied parameters to recipe.metadata().
    # Pass the actual strategy through unchanged; no Greedy relabeling occurs.
    result = common.validate_case(recipe, records, frame, whole)
    return result | {'depth':metadata['depth']}


def validate_partial(records, pledge, window, depth):
    need(type(depth) is int and 0 <= depth <= 2, 'partial depth domain')
    common.validate_partial(records, pledge, window)


def predecessor_records(records, old_name):
    """Only the documented d0_ case prefix changes in exact predecessor checks."""
    import copy
    result = copy.deepcopy(records)
    for record in result:
        need(record.get('case') == 'd0_' + old_name, 'predecessor case prefix')
        record['case'] = old_name
    return result
