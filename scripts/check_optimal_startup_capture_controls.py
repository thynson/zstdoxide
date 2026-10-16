#!/usr/bin/env python3
"""Reject mutated complete fresh ordinary Optimal startup captures with no child execution."""
import argparse
import copy
import json
import os
from pathlib import Path
import shutil
import sys
import tempfile
import check_optimal_startup_capture_v2 as auditor
from oracles.optimal_startup import require


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
    with tempfile.TemporaryDirectory(prefix='optimal-startup-forgeries-') as directory:
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
            ('changed-source', lambda e: e['source'].__setitem__('scripts/oracles/optimal_startup.py', '0' * 64)),
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
            ('artifact-hash', lambda e: e['artifacts']['c-normal/optimal-startup'].__setitem__('sha256', '0' * 64)),
        ]
        for name, change in mutations:
            reject(name, change)
        for name, path in [('source-bytes', 'source/scripts/oracles/optimal_startup.c'),
                           ('request-bytes', 'requests/startup_uniform.bin'),
                           ('executable-bytes', 'c-normal/optimal-startup')]:
            reject(name, lambda e, path=path: write(path, b'!' + (root / path).read_bytes()[1:]))

        for filename in ['original_zstd_opt.c','observed_zstd_opt.c','instrumentation.json']:
            name='generated/'+filename
            reject('generated-'+filename,lambda e,name=name:write(name,b'!'+(root/name).read_bytes()[1:]))
        target=next(i for i,r in enumerate(original['commands'])if r['label']=='normal-run-startup_three_short')
        path=original['commands'][target]['stdout']['path']
        for kind in ['price','offset','match-length','literal-length','repeat','phase','reverse','frequency',
                     'sequence','query','numeric-type','padding-pass','frontier','head','node','short','before']:
            def change(e,kind=kind):
                rows=[auditor.load(line)for line in (root/path).read_bytes().splitlines()]
                row=next(r for r in rows if r.get('stage')=='literal')
                query=next(r for r in rows if r.get('stage')=='query')
                if kind in ['price','offset','match-length','literal-length','repeat']:
                    column=['price','offset','match-length','literal-length','repeat'].index(kind)
                    row['cells'][0][column]+=1
                elif kind=='phase':row['phase']='reverse'
                elif kind=='reverse':next(r for r in rows if r.get('stage')=='reverse')['reverse']=[0,0,0]
                elif kind=='frequency':row['frequencies']['ll'][0]+=1
                elif kind=='sequence':next(r for r in rows if r.get('stage')=='sequence')['sequences']=[]
                elif kind=='query':query['query'][3]=7
                elif kind=='numeric-type':row['cur']=float(row['cur'])
                elif kind=='padding-pass':row['padding_pass']=1
                elif kind=='frontier':query['search']['frontier']+=1
                elif kind=='head':query['search']['heads'][0]+=1
                elif kind=='node':query['search']['nodes'][0][0]+=1
                elif kind=='short':query['search']['short_heads'][0]+=1
                elif kind=='before':query['search_before']['frontier']+=1
                data=b''.join((json.dumps(r,sort_keys=True,separators=(',',':'))+'\n').encode()for r in rows)
                write(path,data)
                e['commands'][target]['stdout']['bytes']=len(data)
                e['commands'][target]['stdout']['sha256']=auditor.specification.sha(data)
            reject('semantic-'+kind,change)
        def artifact(e,path,data):
            write(path,data)
            e['artifacts'][path]=dict(bytes=len(data),sha256=auditor.specification.sha(data))
        for kind in ['frame-equality','frame-header','frame-tail','raw-payload','ledger-release','ledger-bool',
                     'ledger-peak','ledger-overcap','ledger-padding']:
            def change(e,kind=kind):
                case='startup_noise'
                paths=[p for mode in ['normal','sanitized'] for p in auditor.specification.artifact_paths(mode,case)]
                if kind.startswith('ledger'):
                    targets=[p for p in paths if p.endswith('.json')]
                    if kind=='ledger-padding':targets=targets[:1]
                    for path in targets:
                        value=auditor.load((root/path).read_bytes())
                        if kind=='ledger-release':value['released']=False
                        elif kind=='ledger-bool':value['allocations'][0]=True
                        elif kind=='ledger-peak':value['peak_bytes']=0
                        elif kind=='ledger-overcap':value['allocations']=[(8<<20)+1]
                        else:value['allocations'][0]+=1
                        artifact(e,path,(json.dumps(value)+'\n').encode())
                else:
                    targets=[p for p in paths if p.endswith('.zst')]
                    if kind=='frame-equality':targets=targets[:1]
                    for path in targets:
                        data=(root/path).read_bytes()
                        if kind=='frame-header':data=data[:5]+bytes([data[5]^1])+data[6:]
                        elif kind=='frame-tail':data+=b'!'
                        else:data=data[:-1]+bytes([data[-1]^1])
                        artifact(e,path,data)
            reject(kind,change)
        auditor.audit(root)
    require(not children,'no child execution')
    return dict(status='passed',positives=2,rejected=len(rejected),cases=rejected,child_processes=0)

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('capture',type=Path)
    print(json.dumps(check(parser.parse_args().capture),sort_keys=True))
