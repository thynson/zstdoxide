#!/usr/bin/env python3
"""Pure preparation and forged-plan controls for scripted path capture."""
import argparse
from copy import deepcopy
import json
from pathlib import Path
import subprocess
import optimal_path_capture_plan as specification
from capture_optimal_paths import prepare
from oracles.optimal_paths import require
from oracles.optimal_path_instrumentation import instrument


def check(upstream, output, compiler):
    children = []
    original = subprocess.Popen
    def blocked(*args, **kwargs):
        children.append((args, kwargs))
        raise ValueError('pure preflight attempted a child')
    subprocess.Popen = blocked
    try:
        upstream, output, compiler, source, reference, rows = prepare(upstream, output, compiler)
        _, units = specification.reference_sources(upstream)
        rejected = 0
        forged = [rows[:-1], rows + [rows[-1]], list(reversed(rows))]
        for field, value in [('expected_exit', 1), ('timeout_seconds', 999), ('stdout_limit', 1 << 30), ('stderr_limit', 1 << 30), ('label', 'forged')]:
            changed = deepcopy(rows)
            changed[0][field] = value
            forged.append(changed)
        changed = deepcopy(rows)
        row = next(row for row in changed if row['label'] == 'sanitized-compile-0')
        row['argv'].remove('-fsanitize=address,undefined')
        forged.append(changed)
        for changed in forged:
            try: specification.validate_plan(changed, upstream, output, compiler, units)
            except ValueError: rejected += 1
            else: raise ValueError('forged plan accepted')
        raw = (upstream/'lib/compress/zstd_opt.c').read_bytes()
        transformed, manifest = instrument(raw)
        require(len(manifest['substitutions']) == 15 and transformed != raw, 'reviewed unique substitutions')
        anchor = b'ip += (ip==prefixStart);'
        for bad in [raw.replace(anchor, b'/* absent */'), raw + b'\n' + anchor]:
            try: instrument(bad)
            except ValueError: rejected += 1
            else: raise ValueError('nonunique instrumentation accepted')
        require(not children, 'pure preflight')
        return dict(status='passed', commands=len(rows), codec_units=len(units)+1,
                    source_files=len(source), reference_files=len(reference), substitutions=15,
                    negative_packets=len(specification.malformed_packets()), forged_rejected=rejected, child_processes=0)
    finally:
        subprocess.Popen = original


if __name__ == '__main__':
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--upstream',type=Path,required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--cc',default='clang')
    a=p.parse_args();print(json.dumps(check(a.upstream,a.output,a.cc),sort_keys=True))
