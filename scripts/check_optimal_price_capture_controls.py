#!/usr/bin/env python3
"""Reject mutated complete Optimal price captures with no child execution."""
import argparse
import copy
import json
import os
from pathlib import Path
import shutil
import sys
import tempfile
import check_optimal_price_capture_v2 as auditor
from oracles.optimal_prices_v2 import require


def check(capture):
    children = []

    def guard(event, args):
        if event in ('subprocess.Popen', 'os.system', 'os.posix_spawn', 'os.fork'):
            children.append(event)
            raise ValueError('child execution forbidden')

    sys.addaudithook(guard)
    capture = capture.resolve(strict=True)
    original_bytes = (capture / 'evidence.json').read_bytes()
    original = auditor.original.load(original_bytes)
    result = auditor.audit(capture)
    with tempfile.TemporaryDirectory(prefix='optimal-price-forgeries-') as directory:
        root = Path(directory) / 'capture'
        shutil.copytree(capture, root, copy_function=os.link)
        rejected = []
        modified = {}

        def write(name, data):
            p = root / name
            if name not in modified:
                modified[name] = p.read_bytes()
            p.unlink()
            p.write_bytes(data)

        def restore():
            for name, data in modified.items():
                p = root / name
                p.unlink()
                p.write_bytes(data)
            modified.clear()

        def reject(name, change):
            value = copy.deepcopy(original)
            change(value)
            write('evidence.json', (json.dumps(value, sort_keys=True, indent=2) + '\n').encode())
            try:
                auditor.audit(root)
            except (ValueError, OSError, KeyError, TypeError):
                rejected.append(name)
            else:
                raise ValueError('accepted forgery: ' + name)
            finally:
                restore()

        normal = next(i for i, r in enumerate(original['commands']) if r['label'] == 'normal-compile-25')
        sanitized = next(i for i, r in enumerate(original['commands']) if r['label'] == 'sanitized-compile-25')
        mutations = [
            ('float-schema', lambda e: e.__setitem__('schema_version', 2.0)),
            ('extra-envelope', lambda e: e.__setitem__('extra', True)),
            ('changed-self-report', lambda e: e['audit'].__setitem__('recipes', 148)),
            ('incomplete-status', lambda e: e.__setitem__('status', 'running')),
            ('changed-source', lambda e: e['source'].__setitem__('scripts/oracles/optimal_prices_v2.py', '0' * 64)),
            ('false-unchanged', lambda e: e.__setitem__('source_unchanged', False)),
            ('compiler-hash', lambda e: e.__setitem__('compiler_sha256', '0' * 64)),
            ('reference-hash', lambda e: e['upstream_source_sha256'].__setitem__('lib/compress/zstd_opt.c', '0' * 64)),
            ('unsafe-env', lambda e: e['environment'].__setitem__('DYLD_INSERT_LIBRARIES', '/tmp/extra.dylib')),
            ('asan-env', lambda e: e['environment'].__setitem__('ASAN_OPTIONS', 'detect_leaks=0')),
            ('ubsan-env', lambda e: e['environment'].__setitem__('UBSAN_OPTIONS', 'halt_on_error=0')),
            ('missing-command', lambda e: e['commands'].pop()),
            ('duplicate-command', lambda e: e['commands'].append(copy.deepcopy(e['commands'][-1]))),
            ('missing-instrumentation', lambda e: (e['commands'][sanitized]['argv'].remove('-fsanitize=address,undefined'), e['plan'][sanitized]['argv'].remove('-fsanitize=address,undefined'))),
            ('disabled-debug', lambda e: (e['commands'][normal]['argv'].remove('-DDEBUGLEVEL=2'), e['plan'][normal]['argv'].remove('-DDEBUGLEVEL=2'))),
            ('loose-output-bound', lambda e: (e['commands'][0].__setitem__('stdout_limit', 4 << 20), e['plan'][0].__setitem__('stdout_limit', 4 << 20))),
            ('wrong-stdin', lambda e: e['commands'][0].__setitem__('stdin', 'other')),
            ('false-exit', lambda e: e['commands'][0].__setitem__('exit_code', False)),
            ('timeout', lambda e: e['commands'][0].__setitem__('seconds', 126)),
            ('escaping-log', lambda e: e['commands'][0]['stdout'].__setitem__('path', '../outside')),
            ('log-hash', lambda e: e['commands'][0]['stdout'].__setitem__('sha256', '0' * 64)),
            ('artifact-hash', lambda e: e['artifacts']['c-normal/optimal-price'].__setitem__('sha256', '0' * 64)),
        ]
        for name, change in mutations:
            reject(name, change)
        for name, path in [('source-bytes', 'source/scripts/oracles/optimal_price_v2.c'),
                           ('request-bytes', 'requests/op_init_single_9_m0.bin'),
                           ('executable-bytes', 'c-normal/optimal-price')]:
            reject(name, lambda e, path=path: write(path, b'!' + (root / path).read_bytes()[1:]))

        target = next(i for i, r in enumerate(original['commands']) if r['label'] == 'normal-run-op_grouped_dirty_cache')
        path = original['commands'][target]['stdout']['path']
        for kind in ('array', 'coherent-array-sum', 'early-cache', 'query-price', 'numeric-type'):
            def change(e, kind=kind):
                rows = [auditor.original.load(line) for line in (root / path).read_bytes().splitlines()]
                if kind == 'array':
                    rows[1]['frequencies']['lit'][0] += 1
                elif kind == 'coherent-array-sum':
                    rows[1]['frequencies']['lit'][0] += 1
                    rows[1]['sums']['lit'] += 1
                elif kind == 'early-cache':
                    rows[2]['base_prices']['lit'] += 256
                elif kind == 'query-price':
                    next(r for r in rows if r.get('query') is not None)['query'][2] += 1
                else:
                    rows[1]['index'] = 0.0
                data = b''.join((json.dumps(r, separators=(',', ':')) + '\n').encode() for r in rows)
                write(path, data)
                e['commands'][target]['stdout'].update(bytes=len(data), sha256=auditor.original.specification.sha(data))
            reject('coherent-log-' + kind, change)
        require(auditor.audit(root) == result, 'restored positive capture')
        require(len(rejected) == 30 and not children and (capture / 'evidence.json').read_bytes() == original_bytes,
                'complete controls and unchanged original')
    print(json.dumps(dict(status='passed', rejections=rejected, child_attempts=len(children),
                          positive_audits=2, audit=result), sort_keys=True, indent=2))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('capture', type=Path)
    check(parser.parse_args().capture)
