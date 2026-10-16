#!/usr/bin/env python3
"""Independent complete-state audit for deterministic frequency measurements."""
from pathlib import Path
import argparse,copy,json,sys
sys.dont_write_bytecode=True
from oracles import ultra_frequency as model
from oracles.ultra_frequency_recipes import recipes
from oracles.ultra_scalar import need,integer,load,sha
ROOT=Path(__file__).resolve().parents[1]
SUFFIXES=('init_single_9_m0','init_cycle_131072_m0','counts_8192_m0','symbols_n9_m1_2','grouped_dirty_cache','fraction_boundary_30')
PACK_SHA='a92871e38d38d30749d7d65ac710e133500ab65348191ba9579e3592f95aa11c'

def expectations():
    need(sha((ROOT/'tests/fixtures/reference-ultra-frequencies/cases.bin').read_bytes())==PACK_SHA,'native fixture pin')
    result=[]
    for name,request,_ in recipes():
        if name[6:]not in SUFFIXES:continue
        observed=list(model.observations(request))[-2];original=request.original
        state=dict(counts=[n for key in model.prior.KEYS for n in observed['frequencies'][key]],sums=[observed['sums'][key]for key in model.prior.KEYS],base_prices=[observed['base_prices'][key]for key in model.prior.KEYS],predefined=observed['price_type']=='predefined',initialized=True,dirty=False)
        for tag,*_ in original.operations:
            if tag in (0,2,3):state['dirty']=False
            elif tag in (1,5):state['dirty']=True
        result.append(dict(record='case',name=name,policy=request.policy,primitive_operations=sum(op[5]if op[0]==5 else 1 for op in original.operations),allocation_attempts=1,requested_bytes=1508,owners_dropped=1,final_state=state))
    need(len(result)==12,'selected exact schedules');return result

def parse(data):
    need(type(data)is bytes and len(data)<1<<20,'bounded transcript')
    return [load(line)for line in data.splitlines()]

def check(records,expected,unsafe,timed):
    need(type(unsafe)is bool and type(timed)is bool,'configuration flags')
    first=dict(record='qualification',cases=298,transitions=4065,queries=2848,frequency_unsafe=unsafe,timed=timed)
    need(records and records[0]==first,'complete qualification header')
    index=1;samples=[]
    for case in expected:
        need(index<len(records)and records[index]==case,'complete independent case/state/owner record');index+=1
        if timed:
            for sample in range(9):
                need(index<len(records),'sample extent');row=records[index];index+=1
                need(type(row)is dict and set(row)=={'record','name','sample','iterations','nanoseconds'},'sample keys')
                count=8 if case['name'][6:]in ('fraction_boundary_30','counts_8192_m0')else 128
                need(row['record']=='sample'and row['name']==case['name']and type(row['sample'])is int and row['sample']==sample and type(row['iterations'])is int and row['iterations']==count,'sample identity/order/count')
                integer(row['nanoseconds'],1,60_000_000_000,'positive bounded elapsed duration');samples.append(row)
    need(index+1==len(records)and records[index]==dict(record='end',selected=12,samples=108 if timed else 0),'exact end/no extras')
    # Equality must not let bools impersonate numeric state or owner fields.
    canonical=json.dumps(records,sort_keys=True,separators=(',',':'))
    rebuilt=[first]
    for case in expected:
        rebuilt.append(case)
        if timed:rebuilt.extend(row for row in samples if row['name']==case['name'])
    rebuilt.append(dict(record='end',selected=12,samples=len(samples)))
    need(canonical==json.dumps(rebuilt,sort_keys=True,separators=(',',':')),'exact JSON scalar types')
    return samples

def controls(records,expected,unsafe,timed,out):
    need(not out.exists(),'fresh control output');out.mkdir();results=[]
    def reject(name,edit):
        changed=copy.deepcopy(records);edit(changed)
        try:check(changed,expected,unsafe,timed)
        except ValueError as error:diagnostic=str(error)
        else:raise ValueError('forgery accepted '+name)
        data=(json.dumps(changed,separators=(',',':'))+'\n').encode();(out/(name+'.json')).write_bytes(data);results.append(dict(name=name,sha256=sha(data),diagnostic=diagnostic))
    reject('qualification-count',lambda rows:rows[0].update(transitions=4064))
    reject('wrong-backend',lambda rows:rows[0].update(frequency_unsafe=not unsafe))
    reject('policy',lambda rows:rows[1].update(policy=2))
    reject('owner-allocation',lambda rows:rows[1].update(allocation_attempts=2))
    reject('owner-leak',lambda rows:rows[1].update(owners_dropped=0))
    reject('coherent-count',lambda rows:rows[1]['final_state']['counts'].__setitem__(0,1))
    reject('sum',lambda rows:rows[1]['final_state']['sums'].__setitem__(0,999))
    reject('cache',lambda rows:rows[1]['final_state']['base_prices'].__setitem__(0,999))
    reject('dirty',lambda rows:rows[1]['final_state'].update(dirty=True))
    reject('numeric-bool',lambda rows:rows[1].update(allocation_attempts=True))
    reject('extra',lambda rows:rows.append(rows[-1]))
    reject('missing',lambda rows:rows.pop())
    if timed:
        reject('sample-order',lambda rows:rows[2].update(sample=1))
        reject('iteration-count',lambda rows:rows[2].update(iterations=127))
        reject('zero-duration',lambda rows:rows[2].update(nanoseconds=0))
        reject('bool-duration',lambda rows:rows[2].update(nanoseconds=True))
    check(records,expected,unsafe,timed)
    report=dict(status='passed',optimized=not __debug__,rejected=len(results),records=results);(out/'summary.json').write_text(json.dumps(report,indent=2)+'\n');return report

def main():
    p=argparse.ArgumentParser();p.add_argument('transcript',type=Path);p.add_argument('--unsafe',action='store_true');p.add_argument('--timed',action='store_true');p.add_argument('--controls',type=Path);a=p.parse_args();expected=expectations();records=parse(a.transcript.read_bytes());samples=check(records,expected,a.unsafe,a.timed)
    if a.controls:controls(records,expected,a.unsafe,a.timed,a.controls)
    print(json.dumps(dict(status='passed',cases=12,samples=len(samples),optimized=not __debug__)))
if __name__=='__main__':main()
