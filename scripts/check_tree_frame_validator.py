#!/usr/bin/env python3
"""Qualify tree frame structural replay before executing C; no synthetic fixture."""
from pathlib import Path
from dataclasses import fields
import copy
import hashlib
import json
import sys
ROOT=Path(__file__).resolve().parents[1]
PINS={'scripts/check_greedy_frame_validator.py': 'ac39ce8dc61cae6e2f08e2eb34a4fecbecf0d35c5648edfcbbba75aa9db8186c', 'scripts/oracles/greedy_frame_validation.py': '8e400ccad307b562dd6a41ab02c5d00359d36431cd38a5c51040e08e3903c71f', 'scripts/oracles/tree_frames.py': '7a4bdbc669c8d0a512870fd6198ae94e780af3f7540e265cea42c84cd1d28353', 'scripts/oracles/tree_frame_validation.py': '91bb1f8e4c0ea00ec096788287892dd86da5a03758465eed9ee0a6b16806e51f'}
def check():
    children=[]
    def audit(event,args):
        if event in {'subprocess.Popen','os.system','os.posix_spawn','os.fork'}:
            children.append(event);raise ValueError('child execution forbidden')
    sys.addaudithook(audit)
    for name,digest in PINS.items():
        p=ROOT/name
        if not p.is_file() or p.is_symlink() or hashlib.sha256(p.read_bytes()).hexdigest()!=digest:raise ValueError('frozen dependency '+name)
    import check_greedy_frame_validator as old
    from oracles import tree_frames as g,tree_frame_validation as v
    rejected=0;positives=0
    def reject(action):
        nonlocal rejected
        try:action()
        except ValueError:rejected+=1
        else:raise ValueError('forged control accepted')
    for length in [1,7,32]:
        prior,records,frame=old.synthetic(False,length)
        args={f.name:getattr(prior,f.name) for f in fields(g.Recipe)};args['name']='bt_'+prior.name
        recipe=g.Recipe(**args)
        for record in records:record['case']=recipe.name
        records[0]['applied']=recipe.metadata()['parameters'];records[0]['branch']='tree'
        v.validate_case(recipe,records,frame);positives+=1
        changes=[([0,'applied',6],5),([0,'branch'],'chain'),([0,'source_bytes'],True),([0,'sampling','next_to_update'],3),([3,'sampling_before','next_to_update'],3),([3,'sampling_after','skipping'],2),([3,'sampling_after','row_hash_log'],1),([3,'sampling_after','salt_entropy'],1),([3,'window','dict_limit'],3),([3,'window','next_to_update'],3),([3,'first_table','log'],20),([3,'second_table','element_bytes'],1),([3,'first_table','after_fnv64'],'0'*16),([3,'second_table','changed'],[[1,0,1]]),([3,'after','raw_repeats',0],0),([3,'prev_owner_after'],1),([4,'produced'],1),([5,'frame_bytes'],len(frame)+1)]
        if length==32:
            changes += [([3,'candidate','sequences',0,0],True),([3,'candidate','sequences',0,1],131072),([3,'candidate','sequences',0,2],2),([3,'candidate','sequences',0,3],5),([3,'candidate','sequences',0,4],5),([3,'candidate','sequences',0,5],24),([3,'candidate','literal_bytes'],'01020304'),([3,'candidate','trailing'],1),([3,'changed_candidate_first_two'],False)]
        for keys,value in changes:
            forged=copy.deepcopy(records);target=forged
            for key in keys[:-1]:target=target[key]
            target[keys[-1]]=value;reject(lambda:v.validate_case(recipe,forged,frame))
        reject(lambda:v.validate_case(recipe,records,frame[:-1]));reject(lambda:v.validate_case(recipe,records,frame+b'x'))
    # These primitive controls verify storage interpretation, not tree selection.
    tables=[[0]*64,[0]*64];state={'next_to_update':8};tables[1][1]=1
    v.validate_tables(tables,recipe,32,state);positives+=1
    for owner,slot,value in [(0,0,1),(1,0,1),(1,1,True),(1,1,8),(1,1,999999999)]:
        forged=copy.deepcopy(tables);forged[owner][slot]=value;reject(lambda:v.validate_tables(forged,recipe,32,state))
    metadata={'type':'metadata','revision':g.REVISION,'protocol':v.PROTOCOL,'schema_version':1,'strategies':[6],'padding_values':[106,195],'pointer_bits':64,'int_bits':32,'u32_bytes':4,'source_cap':4<<20,'block_cap':131072,'call_cap':64,'leading_bytes':64,'suffix_bytes':262656,'guard_bytes':64,'scratch_bytes':2<<20}
    v.validate_metadata(metadata)
    for key,value in [('strategies',[5]),('padding_values',[195,106]),('schema_version',True),('protocol','lazy-family-ordinary-v1'),('pointer_bits',True),('source_cap',1<<30)]:reject(lambda:v.validate_metadata(metadata|{key:value}))
    v.need(not children,'offline-only replay')
    return dict(status='passed',synthetic_positives=positives,metadata_positives=1,rejections=rejected,child_attempts=len(children),qualification='Structural synthetic replay and marker-domain controls, not algorithm expectations or C observations.')
if __name__=='__main__':print(json.dumps(check(),sort_keys=True))
