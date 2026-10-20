#!/usr/bin/env python3
"""Offline copied-capture adversarial controls with no child execution."""
import argparse
import copy
import hashlib
import json
from pathlib import Path
import shutil
import sys

sys.dont_write_bytecode = True
import check_ultra2_capture as auditor
from oracles.ultra_scalar import need


def sha(data):return hashlib.sha256(data).hexdigest()
def encode(value):return (json.dumps(value,sort_keys=True,indent=2)+'\n').encode()


def check(original, output):
    need(not output.exists(), 'fresh control output')
    original=original.resolve(strict=True);output.mkdir(parents=True);work=output/'working-copy';work.mkdir()
    for name in ('logs','requests','source','generated','artifacts','c-normal','c-sanitized'):
        shutil.copytree(original/name,work/name)
    initial=(original/'evidence.json').read_bytes();baseline=auditor.load(initial)
    (work/'evidence.json').write_bytes(initial)
    children=[]
    def guard(event,args):
        if event in ('subprocess.Popen','os.system','os.fork','os.posix_spawn','os.exec','os.spawn'):
            children.append(event);raise RuntimeError('offline controls forbid child execution')
    sys.addaudithook(guard)
    records=[];saved={};mutations={}
    def change_file(name,data):
        path=work/name
        if name not in saved:saved[name]=path.read_bytes() if path.exists() else None
        path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes(data);mutations[name]=data
    def change_log(e,label,stream,data):
        name='logs/'+label+'.'+stream;change_file(name,data)
        next(row for row in e['commands']if row['label']==label)[stream]=dict(path=name,bytes=len(data),sha256=sha(data))
    def change_artifact(e,name,value):
        data=encode(value);change_file(name,data);e['artifacts'][name]=dict(bytes=len(data),sha256=sha(data))
    target='u2_seed_fresh_startup_uniform'
    def transcript(e,mutate,both=False):
        label='normal-run-'+target
        rows=[auditor.load(line)for line in (work/'logs'/(label+'.stdout')).read_bytes().splitlines()]
        mutate(rows);data=b''.join((auditor.canonical(row)+'\n').encode()for row in rows)
        change_log(e,label,'stdout',data)
        if both:change_log(e,'sanitized-run-'+target,'stdout',data)
    def run(name,mutate,positive=False):
        saved.clear();mutations.clear();e=copy.deepcopy(baseline);mutate(e);change_file('evidence.json',encode(e))
        try:auditor.audit(work)
        except (ValueError,KeyError,TypeError,IndexError,OSError)as error:accepted=False;diagnostic=str(error)
        else:accepted=True;diagnostic=None
        passed=accepted==positive
        snapshot=output/'mutations'/name;snapshot.mkdir(parents=True)
        inventory={}
        for path,data in mutations.items():
            dest=snapshot/path;dest.parent.mkdir(parents=True,exist_ok=True);dest.write_bytes(data)
            inventory[path]=dict(bytes=len(data),sha256=sha(data))
        records.append(dict(name=name,expected='accepted'if positive else'rejected',accepted=accepted,
                            passed=passed,diagnostic=diagnostic,mutations=inventory))
        for path,data in saved.items():
            dest=work/path
            if data is None:dest.unlink()
            else:dest.write_bytes(data)
        print(name,'PASS'if passed else'FAIL',flush=True)
        need(passed,'unexpected control result: '+name)
    run('positive',lambda e:None,True)
    run('missing-command',lambda e:e['commands'].pop())
    run('reordered-commands',lambda e:e['commands'].reverse())
    run('boolean-exit',lambda e:e['commands'][0].update(exit_code=False))
    run('unknown-envelope-field',lambda e:e.update(unobserved=True))
    run('compiler-identity',lambda e:e.update(compiler_sha256='0'*64))
    run('unsafe-environment',lambda e:e['environment'].update(DYLD_INSERT_LIBRARIES='/tmp/injected'))
    run('missing-sanitizer',lambda e:next(r for r in e['commands']if r['label']=='sanitized-compile-0')['argv'].remove('-fsanitize=address,undefined'))
    run('source-closure',lambda e:e['source'].update({'scripts/oracles/ultra2.c':'0'*64}))
    run('retained-source',lambda e:change_file('source/scripts/oracles/ultra2.c',b'forged'))
    run('generated-source',lambda e:change_file('generated/observed_zstd_opt.c',b'forged'))
    run('request-bytes',lambda e:change_file('requests/'+target+'.bin',b'forged'))
    run('false-completion',lambda e:e.update(status='running'))
    run('false-source-stability',lambda e:e.update(source_unchanged=1))
    negative=next(row['label']for row in baseline['commands']if '-run-negative-'in row['label'])
    run('malformed-diagnostic',lambda e:change_log(e,negative,'stderr',b'forged\n'))
    ledger='artifacts/normal/'+target+'.0.json'
    value=auditor.load((work/ledger).read_bytes())
    run('allocation-boolean',lambda e:change_artifact(e,ledger,dict(value,allocations=[True])))
    run('allocation-residual',lambda e:change_artifact(e,ledger,dict(value,released=False)))
    run('allocation-peak',lambda e:change_artifact(e,ledger,dict(value,peak_bytes=0)))
    def event(rows):return next(r for r in rows if r.get('pass_name')=='encoding' and r.get('stage')=='query')
    def reset(rows):return next(r for r in rows if r['record']=='reset')
    for name,mutate in [
        ('event-boolean',lambda rows:event(rows).update(index=True)),
        ('pass-swap',lambda rows:event(rows).update(pass_name='seeding')),
        ('stale-origin',lambda rows:reset(rows).update(origin_after=2)),
        ('stale-frontier',lambda rows:reset(rows).update(next_to_update=2)),
        ('discarded-count',lambda rows:reset(rows).update(discarded_sequences=0)),
        ('caller-repeat',lambda rows:reset(rows)['caller_repeats'].__setitem__(0,99)),
        ('discarded-repeat',lambda rows:reset(rows)['discarded_repeats'].__setitem__(0,99)),
        ('reset-cache',lambda rows:reset(rows)['frequency']['cache'].__setitem__('ll',0)),
        ('old-tree-cell',lambda rows:event(rows)['search']['changes'].append([64,2])),
        ('missing-reset',lambda rows:rows.remove(reset(rows))),
        ('event-count',lambda rows:rows[-1].update(events=0)),
        ('padding-identity',lambda rows:event(rows).update(padding_pass=1)),
    ]:run(name,lambda e,f=mutate:transcript(e,f,True))
    def coherent(rows):
        for row in rows:
            if 'frequency'in row:
                row['frequency']['tables']['ll'][0]+=1;row['frequency']['sums']['ll']+=1
            if 'frequencies'in row:
                row['frequencies']['ll'][0]+=1;row['sums']['ll']+=1
    run('coherent-both-build-frequency',lambda e:transcript(e,coherent,True))
    run('saved-audit',lambda e:e['audit'].update(recipes=52))
    run('restored-positive',lambda e:None,True)
    need(not children and (original/'evidence.json').read_bytes()==initial,'no child or original mutation')
    result=dict(status='passed',optimized=not __debug__,checks=len(records),rejected=sum(not r['accepted']for r in records),
                child_processes=0,source_sha256=sha(Path(__file__).read_bytes()),original_evidence_sha256=sha(initial),records=records,
                qualification='Full copied-capture audits. Every forged file and envelope is retained; the working copy is restored after each case. Original capture is never written.')
    (output/'summary.json').write_bytes(encode(result));return result


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('capture',type=Path);p.add_argument('output',type=Path)
    a=p.parse_args();result=check(a.capture,a.output)
    print(json.dumps({k:v for k,v in result.items()if k!='records'},sort_keys=True))
