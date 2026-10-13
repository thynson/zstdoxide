#!/usr/bin/env python3
"""Strict envelope and self-report extension to the immutable price capture audit.

The captured v1 auditor remains byte-for-byte frozen. This additive v2 wrapper
rejects top-level numeric type substitutions/unknown fields and independently
checks the saved audit summary; all original source, command and state checks run.
"""
import argparse
import hashlib
import json
from pathlib import Path
import sys
sys.dont_write_bytecode = True
SOURCE = Path(__file__).with_name('check_optimal_price_capture.py')
if hashlib.sha256(SOURCE.read_bytes()).hexdigest() != 'fef7d4c96d1bfb5d45d21772d8a5b3201c2266f4182adadd1d74112ce29ab047':
    raise ValueError('frozen original auditor identity')
import check_optimal_price_capture as original
from oracles.optimal_prices_v2 import require


def audit(root):
    root = root.resolve(strict=True)
    e = original.load(original.read(root, 'evidence.json', 8 << 20))
    keys = {'schema_version', 'status', 'started_at', 'cwd', 'upstream', 'output',
            'compiler', 'compiler_sha256', 'source', 'upstream_revision',
            'upstream_source_sha256', 'environment', 'plan', 'commands', 'artifacts',
            'completed_at', 'source_unchanged'}
    if e.get('status') == 'passed':
        keys.add('audit')
    require(set(e) == keys, 'strict capture envelope')
    require(type(e['schema_version']) is int and e['schema_version'] == 2,
            'integer evidence schema version')
    require(all(type(e[k]) is str for k in ('started_at', 'completed_at', 'cwd',
            'upstream', 'output', 'compiler', 'compiler_sha256', 'upstream_revision')),
            'capture envelope string types')
    result = original.audit(root)
    if 'audit' in e:
        require(original.canonical(e['audit']) == original.canonical(result),
                'saved self-report equals independent replay')
    return result


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('capture', type=Path)
    print(json.dumps(audit(parser.parse_args().capture), sort_keys=True, indent=2))
