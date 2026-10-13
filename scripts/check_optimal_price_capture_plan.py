#!/usr/bin/env python3
"""Preflight exact Optimal price commands and reject plan mutations without children."""
import argparse
import copy
import json
from pathlib import Path
import sys
import tempfile
import capture_optimal_prices as capture
import optimal_price_capture_plan as plan
from oracles.optimal_prices_v2 import require


def check(upstream):
    children = []

    def guard(event, args):
        if event in ('subprocess.Popen', 'os.system', 'os.posix_spawn', 'os.fork'):
            children.append(event)
            raise ValueError('child execution forbidden')

    sys.addaudithook(guard)
    with tempfile.TemporaryDirectory(prefix='optimal-price-preflight-') as directory:
        output = Path(directory) / 'fresh'
        u, o, cc, source, _, rows = capture.prepare(upstream, output, 'clang')
        _, units = plan.reference_sources(u)
        require(len(rows) == 390 and len(plan.requests()) == 164, 'complete bounded command inventory')
        require(plan.validate_plan(rows, u, o, cc, units) == 390, 'positive exact plan')
        rejected = []

        def reject(name, callback):
            try:
                callback()
            except (ValueError, OSError):
                rejected.append(name)
            else:
                raise ValueError('accepted bad plan: ' + name)

        normal = next(i for i, row in enumerate(rows) if row['label'] == 'normal-compile-25')
        sanitized = next(i for i, row in enumerate(rows) if row['label'] == 'sanitized-compile-25')
        negative = next(i for i, row in enumerate(rows) if row['expected_exit'] == 1)
        mutations = [
            ('missing-command', lambda r: r.pop()),
            ('extra-command', lambda r: r.append(copy.deepcopy(r[-1]))),
            ('reordered-commands', lambda r: r.__setitem__(slice(0, 2), [r[1], r[0]])),
            ('changed-working-input', lambda r: r[0]['argv'].__setitem__(2, '/tmp/other-reference')),
            ('missing-sanitizer', lambda r: r[sanitized]['argv'].remove('-fsanitize=address,undefined')),
            ('sanitizer-recovery', lambda r: r[sanitized]['argv'].remove('-fno-sanitize-recover=all')),
            ('enable-assembly', lambda r: r[normal]['argv'].remove('-DZSTD_DISABLE_ASM')),
            ('enable-threads', lambda r: r[normal]['argv'].append('-DZSTD_MULTITHREAD')),
            ('disable-debug', lambda r: r[normal]['argv'].remove('-DDEBUGLEVEL=2')),
            ('wrong-helper', lambda r: r[normal]['argv'].__setitem__(-3, '/tmp/other.c')),
            ('wrong-negative-status', lambda r: r[negative].__setitem__('expected_exit', 0)),
            ('boolean-status', lambda r: r[0].__setitem__('expected_exit', False)),
            ('loose-timeout', lambda r: r[0].__setitem__('timeout_seconds', 121)),
            ('loose-stdout', lambda r: r[0].__setitem__('stdout_limit', 4 << 20)),
            ('loose-stderr', lambda r: r[0].__setitem__('stderr_limit', 2 << 20)),
        ]
        for name, change in mutations:
            changed = copy.deepcopy(rows)
            change(changed)
            reject(name, lambda changed=changed: plan.validate_plan(changed, u, o, cc, units))
        reject('repository-output', lambda: capture.prepare(upstream, plan.ROOT / 'forbidden-capture', 'clang'))
        reject('reference-output', lambda: capture.prepare(upstream, upstream / 'forbidden-capture', 'clang'))
        reject('existing-output', lambda: capture.prepare(upstream, Path(directory), 'clang'))
        reject('missing-compiler', lambda: capture.prepare(upstream, output, 'missing-optimal-price-compiler-54321'))
        require(len(rejected) == 19 and not children and not output.exists(), 'rejection count/no side effects')
        print(json.dumps(dict(status='passed', commands=390, positive_requests=147,
                              malformed_packets=17, plan_rejections=rejected,
                              child_attempts=len(children), source=source), sort_keys=True, indent=2))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--upstream', type=Path, required=True)
    check(parser.parse_args().upstream)
