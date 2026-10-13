#!/usr/bin/env python3
"""Qualify ordinary Lazy replay with synthetic records and frozen predecessors.

Synthetic records test structural replay, never depth-specific parser behavior.
No codec or child process executes, and no synthetic result becomes a fixture.
"""
from pathlib import Path
from dataclasses import fields, replace
import argparse
import copy
import hashlib
import json
import sys

ROOT=Path(__file__).resolve().parents[1]
PINS={
 'scripts/check_greedy_frame_validator.py':'ac39ce8dc61cae6e2f08e2eb34a4fecbecf0d35c5648edfcbbba75aa9db8186c',
 'scripts/check_greedy_frames.py':'3c82513af6e9ab804b40be9b37781b6849f5b0eb3031edbfcc08f340d5f36e2c',
 'scripts/check_lazy_frame_recipes.py':'0a6ac1e5b21a82582048c722951fdaf1b52faf012cab2f65416ee6b766724db0',
 'scripts/oracles/lazy_frame_validation.py':'b57f0c166b0c2ce5a4ff1f3c87e87eea969d25bdac93f264073d2545d60d11f3',
}


def check():
    children=[]
    def audit(event,args):
        if event in {'subprocess.Popen','os.system','os.posix_spawn','os.fork'}:
            children.append(event)
            raise ValueError('child execution forbidden')
    sys.addaudithook(audit)
    for name,digest in PINS.items():
        path=ROOT/name
        if not path.is_file() or path.is_symlink() or path.stat().st_size>128<<10 or hashlib.sha256(path.read_bytes()).hexdigest()!=digest:
            raise ValueError('frozen validation dependency: '+name)
    import check_lazy_frame_recipes as inputs
    import check_greedy_frame_validator as predecessor
    import check_greedy_frames as fixture
    from oracles import lazy_frame_validation as v
    generator,_=inputs.inputs();inputs.check()
    inherited=predecessor.check()
    manifest,artifacts=fixture.check()
    recipes={r.name:r for r in generator.recipes()}
    predecessors=0
    for old in manifest['cases']:
        name=old['id'];recipe=recipes['d0_'+name]
        records=[v.parse_json(line) for line in artifacts[name+'.stdout'].splitlines()]
        projected=copy.deepcopy(records)
        for record in projected:record['case']=recipe.name
        frame=artifacts[name+'.zst'];whole=artifacts.get(name+'.whole.zst')
        v.validate_case(recipe,projected,frame,whole)
        v.need(v.predecessor_records(projected,name)==records,'lossless predecessor normalization')
        predecessors+=1
    rejected=0;positives=0
    def reject(action):
        nonlocal rejected
        try:action()
        except ValueError:rejected+=1
        else:raise ValueError('forged control accepted')
    for depth in range(3):
        for row in [False,True]:
            for length in [1,7,32]:
                old,records,frame=predecessor.synthetic(row,length)
                values={f.name:getattr(old,f.name) for f in fields(generator.base.Recipe)}
                values['name']=f'd{depth}_'+old.name
                recipe=generator.Recipe(**values,depth=depth)
                for record in records:record['case']=recipe.name
                records[0]['applied']=recipe.metadata()['parameters']
                v.validate_case(recipe,records,frame);positives+=1
                for keys,value in [([0,'applied',6],3+(depth+1)%3),([0,'applied',6],True),
                                   ([0,'case'],f'd{(depth+1)%3}_'+old.name),
                                   ([0,'sampling','next_to_update'],3),
                                   ([3,'sampling_before','next_to_update'],3),
                                   ([3,'sampling_after','skipping'],2),
                                   ([3,'sampling_after','salt_entropy'],1<<32),
                                   ([3,'window','consumed'],length+1),
                                   ([3,'first_table','after_fnv64'],'0'*16),
                                   ([3,'second_table','element_bytes'],4 if row else 1),
                                   ([3,'after','raw_repeats',0],0),
                                   ([3,'prev_owner_after'],1),([4,'produced'],1),
                                   ([5,'frame_bytes'],len(frame)+1)]:
                    forged=copy.deepcopy(records);target=forged
                    for key in keys[:-1]:target=target[key]
                    target[keys[-1]]=value
                    reject(lambda:v.validate_case(recipe,forged,frame))
                reject(lambda:v.validate_case(recipe,records,frame[:-1]))
                reject(lambda:v.validate_case(recipe,records,frame+b'x'))
                reject(lambda:v.validate_case(replace(recipe,depth=(depth+1)%3),records,frame))
                reject(lambda:v.predecessor_records(records,'different'))
    metadata={'type':'metadata','revision':generator.REVISION,'protocol':v.PROTOCOL,'schema_version':1,
              'depths':[0,1,2],'strategies':[3,4,5],'pointer_bits':64,'int_bits':32,'u32_bytes':4,
              'source_cap':4<<20,'block_cap':131072,'call_cap':64,'leading_bytes':64,
              'suffix_bytes':262656,'guard_bytes':64,'scratch_bytes':2<<20}
    v.validate_metadata(metadata)
    for key,value in [('depths',[False,1,2]),('depths',[0,1]),('depths',[0,2,1]),
                      ('strategies',[3,5,4]),('schema_version',True),('protocol','greedy'),
                      ('pointer_bits',True),('source_cap',1<<30)]:
        reject(lambda:v.validate_metadata(metadata|{key:value}))
    v.need(predecessors==138 and positives==18 and not children,'coverage counts')
    return {'status':'passed','predecessors':predecessors,'synthetic_positives':positives,
            'metadata_positives':1,'new_rejections':rejected,'inherited':inherited,'child_attempts':len(children),
            'qualification':'No new C execution or depth-specific parser expectation; synthetic schema replay plus immutable depth-zero predecessor records.'}


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',required=True,action='store_true');p.parse_args()
    print(json.dumps(check(),sort_keys=True))


if __name__=='__main__':main()
