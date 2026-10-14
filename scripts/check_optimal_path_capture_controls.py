#!/usr/bin/env python3
"""Reject mutated complete scripted Optimal path captures with no child execution."""
import argparse
import copy
import json
import os
from pathlib import Path
import shutil
import sys
import tempfile
import check_optimal_path_capture as auditor
from oracles.optimal_paths import require


def check(capture):
    children = []

    def guard(event, args):
        if event in ('subprocess.Popen', 'os.system', 'os.posix_spawn', 'os.fork'):
            children.append(event)
            raise ValueError('child execution forbidden')

    sys.addaudithook(guard)
    capture = capture.resolve(strict=True)
    original_bytes = (capture / 'evidence.json').read_bytes()
    original = auditor.load(original_bytes)
    result = auditor.audit(capture)
    with tempfile.TemporaryDirectory(prefix='optimal-path-forgeries-') as directory:
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
            ('changed-self-report', lambda e: e['audit'].__setitem__('recipes', 38)),
            ('incomplete-status', lambda e: e.__setitem__('status', 'running')),
            ('changed-source', lambda e: e['source'].__setitem__('scripts/oracles/optimal_paths.py', '0' * 64)),
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
            ('artifact-hash', lambda e: e['artifacts']['c-normal/optimal-path'].__setitem__('sha256', '0' * 64)),
        ]
        for name, change in mutations:
            reject(name, change)
        for name, path in [('source-bytes', 'source/scripts/oracles/optimal_paths.c'),
                           ('request-bytes', 'requests/path_script_seed_0.bin'),
                           ('executable-bytes', 'c-normal/optimal-path')]:
            reject(name, lambda e, path=path: write(path, b'!' + (root / path).read_bytes()[1:]))

        for filename in ['original_zstd_opt.c','observed_zstd_opt.c','instrumentation.json']:
            name='generated/'+filename
            reject('generated-'+filename,lambda e,name=name:write(name,b'!'+(root/name).read_bytes()[1:]))
        target=next(i for i,r in enumerate(original['commands'])if r['label']=='normal-run-path_script_seed_0')
        path=original['commands'][target]['stdout']['path']
        for kind in ['price','offset','match-length','literal-length','repeat','phase','reverse','frequency','sequence','query','numeric-type','padding-pass']:
            def change(e,kind=kind):
                rows=[auditor.load(line)for line in (root/path).read_bytes().splitlines()]
                row=next(r for r in rows if r.get('stage')=='literal')
                if kind in ['price','offset','match-length','literal-length','repeat']:
                    column=['price','offset','match-length','literal-length','repeat'].index(kind)
                    row['cells'][row['cur']][column]+=1
                elif kind=='phase':row['phase']='reverse'
                elif kind=='reverse':next(r for r in rows if r.get('stage')=='reverse')['reverse']=[0,0,0]
                elif kind=='frequency':row['frequencies']['ll'][0]+=1
                elif kind=='sequence':next(r for r in rows if r.get('stage')=='sequence')['sequences']=[]
                elif kind=='query':next(r for r in rows if r.get('stage')=='query')['query'][1]+=1
                elif kind=='numeric-type':row['cur']=float(row['cur'])
                elif kind=='padding-pass':row['padding_pass']=1
                data=b''.join((json.dumps(r,sort_keys=True,separators=(',',':'))+'\n').encode()for r in rows)
                write(path,data)
                e['commands'][target]['stdout']['bytes']=len(data)
                e['commands'][target]['stdout']['sha256']=auditor.specification.sha(data)
            reject('semantic-'+kind,change)
        auditor.audit(root)
    require(not children,'no child execution')
    return dict(status='passed',positives=2,rejected=len(rejected),cases=rejected,child_processes=0)

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('capture',type=Path)
    print(json.dumps(check(parser.parse_args().capture),sort_keys=True))
