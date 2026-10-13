#!/usr/bin/env python3
"""Offline public tree recipe, transport and controller rejection gates."""
from pathlib import Path
import argparse
import ast
import copy
import hashlib
import json
import sys
import tempfile
from types import SimpleNamespace
ROOT=Path(__file__).resolve().parents[1]
def need(v,why):
    if not v:raise ValueError(why)
def main():
    child=[]
    def audit(event,args):
        if event in ('subprocess.Popen','os.system','os.posix_spawn','os.fork','os.exec'):
            child.append(event);raise RuntimeError('offline gate attempted child creation')
    sys.addaudithook(audit)
    import check_tree_interop_recipes as checker
    import capture_tree_interop as controller
    import check_tree_interop_capture as reader
    from oracles import greedy_interop_protocol as protocol
    for name in ['scripts/capture_tree_interop.py','scripts/check_tree_interop_capture.py']:
        ast.parse((ROOT/name).read_bytes())
    m=checker.check();_,_,_,plan=controller.inputs();need(plan==m,'same frozen source plan')
    raw=checker.MANIFEST.read_bytes();rejected=0
    def rejects(fn):
        nonlocal rejected
        try:fn()
        except (ValueError,KeyError,FileNotFoundError):rejected+=1
        else:raise ValueError('forged preflight accepted')
    with tempfile.TemporaryDirectory(prefix='tree-interop-preflight-') as tmp:
        base=Path(tmp)
        mutations=[b'',raw[:-1],b'[]',b'{"x":1,"x":2}',b'{"x":NaN}']
        for key,value in [('recipe_count',1196),('source_bytes',145529305),('classifications',{}),('resolved_tuples',[]),('recipes',[]),('input_sha256',{}),('expected_rust_frame_equivalence',[])]:
            forged=copy.deepcopy(m);forged[key]=value;mutations.append(json.dumps(forged).encode())
        for i,b in enumerate(mutations):
            p=base/str(i);p.write_bytes(b);rejects(lambda:checker.check(p))
        link=base/'link';link.symlink_to(checker.MANIFEST);rejects(lambda:checker.check(link))
        rejects(lambda:checker.check(base))
        rejects(lambda:checker.check(base/'missing-manifest'))
        rejects(lambda:controller.execute(SimpleNamespace(upstream=ROOT,output=base/'new',cc='cc')))
        rejects(lambda:controller.execute(SimpleNamespace(upstream=base,output=base,cc='cc')))
        rejects(lambda:reader.check(base))
    packets=protocol.negative_packets();need(len(packets)==39 and len({n for n,_ in packets})==39,'frozen malformed transport inventory')
    need(not child and rejected==18,'complete no-child rejection coverage')
    print(json.dumps(dict(status='passed',recipes=1197,planned_commands=13606,rejected=rejected,malformed_packets=39,child_attempts=len(child)),sort_keys=True))
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',required=True,action='store_true');p.parse_args();main()
