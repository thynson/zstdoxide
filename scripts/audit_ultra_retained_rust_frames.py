#!/usr/bin/env python3
"""Replay private Rust tokens under actual wire kinds; no native child execution."""
import argparse,hashlib,json
from pathlib import Path
from oracles import ultra_retained_validation as model
from oracles.ultra_retained_recipes import recipes
from oracles.optimal_retained_frames import require
from oracles.fast_frame_publication_support import frame_blocks
from oracles import tree_validation as strict

def read(root,name,limit):
    p=root/name
    require(p.is_file()and not p.is_symlink()and p.resolve().is_relative_to(root),'contained regular artifact')
    require(p.stat().st_size<=limit,'bounded artifact')
    data=p.read_bytes();require(len(data)<=limit,'bounded read');return data

def audit(root):
    root=root.resolve(strict=True)
    rows=json.loads(read(root,'observations.json',8<<20),object_pairs_hook=strict.unique,parse_constant=strict.bad_constant)
    expected={n:r for n,r in recipes().items()if r.request.policy==2 and r.request.original.literal_mode==0}
    require(type(rows)is list and len(rows)==len(expected)==18,'exact automatic-literal case inventory')
    require(all(type(r)is dict and set(r)=={'name','blocks'}for r in rows),'exact case objects before field access')
    require([r['name']for r in rows]==list(expected),'exact ordered names')
    names={'observations.json'}|{n+'.'+suffix for n in expected for suffix in ('raw','zst')}
    require({p.name for p in root.iterdir()}==names,'exact capture file inventory')
    report=[];events=sequences=0;changed=0
    for row in rows:
        require(type(row)is dict and set(row)=={'name','blocks'},'case fields')
        recipe=expected[row['name']];tagged=recipe.request;request=tagged.original;source=request.source
        require(read(root,row['name']+'.raw',32768)==source,'independent source recipe')
        frame=read(root,row['name']+'.zst',32832)
        n=len(source);header=bytes.fromhex('28b52ffd')+(bytes((0x20,n))if n<256 else bytes((0x60,))+(n-256).to_bytes(2,'little'))
        require(frame.startswith(header),'exact source-aware frame header')
        wire=frame_blocks(frame);blocks=row['blocks']
        require(type(blocks)is list and len(blocks)==len(wire)==len(request.blocks),'exact balanced or observed block inventory')
        kinds=tuple(b['kind']for b in wire)
        require(all(b['last']==int(i+1==len(wire))for i,b in enumerate(wire)),'only last scheduled block terminates')
        predictions,coverage=model.records(tagged,kinds)
        begins=[r for r in predictions if r['record']=='block_begin'];ends=[r for r in predictions if r['record']=='block_publish']
        events+=sum(r['record']=='event'for r in predictions)
        for index,(actual,w,length,begin,end)in enumerate(zip(blocks,wire,request.blocks,begins,ends)):
            require(type(actual)is dict and set(actual)=={'start','length','kind','incoming','candidate','committed','tail','sequences','literals'},'block fields')
            start=begin['start'];source_block=source[start:start+length]
            final=next(r for r in reversed(predictions)if r['record']=='event'and r['block']==index)
            expected_sequences=final['sequences'];literals=[]
            for anchor,ll,ml,offset in expected_sequences:literals.extend(source[anchor:anchor+ll])
            literals.extend(source[start+length-end['tail']:start+length])
            expected_block=dict(start=start,length=length,kind=w['kind'],incoming=end['before_repeats'],candidate=end['candidate_repeats'],committed=end['owners'][end['previous_owner_after']],tail=end['tail'],sequences=expected_sequences,literals=literals)
            # Canonical JSON distinguishes bools from integers and exact array shapes.
            require(json.dumps(actual,sort_keys=True,separators=(',',':'))==json.dumps(expected_block,sort_keys=True,separators=(',',':')),'source-derived tokens and publication under actual wire kinds')
            sequences+=len(expected_sequences)
            if w['kind']==0:require(w['payload']==source_block,'exact raw source bytes')
            elif w['kind']==1:require(w['field']==length and w['payload']*length==source_block,'exact RLE source extent')
            else:require(0<w['field']<length,'beneficial bounded compressed block')
        changed+=kinds!=recipe.kinds
        report.append(dict(name=row['name'],kinds=kinds,native_hypothesis=recipe.kinds,frame_sha256=hashlib.sha256(frame).hexdigest(),source_sha256=hashlib.sha256(source).hexdigest(),coverage=sorted(coverage)))
    return dict(status='passed',cases=report,model_events=events,sequences=sequences,changed_native_histories=changed,child_processes=0,qualification='Independent immutable retained policy-2 integer model derives tokens, literals, tails and repeat publication from source and actual Rust wire kinds. Native matches or C wire hypotheses never provide parser input. This audit does not decode compressed payloads; native and Rust decode checks are separate.')

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('capture',type=Path);args=parser.parse_args()
    print(json.dumps(audit(args.capture),sort_keys=True,indent=2))
