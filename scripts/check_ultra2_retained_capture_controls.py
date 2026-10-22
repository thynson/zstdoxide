#!/usr/bin/env python3
"""Strengthened ordinary Ultra controls with identical cross-build semantic forgeries."""
import argparse
import copy
import json
import os
from pathlib import Path
import shutil
import sys
import tempfile
import check_ultra2_retained_capture as auditor
from oracles.ultra2_retained import need as require


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
    with tempfile.TemporaryDirectory(prefix='ultra2-retained-forgeries-') as directory:
        root = Path(directory) / 'capture'
        shutil.copytree(capture, root, copy_function=os.link, ignore=lambda path,names: ['special'] if Path(path)==capture else [])
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

        def rewrite_output(e, path, data):
            # Corrupt both builds identically, so cross-build byte inequality
            # cannot substitute for independent semantic rejection.
            require(data != (root/path).read_bytes(), 'effective semantic mutation')
            for changed_path in [path, path.replace('/normal-', '/sanitized-')]:
                write(changed_path, data)
                record=next(c for c in e['commands'] if c['stdout']['path']==changed_path)
                record['stdout'].update(bytes=len(data),sha256=auditor.specification.sha(data))

        def reject(name, change):
            value = copy.deepcopy(original)
            change(value)
            write('evidence.json', (json.dumps(value, sort_keys=True, indent=2) + '\n').encode())
            try:
                auditor.audit(root)
            except (ValueError, OSError, KeyError, TypeError) as error:
                if name.startswith(('semantic-', 'publication-', 'entry-frontier-', 'ultra-', 'lifecycle-')) or name in ('horizon-index','path-duplicate','path-length','search-length'):
                    require(type(error)is ValueError and str(error)=='independent retained Ultra2 phase/block state/publication', 'semantic rejection must come from independent replay: '+name+': '+str(error))
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
            ('changed-failure-diagnostic', lambda e: e.__setitem__('error', 'forged')),
            ('incomplete-status', lambda e: e.__setitem__('status', 'running')),
            ('changed-source', lambda e: e['source'].__setitem__('scripts/oracles/ultra2_retained.py', '0' * 64)),
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
            ('artifact-hash', lambda e: e['artifacts']['c-normal/ultra2-retained'].__setitem__('sha256', '0' * 64)),
        ]
        for name, change in mutations:
            reject(name, change)
        for name, path in [('source-bytes', 'source/scripts/oracles/ultra2_retained.c'),
                           ('request-bytes', 'requests/u2r_wide_uniform.bin'),
                           ('executable-bytes', 'c-normal/ultra2-retained')]:
            reject(name, lambda e, path=path: write(path, b'!' + (root / path).read_bytes()[1:]))

        for filename in ['original_zstd_opt.c','observed_zstd_opt.c','instrumentation.json','adapter-diff.patch','adapter-digests.json','adapter-changes.json']:
            name='generated/'+filename
            reject('generated-'+filename,lambda e,name=name:write(name,b'!'+(root/name).read_bytes()[1:]))
        target=next(i for i,r in enumerate(original['commands'])if r['label']=='normal-run-u2r_wide_short')
        path=original['commands'][target]['stdout']['path']
        for kind in ['price','offset','match-length','literal-length','repeat','phase','reverse','frequency',
                     'sequence','query','numeric-type','padding-pass','frontier','head','node','short','before']:
            def change(e,kind=kind):
                rows=[auditor.load(line)for line in (root/path).read_bytes().splitlines()]
                row=next(r for r in rows if r.get('stage')=='literal')
                query=next(r for r in rows if r.get('stage')=='query')
                if kind in ['price','offset','match-length','literal-length','repeat']:
                    column=['price','offset','match-length','literal-length','repeat'].index(kind)
                    row['cells']['changes'][0][1][column]+=1
                elif kind=='phase':row['phase']='reverse'
                elif kind=='reverse':next(r for r in rows if r.get('stage')=='reverse')['reverse']=[0,0,0]
                elif kind=='frequency':row['frequencies']['ll'][0]+=1
                elif kind=='sequence':next(r for r in rows if r.get('stage')=='sequence')['sequences']=[]
                elif kind=='query':query['query'][3]=7
                elif kind=='numeric-type':row['cur']=float(row['cur'])
                elif kind=='padding-pass':row['padding_pass']=1
                elif kind in ['frontier','head','node','short']:
                    request=auditor.specification.recipes()['u2r_wide_short'].original
                    at={'head':0,'node':1<<request.hash_log,'short':(1<<request.hash_log)+(1<<request.chain_log),
                        'frontier':query['search']['length']-2}[kind]
                    changes=query['search']['changes']
                    found=next((entry for entry in changes if entry[0]==at),None)
                    if found is None:changes.append([at,2]);changes.sort()
                    else:found[1]+=1
                elif kind=='before':query['search_before']['changes']=[[0,2]]
                data=b''.join((json.dumps(r,sort_keys=True,separators=(',',':'))+'\n').encode()for r in rows)
                rewrite_output(e,path,data)
            reject('semantic-'+kind,change)
        def artifact(e,path,data):
            write(path,data)
            e['artifacts'][path]=dict(bytes=len(data),sha256=auditor.specification.sha(data))
        for kind in ['frame-equality','frame-header','frame-tail','raw-payload','ledger-release','ledger-bool',
                     'ledger-peak','ledger-overcap','ledger-padding']:
            def change(e,kind=kind):
                case='u2r_wide_noise_copy'
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
                        elif kind=='raw-payload':data=data[:10]+bytes([data[10]^1])+data[11:]
                        else:data=data[:-1]+bytes([data[-1]^1])
                        artifact(e,path,data)
            reject(kind,change)
        target=next(i for i,r in enumerate(original['commands'])if r['label']=='normal-run-u2r_wide_uniform')
        path=original['commands'][target]['stdout']['path']
        for kind in ['before-owner','after-owner','incoming-repeats','candidate-repeats','missing-commit','wire-kind','missing-publication','block-order']:
            def change(e,kind=kind):
                rows=[auditor.load(line)for line in (root/path).read_bytes().splitlines()]
                pub=next(r for r in rows if r['record']=='block_publish'and r['block']==1)
                if kind=='before-owner':pub['previous_owner_before']^=1
                elif kind=='after-owner':pub['previous_owner_after']^=1
                elif kind=='incoming-repeats':pub['before_repeats'][0]+=1
                elif kind=='candidate-repeats':pub['candidate_repeats'][0]+=1
                elif kind=='missing-commit':pub['owners'][pub['previous_owner_after']]=[0,0,0]
                elif kind=='wire-kind':pub['kind']=0
                elif kind=='missing-publication':rows.remove(pub)
                else:pub['block']=0
                data=b''.join((json.dumps(r,sort_keys=True,separators=(',',':'))+'\n').encode()for r in rows)
                rewrite_output(e,path,data)
            reject('publication-'+kind,change)
        for case in ['u2r_wide_uniform','u2r_wide_noise_copy','u2r_wide_horizon']:
            target=next(i for i,r in enumerate(original['commands'])if r['label']=='normal-run-'+case)
            path=original['commands'][target]['stdout']['path']
            def change(e,case=case):
                rows=[auditor.load(line)for line in (root/path).read_bytes().splitlines()]
                entry=next(r for r in rows if r.get('stage')=='begin'and r['block']==1)
                entry['search']['changes']=[[entry['search']['length']-2,3]]
                data=b''.join((json.dumps(r,sort_keys=True,separators=(',',':'))+'\n').encode()for r in rows)
                rewrite_output(e,path,data)
            reject('entry-frontier-'+case,change)
        target=next(i for i,r in enumerate(original['commands'])if r['label']=='normal-run-u2r_wide_horizon')
        path=original['commands'][target]['stdout']['path']
        for kind in ['horizon-index','path-duplicate','path-length','search-length']:
            def change(e,kind=kind):
                rows=[auditor.load(line)for line in (root/path).read_bytes().splitlines()]
                entry=next(r for r in rows if r['record']=='event'and any(c[0]==4096 for c in r['cells']['changes']))
                if kind=='horizon-index':next(c for c in entry['cells']['changes']if c[0]==4096)[0]=4099
                elif kind=='path-duplicate':entry['cells']['changes'].append(entry['cells']['changes'][-1])
                elif kind=='path-length':entry['cells']['length']=515
                else:entry['search']['length']+=1
                data=b''.join((json.dumps(r,sort_keys=True,separators=(',',':'))+'\n').encode()for r in rows)
                rewrite_output(e,path,data)
            reject(kind,change)
        target=next(i for i,r in enumerate(original['commands'])if r['label']=='normal-run-u2r_alternatives_0202')
        path=original['commands'][target]['stdout']['path']
        for kind in [*range(12), 'policy-bool', 'nonquery-search', 'alternative-stage', 'selected-frontier']:
            def change(e,kind=kind):
                rows=[auditor.load(line)for line in (root/path).read_bytes().splitlines()]
                at=next(i for i,r in enumerate(rows)if r.get('alternative') is not None and r['block']==1)
                row=rows[at]
                if type(kind)is int:row['alternative'][kind]+=1
                elif kind=='policy-bool':row['price_policy']=True
                elif kind=='nonquery-search':row['search']['changes']=[[0,123]]
                elif kind=='alternative-stage':row['stage']='literal'
                else:rows[at+1]['last']+=1
                data=b''.join((json.dumps(r,sort_keys=True,separators=(',',':'))+'\n').encode()for r in rows)
                rewrite_output(e,path,data)
            reject('ultra-'+str(kind),change)
        # Rewrite both build transcripts identically and update their hashes.
        # Only independent source replay may reject these lifecycle forgeries.
        path='logs/normal-run-u2r_alternatives_0202.stdout'
        for kind in ['seed-gate','reset-missing','reset-duplicate','full-source-bias','later-seed',
                     'caller-repeat','discarded-repeat','discarded-count','discarded-literals',
                     'reset-frontier','tables-cleared','later-origin','later-entry-frontier',
                     'later-source-size','phase-count','coherent-frequency','coherent-bias']:
            def change(e,kind=kind):
                rows=[auditor.load(line)for line in (root/path).read_bytes().splitlines()]
                reset=next(row for row in rows if row['record']=='reset')
                gate=next(row for row in rows if row['record']=='eligibility' and row['block']==1)
                if kind=='seed-gate':next(row for row in rows if row['record']=='eligibility')['seed']=False
                elif kind=='reset-missing':rows.remove(reset)
                elif kind=='reset-duplicate':rows.insert(rows.index(reset),copy.deepcopy(reset))
                elif kind=='full-source-bias':reset['origin_after']=2+rows[0]['source_bytes']
                elif kind=='later-seed':gate['seed']=True
                elif kind=='caller-repeat':reset['caller_repeats'][0]+=1
                elif kind=='discarded-repeat':reset['discarded_repeats'][0]+=1
                elif kind=='discarded-count':reset['discarded_sequences']+=1
                elif kind=='discarded-literals':reset['discarded_literals']+=1
                elif kind=='reset-frontier':reset['next_to_update']+=1
                elif kind=='tables-cleared':reset['tables_preserved']=False
                elif kind=='later-origin':next(row for row in rows if row['record']=='pass_begin' and row['block']==1)['origin']+=1
                elif kind=='later-entry-frontier':gate['next_to_update']+=1
                elif kind=='later-source-size':gate['source_bytes']+=1
                elif kind=='phase-count':rows[-1]['algorithm_passes']+=1
                elif kind=='coherent-frequency':
                    for row in rows:
                        if 'frequency' in row:
                            row['frequency']['tables']['ll'][0]+=1;row['frequency']['sums']['ll']+=1
                        if 'frequencies' in row:
                            row['frequencies']['ll'][0]+=1;row['sums']['ll']+=1
                else:
                    for row in rows:
                        for key in ['origin','origin_after','dictionary_limit','low_limit','next_to_update']:
                            if key in row and row[key]>2:row[key]+=1
                data=b''.join((json.dumps(row,sort_keys=True,separators=(',',':'))+'\n').encode() for row in rows)
                rewrite_output(e,path,data)
            reject('lifecycle-'+kind,change)
        target=next(i for i,r in enumerate(original['commands'])if r['label']=='normal-run-negative-policy-one')
        path=original['commands'][target]['stderr']['path']
        def wrong_diagnostic(e):
            data=b'Retained Ultra2 observer: request admission\n'
            write(path,data);e['commands'][target]['stderr'].update(bytes=len(data),sha256=auditor.specification.sha(data))
        reject('wrong-exact-diagnostic',wrong_diagnostic)
        auditor.audit(root)
    require(not children,'no child execution')
    return dict(status='passed',positives=2,rejected=len(rejected),cases=rejected,child_processes=0,semantic_cross_build_forgeries_equal=True,semantic_rejection='independent retained Ultra2 phase/block state/publication')

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('capture',type=Path)
    print(json.dumps(check(parser.parse_args().capture),sort_keys=True))
