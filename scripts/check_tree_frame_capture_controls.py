#!/usr/bin/env python3
"""Reject forged tree capture provenance and actual observations without children."""
from pathlib import Path
import argparse
import copy
import hashlib
import json
import sys
ROOT=Path(__file__).resolve().parents[1]
AUDITOR_SHA='32ab218ed5376831605569dacb58ad0586a8d47994629c4e089fe7f5601c10f1'
def check(directory):
    attempted=[]
    def hook(event,args):
        if event in {'subprocess.Popen','os.system','os.posix_spawn','os.fork'}:
            attempted.append(event);raise ValueError('offline controls forbid child execution')
    sys.addaudithook(hook)
    p=ROOT/'scripts/check_tree_frame_capture_v2.py'
    if hashlib.sha256(p.read_bytes()).hexdigest()!=AUDITOR_SHA:raise ValueError('frozen four-pass auditor')
    import check_tree_frame_capture_v2 as a
    c,_,_,v,g,cases=a.inputs();root=directory.resolve();original=a.read(root,'evidence.json',8<<20);e=v.parse_json(original);rejected=0
    def reject(action):
        nonlocal rejected
        try:action()
        except ValueError:rejected+=1
        else:raise ValueError('forged capture accepted')
    # These fail before expensive semantic replay and preserve real artifacts.
    changes=[(['status'],'failed'),(['schema_version'],True),(['normal_sanitized_equal'],False),(['reference_revision'],'0'*40),(['support_archive_used'],True),(['commands'],e['commands'][:-1]),(['source_after'],[])]
    real_read=a.read
    for keys,value in changes:
        forged=copy.deepcopy(e);target=forged
        for key in keys[:-1]:target=target[key]
        target[keys[-1]]=value;encoded=json.dumps(forged).encode()
        def virtual_read(r,name,cap=64<<20):
            if r==root and name=='evidence.json':return encoded
            return real_read(r,name,cap)
        try:a.read=virtual_read;reject(lambda:a.audit(root))
        finally:a.read=real_read
    coherent=[]
    forged=copy.deepcopy(e);forged['source_before'][0]['sha256']='0'*64;forged['source_after']=copy.deepcopy(forged['source_before']);coherent.append(forged)
    forged=copy.deepcopy(e);forged['commands'][1]['label']=forged['commands'][0]['label'];coherent.append(forged)
    forged=copy.deepcopy(e);row=next(r for r in forged['commands'] if r['label']=='normal-metadata');row['stdout']['sha256']='0'*64;coherent.append(forged)
    forged=copy.deepcopy(e);row=next(r for r in forged['commands'] if r['label']=='sanitized-compile-0');row['argv'].remove('-fsanitize=address,undefined');coherent.append(forged)
    forged=copy.deepcopy(e);forged['results']['sanitized']['instrumented_codec_sources']=[];coherent.append(forged)
    forged=copy.deepcopy(e);dependencies=forged['results']['normal']['dependencies'];dependencies[next(iter(dependencies))]='0'*64;coherent.append(forged)
    for forged in coherent:
        encoded=json.dumps(forged).encode()
        def virtual_read(r,name,cap=64<<20):
            if r==root and name=='evidence.json':return encoded
            return real_read(r,name,cap)
        try:a.read=virtual_read;reject(lambda:a.audit(root))
        finally:a.read=real_read
    case=next(c for c in cases if c.name=='bt_chain_first_uniform');suffix=case.name+'-p106'
    records=[v.parse_json(line) for line in a.read(root,'normal-'+suffix+'.stdout').splitlines()];frame=a.read(root,'frames-normal/'+suffix+'.zst');whole=a.read(root,'frames-normal/'+suffix+'.whole.zst')
    v.validate_case(case,records,frame,whole)
    bi=next(i for i,r in enumerate(records) if r['type']=='block');last=next(i for i,r in enumerate(records) if r['type']=='complete')
    changes=[([0,'applied',6],5),([0,'branch'],'chain'),([0,'source_bytes'],True),([0,'sampling','next_to_update'],3),([bi,'sampling_before','next_to_update'],3),([bi,'sampling_after','skipping'],2),([bi,'sampling_after','row_hash_log'],1),([bi,'sampling_after','salt_entropy'],1),([bi,'window','dict_limit'],3),([bi,'window','next_to_update'],3),([bi,'first_table','log'],20),([bi,'second_table','element_bytes'],1),([bi,'first_table','after_fnv64'],'0'*16),([bi,'second_table','changed'],[[0,0,1]]),([bi,'after','raw_repeats',0],0),([bi,'candidate','sequences',0,1],131072),([bi,'candidate','sequences',0,2],2),([bi,'candidate','sequences',0,3],0),([bi,'candidate','literal_bytes'],'01020304'),([last,'frame_bytes'],len(frame)+1)]
    for keys,value in changes:
        forged=copy.deepcopy(records);target=forged
        for key in keys[:-1]:target=target[key]
        target[keys[-1]]=value;reject(lambda:v.validate_case(case,forged,frame,whole))
    for broken in [frame[:-1],frame+b'x',bytes([frame[0]^1])+frame[1:]]:reject(lambda:v.validate_case(case,records,broken,whole))
    reject(lambda:v.validate_case(case,records,frame,whole[:-1]))
    metadata=v.parse_json(a.read(root,'normal-metadata.stdout').strip());v.validate_metadata(metadata)
    for key,value in [('strategies',[5]),('padding_values',[106]),('schema_version',True),('pointer_bits',True)]:reject(lambda:v.validate_metadata(metadata|{key:value}))
    a.require(not attempted and rejected==41,'complete offline forgery count')
    return dict(status='passed',provenance_rejections=13,actual_record_frame_rejections=24,metadata_rejections=4,rejections=rejected,child_attempts=0,auditor_sha256=AUDITOR_SHA,qualification='Unchanged on-disk capture; virtual malformed provenance and copied actual C observations rejected offline.')
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('directory',type=Path);args=p.parse_args();print(json.dumps(check(args.directory),sort_keys=True))
