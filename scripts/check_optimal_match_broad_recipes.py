#!/usr/bin/env python3
"""Freeze and validate broader source-only match requests without children."""
import argparse
from dataclasses import replace
import json
from pathlib import Path
import struct
import sys
from oracles import optimal_match_broad_recipes as recipes
from oracles.optimal_matches_broad import Request, MAX_PACKET, parse_packet, require
ROOT=Path(__file__).resolve().parents[1]
DEST=ROOT/'tests/fixtures/reference-optimal-match-broad-recipes/recipes.json'


def check(write=False):
    children=[]
    def guard(event,args):
        if event in ('subprocess.Popen','os.system','os.posix_spawn','os.fork'):
            children.append(event);raise ValueError('children forbidden')
    sys.addaudithook(guard)
    values=recipes.recipes();manifest=recipes.manifest()
    require((manifest['recipe_count'],manifest['source_bytes'],manifest['queries'])==(37,2119568,82),'inventory')
    for value in values:require(parse_packet(value.request.packet())==value.request,'exact source/query round trip')
    require(len(values[-1].request.packet())==MAX_PACKET,'maximum packet')
    require({v.request.hash3_log for v in values}=={0,10,14,17},'ordinary short-key logs')
    base=Request(b'abcdefgh',((0,0,0,0,0,4),))
    require(parse_packet(base.packet())==base,'minimal request')
    rejected=[]
    def reject(name,action):
        try:action()
        except ValueError:rejected.append(name)
        else:raise ValueError('accepted '+name)
    for field,low,high in [('window_log',10,21),('hash_log',6,12),('chain_log',6,12),('search_log',1,8),('min_match',3,7),('target',0,131072)]:
        for value in (low-1,high+1,True,float(low)):
            reject(field,lambda f=field,v=value:replace(base,**{f:v}).validate())
    for value in (b'1234567',b'x'*524289,bytearray(b'abcdefgh')):
        reject('source',lambda v=value:replace(base,source=v).validate())
    for value in ((),[],base.queries*17,base.queries*2,((0,0,0,0,0),)):
        reject('query shape/count',lambda v=value:replace(base,queries=v).validate())
    for index,values in enumerate(((-1,1,True,0.0),(-1,3,True,0.0),(-1,3,True,0.0),(-1,3,True,0.0),(-1,2,True,0.0),(3,9,True,4.0))):
        for value in values:
            words=list(base.queries[0]);words[index]=value
            reject('query field',lambda q=tuple(words):replace(base,queries=(q,)).validate())
    for w in (10,14,17,18,19,21):
        for h3 in (min(w,17)-1,min(w,17)+1,True):
            reject('ordinary h3',lambda w=w,h3=h3:replace(base,window_log=w,min_match=3,hash3_log=h3,queries=((0,0,0,0,0,3),)).validate())
    reject('absent h3',lambda:replace(base,hash3_log=10).validate())
    packet=base.packet()
    for length in range(len(packet)):reject('truncation',lambda n=length:parse_packet(packet[:n]))
    for data in (packet+b'!',b'WRONG!!!'+packet[8:],bytearray(packet),b'\0'*(MAX_PACKET+1)):
        reject('packet closure',lambda data=data:parse_packet(data))
    for word,value in [(0,524289),(1,17),(2,22),(3,13),(4,13),(5,9),(6,8),(7,131073),(8,10)]:
        p=bytearray(packet);struct.pack_into('<I',p,8+word*4,value)
        reject('wire header',lambda p=bytes(p):parse_packet(p))
    canonical=(json.dumps(manifest,sort_keys=True,indent=2)+'\n').encode()
    if write:
        require(not DEST.exists(),'fresh manifest');DEST.parent.mkdir(parents=True,exist_ok=True);DEST.write_bytes(canonical)
    require(DEST.is_file() and not DEST.is_symlink() and DEST.read_bytes()==canonical,'frozen manifest')
    require(not children,'no child attempts')
    print(json.dumps(dict(status='passed',recipes=37,source_bytes=2119568,queries=82,malformed_rejections=len(rejected),child_attempts=0),indent=2))


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--write',action='store_true');check(parser.parse_args().write)
