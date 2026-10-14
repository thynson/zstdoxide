#!/usr/bin/env python3
"""Offline forged diagnostic timing controls; never execute observed binaries."""
import argparse
import json
from pathlib import Path
import shutil
import subprocess
import tempfile

from benchmark_optimal_parse import audit, need


def check(root):
    children = []
    def blocked(*args, **kwargs):
        children.append((args, kwargs))
        raise ValueError('offline audit attempted a child')
    original = subprocess.Popen
    subprocess.Popen = blocked
    try:
        audit(root)
        rejected = 0
        with tempfile.TemporaryDirectory(prefix='optimal-parse-timing-controls-') as directory:
            copy = Path(directory) / 'capture'
            shutil.copytree(root, copy, ignore=shutil.ignore_patterns('target'))
            def corrupt(name, mutate):
                nonlocal rejected
                path = copy / name
                raw = path.read_bytes()
                try:
                    path.write_bytes(mutate(raw))
                    try:
                        audit(copy)
                    except (ValueError, KeyError, TypeError):
                        rejected += 1
                    else:
                        raise RuntimeError('forged timing accepted: ' + name)
                finally:
                    path.write_bytes(raw)
            def change(name, mutate):
                def transform(raw):
                    data = json.loads(raw)
                    mutate(data)
                    return json.dumps(data).encode()
                corrupt(name, transform)
            change('prepare.json', lambda d: d.update(status='failed'))
            change('prepare.json', lambda d: d['expected'][0].update(checksum='0' * 16))
            change('first.json', lambda d: d['results'].pop('paths'))
            change('repeat.json', lambda d: d.update(status='running'))
            for field, value in [('checksum', '0' * 16), ('events', 0), ('allocations', 5),
                                 ('owner_bytes', 1), ('rounds', 31), ('samples_ns', [-1] * 5),
                                 ('samples_ns', [True] * 5), ('samples_ns', [1] * 4)]:
                change('first.json', lambda d, f=field, v=value: d['results']['safe'][0].update({f: v}))
            change('prepare.json',lambda d:d['commands'].pop())
            change('prepare.json',lambda d:d['commands'].append(d['commands'][0]))
            change('prepare.json',lambda d:d['commands'][0].update(exit_code=False))
            change('prepare.json',lambda d:d['source'].update({'../escape':'0'*64}))
            change('first.json',lambda d:d['gates'].pop())
            change('repeat.json',lambda d:d['gates'][0].update(path='../escape'))
            change('gates/matrix.json',lambda d:d.update(status='running'))
            change('gates/miri.json',lambda d:d['commands'].pop())
            corrupt('first-safe.log', lambda b: b + b'{}\n')
            corrupt('verify-safe.log', lambda b: b + b'{}\n')
            corrupt('parse-safe', lambda b: b + b'!')
            corrupt('snapshot/src/optimal_parse.rs', lambda b: b + b'\n')
            audit(copy)
        need(not children, 'no child processes')
        return dict(status='passed', positives=2, rejected=rejected, child_processes=0)
    finally:
        subprocess.Popen = original


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root', type=Path)
    print(json.dumps(check(parser.parse_args().root), sort_keys=True))
