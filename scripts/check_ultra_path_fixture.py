#!/usr/bin/env python3
"""Reconstruct complete path fixture bytes independently from source-only replay."""
from pathlib import Path
import argparse,json,struct,sys
sys.dont_write_bytecode=True
from oracles.ultra_path_recipes import recipes
from oracles.ultra_path_validation import Replay
from oracles.ultra_scalar import need,sha,load
ROOT=Path(__file__).resolve().parents[1]
PACK_SHA='b5f990623fcc0c09c3d55e86bffa4b3af1a7f9e2decab1a211aaffa70435a7f9'
STAGES=('begin','query','origin','seed','literal','relax','select','reverse_start','reverse','sequence','refresh','complete','alternative')
KEYS=('lit','ll','ml','of')

def reconstruct():
    output=bytearray(b'ZUPFX01\0'+struct.pack('<I',58));layouts=[]
    def u32(*values):output.extend(struct.pack('<'+'I'*len(values),*values))
    def maybe(values):
        u32(int(values is not None))
        if values is not None:u32(*values)
    for name,request in recipes().items():
        replay=Replay(request);replay.run();source=request.original.source;original=request.original;name_bytes=name.encode();case_start=len(output)
        u32(len(name_bytes),len(source),original.min_match,original.target,original.literal_mode,len(replay.cells),len(replay.events),len(original.scripts),request.policy);output+=name_bytes;output+=source
        for position,pairs in original.scripts:
            u32(position,len(pairs))
            for off,length in pairs:u32(off,length)
        counts=[0]*386;sequence_count=0;events=[]
        for event in replay.events:
            start=len(output);u32(STAGES.index(event['stage']),int(event['phase']=='reverse'),event['ip'],event['anchor'],*[0xffffffff if event[key]is None else event[key]for key in ('cur','last','tail')],*event['repeats']);maybe(event['stretch']);maybe(event['reverse']);maybe(event['query'])
            if event['query']is not None:
                u32(len(event['matches']))
                for off,length in event['matches']:u32(off,length)
            u32(int(event['alternative']is not None));alternative_at=None
            if event['alternative']is not None:alternative_at=len(output);output.extend(struct.pack('<12q',*event['alternative']))
            changes=event['cells']['changes'];path_at=len(output);u32(len(changes))
            for index,values in changes:u32(index,*values)
            values=[value for table in KEYS for value in event['frequencies'][table]]+[event['sums'][table]for table in KEYS]+[event['base_prices'][table]for table in KEYS]+[int(event['predefined'])];need(len(values)==386,'complete state width')
            changed=[(i,v)for i,v in enumerate(values)if v!=counts[i]];frequency_at=len(output);u32(len(changed))
            for index,value in changed:u32(index,value)
            counts=values;appended=event['sequences'][sequence_count:];u32(len(event['sequences']),len(appended))
            for row in appended:u32(*row)
            sequence_count=len(event['sequences']);events.append(dict(start=start,stage=event['stage'],alternative=alternative_at,path=path_at,frequency=frequency_at))
        layouts.append(dict(name=name,start=case_start,events=events))
    need(len(output)==8796312 and len(layouts)==58,'complete reconstruction extent');return bytes(output),layouts

def check(data,expected,digest=True):
    need(type(data)is bytes and len(data)==8796312,'exact fixture extent')
    if digest:need(sha(data)==PACK_SHA,'native fixture digest')
    need(data==expected,'complete independent path reconstruction')

def controls(data,expected,layouts,out):
    need(not out.exists(),'fresh controls');out.mkdir();rows=[]
    def reject(name,offset,removed,replacement):
        changed=data[:offset]+replacement+data[offset+removed:]
        try:check(changed,expected,False)
        except ValueError as error:diagnostic=str(error)
        else:raise ValueError('accepted forged pack '+name)
        row=dict(name=name,baseline_sha256=PACK_SHA,offset=offset,removed=removed,replacement_hex=replacement.hex(),bytes=len(changed),sha256=sha(changed),diagnostic=diagnostic)
        p=out/(name+'.json');p.write_text(json.dumps(row,indent=2)+'\n');record=load(p.read_bytes());recovered=data[:record['offset']]+bytes.fromhex(record['replacement_hex'])+data[record['offset']+record['removed']:];need(recovered==changed and sha(recovered)==record['sha256'],'lossless reconstructed forgery');rows.append(row)
    def word(name,offset,value):reject(name,offset,4,struct.pack('<I',value))
    reject('truncated',len(data)-1,1,b'');reject('trailing',len(data),0,b'!');reject('magic',0,1,b'!')
    word('case-count-zero',8,0);word('case-count-large',8,59);word('source-cap',16,8193);word('unknown-policy',44,1);word('valid-wrong-policy',44,2)
    chosen=next(c for c in layouts if c['name']=='up_p2_ultra_alternative_seed_0');event=next(e for e in chosen['events']if e['alternative']is not None)
    word('alternative-stage',event['start'],4);word('alternative-presence',event['alternative']-4,0)
    for column in range(12):
        at=event['alternative']+8*column;value=struct.unpack_from('<q',data,at)[0];reject('alternative-operand-'+str(column),at,8,struct.pack('<q',value+1))
    next_event=chosen['events'][chosen['events'].index(event)+1];word('next-frontier',next_event['start']+20,0)
    word('invalid-path-index',event['path']+4,4099)
    check(data,expected);report=dict(status='passed',optimized=not __debug__,rejected=len(rows),root_digest_bypassed=True,lossless_deltas=True,records=rows);(out/'summary.json').write_text(json.dumps(report,indent=2)+'\n');return len(rows)

def main():
    p=argparse.ArgumentParser();p.add_argument('--controls',type=Path,required=True);a=p.parse_args();data=(ROOT/'tests/fixtures/reference-ultra-paths/cases.bin').read_bytes();expected,layouts=reconstruct();check(data,expected);n=controls(data,expected,layouts,a.controls);print(json.dumps(dict(status='passed',optimized=not __debug__,bytes=len(data),sha256=sha(data),rejected=n)))
if __name__=='__main__':main()
