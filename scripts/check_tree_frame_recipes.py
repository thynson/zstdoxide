#!/usr/bin/env python3
"""Freeze/check source-only ordinary tree frame recipes and malformed controls."""
from pathlib import Path
from dataclasses import replace
import argparse
import json
from oracles import tree_frames as tree
ROOT=Path(__file__).resolve().parents[1]
DEST=ROOT/'tests/fixtures/reference-tree-frame-recipes/recipes.json'
def check(write=False):
    recipes=tree.recipes();value=tree.manifest()
    tree.require(value['recipe_count']==245 and value['source_bytes']==7640096 and value['scheduled_calls']==539 and value['positive_calls']==531 and value['whole_comparisons']==120,'frozen complete inventory')
    canonical=(json.dumps(value,sort_keys=True,indent=2)+'\n').encode()
    if write:
        tree.require(not DEST.exists(),'fresh recipe manifest');DEST.parent.mkdir(parents=True,exist_ok=True);DEST.write_bytes(canonical)
    tree.require(DEST.is_file() and not DEST.is_symlink() and DEST.read_bytes()==canonical,'exact recipe manifest')
    seed=recipes[-1];bad=[dict(window_log=9),dict(window_log=22),dict(hash_log=5),dict(hash_log=20),dict(chain_log=5),dict(chain_log=20),dict(search_log=0),dict(search_log=9),dict(min_match=2),dict(min_match=8),dict(target=-1),dict(target=131073),dict(window_log=True),dict(search_log=4.0),dict(schedule=(True,1023)),dict(schedule=(1023,)),dict(schedule=(0,)*65+(1024,)),dict(source=bytearray(seed.source)),dict(name='bt_BAD'),dict(name='x'*64),dict(whole_compare=1),dict(schedule=(512,512),whole_compare=True)]
    for change in bad:
        try:replace(seed,**change).metadata()
        except ValueError:pass
        else:raise ValueError('accepted malformed recipe '+repr(change))
    tree.require(all(r.metadata()['selected_branch']=='tree' and r.metadata()['parameters'][-1]==6 for r in recipes),'tree-only branch binding')
    print(f'Tree ordinary recipes: {len(recipes)} sources, {len(bad)} malformed controls rejected')
    return value
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--write',action='store_true');a=p.parse_args();check(a.write)
