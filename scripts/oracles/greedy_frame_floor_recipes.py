#!/usr/bin/env python3
"""Additive source-only tiny-window-floor hypotheses; never execute a codec."""

from pathlib import Path
import hashlib
import importlib.util
import sys

BASE = Path(__file__).with_name('greedy_frames.py')
BASE_SHA256 = '360bf5139b7e2a886e9d2359c1ea258317238bd1906cfe9d123be33df7a0ce44'
if hashlib.sha256(BASE.read_bytes()).hexdigest() != BASE_SHA256:
    raise ValueError('frozen ordinary recipe foundation')
_spec = importlib.util.spec_from_file_location('zstdoxide_greedy_frame_foundation', BASE)
base = importlib.util.module_from_spec(_spec)
sys.modules[_spec.name] = base
_spec.loader.exec_module(base)


def recipes():
    result = []
    tail = base.noise(0xabcdef01, 33)
    for label, window in [('chain', 10), ('row', 15)]:
        size = 1 << window
        for tiny in range(1, 7):
            row = base.Recipe(
                f'{label}_window_floor_tiny{tiny}', b'Z' * (2 * size + tiny) + tail,
                (size, size, tiny, len(tail)),
                'Two uniform window-sized blocks may leave the frontier below the next tiny-call floor; observe clamp without parser work, then resume fresh search.',
                window_log=window, hash_log=10, chain_log=10,
            )
            row.metadata()
            result.append(row)
    base.require(len(result) == len({r.name for r in result}) == 12, 'additive floor recipe count')
    return result


def manifest():
    rows = [r.metadata() for r in recipes()]
    return {'schema_version': 1, 'upstream_revision': base.REVISION,
            'status': 'source-only pre-execution additive hypotheses',
            'basis': 'ZSTD_compress_frameChunk clamps nextToUpdate before the tiny ZSTD_buildSeqStore return.',
            'recipes': rows, 'recipe_count': len(rows),
            'source_bytes': sum(r['source_bytes'] for r in rows),
            'scheduled_calls': sum(len(r['schedule']) for r in rows),
            'positive_calls': sum(sum(n > 0 for n in r['schedule']) for r in rows),
            'branches': {branch: sum(r['selected_branch'] == branch for r in rows) for branch in ['chain', 'row']}}
