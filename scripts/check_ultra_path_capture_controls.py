#!/usr/bin/env python3
"""Retain lossless complete-capture forgeries; never execute a child process."""
import argparse,copy,json,os,shutil,sys,tempfile
from pathlib import Path
sys.dont_write_bytecode=True
import check_ultra_path_capture as auditor
from oracles.ultra_scalar import need,sha

def check(capture,out):
    need(not out.exists(),'fresh controls directory');out.mkdir();children=[]
    def guard(event,args):
        if event in ('subprocess.Popen','os.system','os.posix_spawn','os.fork'):
            children.append(event);raise ValueError('child forbidden')
    sys.addaudithook(guard)
    capture=capture.resolve(strict=True);original=auditor.load((capture/'evidence.json').read_bytes());baseline=auditor.audit(capture);records=[]
    with tempfile.TemporaryDirectory(prefix='ultra-path-control-')as directory:
        root=Path(directory)/'capture';shutil.copytree(capture,root,copy_function=os.link);modified={}
        def write(name,data):
            p=root/name
            if name not in modified:modified[name]=p.read_bytes()
            p.unlink();p.write_bytes(data)
        def restore():
            for name,data in modified.items():
                p=root/name;p.unlink();p.write_bytes(data)
            modified.clear()
        def reject(name,change):
            value=copy.deepcopy(original);change(value);write('evidence.json',(json.dumps(value,sort_keys=True,indent=2)+'\n').encode());retained={}
            for path in modified:
                data=(root/path).read_bytes();dest=out/name/path;dest.parent.mkdir(parents=True,exist_ok=True);dest.write_bytes(data);retained[path]=dict(bytes=len(data),sha256=sha(data))
            try:auditor.audit(root)
            except (ValueError,OSError,KeyError,TypeError)as error:diagnostic=str(error)
            else:raise ValueError('accepted forgery '+name)
            records.append(dict(name=name,modified=retained,diagnostic=diagnostic));restore()
        normal=next(i for i,r in enumerate(original['commands'])if r['label']=='normal-compile-25');sanitized=next(i for i,r in enumerate(original['commands'])if r['label']=='sanitized-compile-25')
        for name,change in [
            ('schema-float',lambda e:e.update(schema_version=2.0)),
            ('extra-envelope',lambda e:e.update(extra=True)),
            ('self-report',lambda e:e['audit'].update(recipes=57)),
            ('incomplete',lambda e:e.update(status='running')),
            ('source-identity',lambda e:e['source'].__setitem__('scripts/oracles/ultra_paths.py','0'*64)),
            ('false-unchanged',lambda e:e.update(source_unchanged=False)),
            ('compiler',lambda e:e.update(compiler_sha256='0'*64)),
            ('reference',lambda e:e['upstream_source_sha256'].__setitem__('lib/compress/zstd_opt.c','0'*64)),
            ('environment',lambda e:e['environment'].update(DYLD_INSERT_LIBRARIES='/tmp/extra.dylib')),
            ('asan',lambda e:e['environment'].update(ASAN_OPTIONS='detect_leaks=0')),
            ('missing-command',lambda e:e['commands'].pop()),
            ('duplicate-command',lambda e:e['commands'].append(e['commands'][-1])),
            ('instrumentation',lambda e:(e['commands'][sanitized]['argv'].remove('-fsanitize=address,undefined'),e['plan'][sanitized]['argv'].remove('-fsanitize=address,undefined'))),
            ('debug',lambda e:(e['commands'][normal]['argv'].remove('-DDEBUGLEVEL=2'),e['plan'][normal]['argv'].remove('-DDEBUGLEVEL=2'))),
            ('stdin',lambda e:e['commands'][0].update(stdin='other')),
            ('exit-bool',lambda e:e['commands'][0].update(exit_code=False)),
            ('timeout',lambda e:e['commands'][0].update(seconds=126)),
            ('escaping-log',lambda e:e['commands'][0]['stdout'].update(path='../outside')),
            ('artifact',lambda e:e['artifacts']['c-normal/optimal-path'].update(sha256='0'*64)),
        ]:reject(name,change)
        for name,path in [('source-bytes','source/scripts/oracles/ultra_paths.c'),('request-bytes','requests/up_p2_ultra_alternative_seed_0.bin'),('executable','c-normal/optimal-path'),*[(f,'generated/'+f)for f in ['original_zstd_opt.c','observed_zstd_opt.c','instrumentation.json','adapter-diff.patch','adapter-digests.json']]]:
            reject(name,lambda e,p=path:write(p,b'!'+(root/p).read_bytes()[1:]))
        target=next(i for i,r in enumerate(original['commands'])if r['label']=='normal-run-up_p2_ultra_alternative_seed_0');path=original['commands'][target]['stdout']['path']
        for kind in ['policy','alternative','increment','saved-repeats','predecessor','comparison','next-price','frontier','cell','frequency','cache','sequence','query','numeric-type','padding']:
            def change(e,kind=kind):
                rows=[auditor.load(line)for line in (root/path).read_bytes().splitlines()];i=next(i for i,r in enumerate(rows)if r.get('stage')=='alternative');row=rows[i]
                if kind=='policy':row['price_policy']=0
                elif kind=='alternative':row['alternative']=None
                elif kind in ['increment','saved-repeats','predecessor','comparison','next-price']:
                    column={'increment':7,'saved-repeats':4,'predecessor':11,'comparison':8,'next-price':10}[kind];row['alternative'][column]+=1
                elif kind=='frontier':rows[i+1]['last']+=1
                elif kind=='cell':next(r for r in rows if r.get('stage')=='literal'and r['cells']['changes'])['cells']['changes'][0][1][0]+=1
                elif kind=='frequency':row['frequencies']['ll'][0]+=1
                elif kind=='cache':row['base_prices']['ll']+=1
                elif kind=='sequence':next(r for r in rows if r.get('stage')=='sequence')['sequences']=[]
                elif kind=='query':next(r for r in rows if r.get('stage')=='query')['query'][1]+=1
                elif kind=='numeric-type':row['cur']=float(row['cur'])
                elif kind=='padding':row['padding_pass']=1
                data=b''.join((json.dumps(r,sort_keys=True,separators=(',',':'))+'\n').encode()for r in rows);write(path,data);e['commands'][target]['stdout'].update(bytes=len(data),sha256=sha(data))
            reject('semantic-'+kind,change)
        negative=next(i for i,r in enumerate(original['commands'])if r['label']=='normal-run-negative-policy');negpath=original['commands'][negative]['stderr']['path']
        for name,data in [('wrong-fixed-diagnostic',b'ultra path oracle: packet magic\n'),('internal-guard-diagnostic',b'ultra path oracle: owner guard\n')]:
            def change(e,data=data):write(negpath,data);e['commands'][negative]['stderr'].update(bytes=len(data),sha256=sha(data))
            reject(name,change)
        auditor.audit(root)
    need(not children,'no child execution')
    report=dict(status='passed',optimized=not __debug__,baseline_evidence_sha256=sha((capture/'evidence.json').read_bytes()),baseline_audit=baseline,rejected=len(records),positives=2,child_attempts=0,records=records,qualification='Each directory stores every changed file in full. Apply these exact replacements to the immutable baseline capture to reconstruct each audited forgery; all other files remain baseline bytes.')
    (out/'summary.json').write_text(json.dumps(report,indent=2)+'\n');return report
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('capture',type=Path);p.add_argument('--output',type=Path,required=True);a=p.parse_args();r=check(a.capture,a.output);print(json.dumps({k:r[k]for k in ['status','optimized','rejected','positives','child_attempts']}))
