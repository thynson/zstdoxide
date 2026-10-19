#!/usr/bin/env python3
"""Reject changed source-only Ultra recipe manifests without child execution."""
from pathlib import Path
import hashlib,json,sys,tempfile
import check_ultra_interop_recipes as checker

def check():
    children=[]
    def guard(event,args):
        if event in ('subprocess.Popen','os.system','os.fork','os.posix_spawn'):
            children.append(event);raise ValueError('source-only recipe controls forbid children')
    sys.addaudithook(guard)
    value=checker.check();data=checker.MANIFEST.read_bytes();rejected=[]
    with tempfile.TemporaryDirectory(prefix='ultra-public-recipes-')as directory:
        p=Path(directory)/'recipes.json';p.write_bytes(data);checker.check(p)
        for name in ['count','source-bytes','classification','source-hash','request','tuple','equivalence','missing-case','float-schema','extra-key']:
            bad=checker.decode(data)
            if name=='count':bad['recipe_count']-=1
            elif name=='source-bytes':bad['source_bytes']+=1
            elif name=='classification':bad['classifications']['success']+=1
            elif name=='source-hash':bad['recipes'][0]['source_sha256']='0'*64
            elif name=='request':bad['recipes'][-1]['request'][0]+=1
            elif name=='tuple':bad['resolved_tuples'][-1][6]=7
            elif name=='equivalence':bad['expected_rust_frame_equivalence'][0].append('not-a-case')
            elif name=='missing-case':bad['recipes'].pop()
            elif name=='float-schema':bad['schema_version']=7.0
            else:bad['extra']=True
            p.write_bytes((json.dumps(bad)+'\n').encode())
            original=checker.MANIFEST_SHA256
            try:
                checker.MANIFEST_SHA256=hashlib.sha256(p.read_bytes()).hexdigest()
                try:checker.check(p)
                except ValueError as error:
                    checker.require(str(error)=='independent source regeneration','semantic rejection after digest bypass')
                    rejected.append(name)
                else:raise ValueError('forged manifest accepted '+name)
            finally:checker.MANIFEST_SHA256=original
        for name,raw in [('truncated',data[:-8]),('trailing',data+b'{}'),('duplicate-key',b'{"schema_version":7,"schema_version":7}'),('nonfinite',b'{"x":NaN}')]:
            p.write_bytes(raw)
            try:checker.check(p)
            except ValueError:rejected.append(name)
            else:raise ValueError('malformed manifest accepted '+name)
        p.unlink();p.symlink_to(checker.MANIFEST)
        try:checker.check(p)
        except ValueError:rejected.append('symlink')
        else:raise ValueError('symlink accepted')
    checker.require(not children and len(rejected)==15,'bounded complete controls')
    return dict(status='passed',recipes=value['recipe_count'],source_bytes=value['source_bytes'],classifications=value['classifications'],equivalence_groups=len(value['expected_rust_frame_equivalence']),positives=2,semantic_digest_bypass=True,rejected=rejected,child_processes=0)

if __name__=='__main__':print(json.dumps(check(),sort_keys=True))
