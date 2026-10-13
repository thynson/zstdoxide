#!/usr/bin/env python3
"""Pre-execution qualification of tree capture scope and offline auditor gates."""
from pathlib import Path
import copy
import hashlib
import json
import sys
ROOT=Path(__file__).resolve().parents[1]
def check():
    children=[]
    def no_children(event,args):
        if event in {'subprocess.Popen','os.system','os.posix_spawn','os.fork'}:
            children.append(event);raise ValueError('preflight child execution forbidden')
    sys.addaudithook(no_children)
    import capture_tree_frames as c
    import check_tree_frame_capture as a
    plan=c.plan()
    expected=dict(recipes=245,source_bytes=7640096,scheduled_calls=539,positive_calls=531,whole_comparisons=120,incomplete_headers=15,malformed_per_build=44,padding_passes=[106,195],expected_commands=1162,codec_executions=0,status='pre-execution plan checked')
    c.require(a.exact(plan,expected),'complete reviewed plan')
    c.require(hashlib.sha256((ROOT/a.CONTROLLER).read_bytes()).hexdigest()==a.CONTROLLER_SHA,'auditor controller binding')
    capture,_,_,v,g,cases=a.inputs();c.require(capture is c and len(cases)==245,'auditor input closure')
    negatives=c.malformed(Path('/planned-inputs'),Path('/planned-frames'))
    c.require(len(negatives)==44 and len({n for n,_,_ in negatives})==44,'unique negative commands')
    for name,argv,output in negatives:
        c.require(type(name) is str and all(type(x) is str for x in argv),'argument vector types')
        if output is not None:c.require(output.parent==Path('/planned-frames') and str(output) in argv,'bounded negative output')
    rejects=0
    def reject(action):
        nonlocal rejects
        try:action()
        except ValueError:rejects+=1
        else:raise ValueError('preflight forged control accepted')
    for path in ['/etc/passwd','../Cargo.toml','a/../../Cargo.toml','a\\b','./Cargo.toml']:
        reject(lambda:a.read(ROOT,path))
    pins=c.PINS
    try:
        c.PINS=copy.copy(pins);c.PINS['scripts/oracles/tree_frame_v1.c']='0'*64
        reject(c.load_inputs)
    finally:c.PINS=pins
    for change in [dict(recipes=True),dict(expected_commands=1161),dict(padding_passes=[106]),dict(codec_executions=1)]:
        reject(lambda:c.require(a.exact(plan,expected|change),'forged plan'))
    c.require(not children and rejects==10,'offline preflight coverage')
    return dict(status='passed',plan=plan,forged_rejections=rejects,child_attempts=0,qualification='No C execution; pinned inputs, command counts, malformed inventory, path bounds and altered-pin/plan controls.')
if __name__=='__main__':print(json.dumps(check(),sort_keys=True))
