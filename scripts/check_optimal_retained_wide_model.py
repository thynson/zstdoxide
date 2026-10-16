#!/usr/bin/env python3
"""Reconcile sparse replay with every captured small state and reject forged deltas."""
import dataclasses
import hashlib
import io
import json
from pathlib import Path
import struct
import sys
import tarfile
from oracles.optimal_retained_wide import Request,parse_packet,require
from oracles.optimal_retained_recipes_v2 import recipes
from oracles.optimal_retained_wide_validation import observations,validate_output,delta,flattened,PATH_CELLS
from oracles.tree_validation import parse,same
ROOT=Path(__file__).resolve().parents[1]


def check():
    children=[]
    def guard(event,args):
        if event in ('subprocess.Popen','os.system','os.posix_spawn','os.fork'):
            children.append(event);raise ValueError('child execution forbidden')
    sys.addaudithook(guard)
    report=parse((ROOT/'docs/evidence/2026-10-16-optimal-retained-capture.json').read_bytes())
    archive=ROOT/'docs/evidence'/report['archive']
    require(archive.stat().st_size==report['archive_bytes'] and hashlib.sha256(archive.read_bytes()).hexdigest()==report['archive_sha256'],'immutable capture archive')
    inventory={r['path']:r for r in report['files']};captured={}
    with tarfile.open(archive) as tar:
        members=tar.getmembers()
        require(len(members)==len(inventory) and {m.name for m in members}==set(inventory),'exact capture inventory')
        for member in members:
            require(member.isfile(),'regular capture member')
            data=tar.extractfile(member).read();row=inventory[member.name]
            require(len(data)==row['bytes'] and hashlib.sha256(data).hexdigest()==row['sha256'],'exact capture member')
            if member.name.startswith('logs/normal-run-retained_') and member.name.endswith('.stdout'):captured[member.name]=data
    event_count=0
    for name,recipe in recipes().items():
        request=Request(**dataclasses.asdict(recipe.request));require(parse_packet(request.packet())==request,'wider packet round trip')
        old=io.BytesIO(captured['logs/normal-run-'+name+'.stdout'])
        paths=None;search=None
        for expected in observations(request,recipe.kinds):
            row=parse(old.readline())
            if row['record']=='metadata':
                row.update(protocol='ZOPRW02',path_cells=PATH_CELLS,state_encoding='canonical-deltas-from-zero')
            elif row['record']=='block_begin' and row['block']==0:
                paths=[[0]*7 for _ in range(PATH_CELLS)]
                search=[0]*((1<<request.hash_log)+(1<<request.chain_log)+(1<<request.hash3_log if request.hash3_log else 0)+2)
            elif row['record']=='event':
                require(len(row['cells'])==515 and row['path_tail_zero'] is True,'captured unused path extent')
                row['cells']=delta(paths,row['cells']+[[0]*7 for _ in range(PATH_CELLS-515)],True)
                if row['search_before'] is not None:row['search_before']=delta(search,flattened(row['search_before']),False)
                row['search']=delta(search,flattened(row['search']),True)
                event_count+=1
            same(row,expected,'lossless existing native complete-state projection')
        require(old.read(1)==b'','existing native EOF')
    rejected=[]
    def reject(name,call):
        try:call()
        except (ValueError,TypeError):rejected.append(name)
        else:raise ValueError('accepted forgery '+name)
    request=Request(b'a'*128,(64,64));packet=request.packet();kinds=(2,1)
    for name,data in [('empty',b''),('header',packet[:43]),('truncated',packet[:-1]),('extra',packet+b'!'),('magic',b'!'+packet[1:])]:
        reject(name,lambda d=data:parse_packet(d))
    for at,value in [(8,16385),(12,1),(12,5),(16,9),(16,17),(20,5),(20,13),(24,5),(24,13),(28,0),(28,7),(32,2),(32,8),(36,4096),(40,2),(44,63),(44,8193)]:
        data=packet[:at]+struct.pack('<I',value)+packet[at+4:]
        reject('packet-'+str(at)+'-'+str(value),lambda d=data:parse_packet(d))
    for field,value in [('source',bytearray(request.source)),('blocks',[64,64]),('blocks',(64,True)),('target',True)]:
        reject('type-'+field,lambda f=field,v=value:dataclasses.replace(request,**{f:v}).validate())
    reject('window-expiry',lambda:Request(b'a'*2048,(1024,1024),window_log=10).validate())
    good=b''.join((json.dumps(r,sort_keys=True,separators=(',',':'))+'\n').encode() for r in observations(request,kinds))
    validate_output(good,request,kinds)
    for kind in ['before-owner','after-owner','candidate','committed','kind','start','query','before-search','search-index','search-value','search-length','path-index','path-value','path-length','path-duplicate','frequency','phase','padding','sequence','bool']:
        rows=[parse(line) for line in good.splitlines()]
        pub=next(r for r in rows if r['record']=='block_publish' and r['block']==1)
        event=next(r for r in rows if r.get('stage')=='query' and r['block']==1)
        path=next(r for r in rows if r.get('stage')=='origin')
        if kind=='before-owner':pub['previous_owner_before']^=1
        elif kind=='after-owner':pub['previous_owner_after']^=1
        elif kind=='candidate':pub['candidate_repeats'][0]+=1
        elif kind=='committed':pub['owners'][pub['previous_owner_after']][0]+=1
        elif kind=='kind':pub['kind']=2
        elif kind=='start':next(r for r in rows if r['record']=='block_begin' and r['block']==1)['start']+=1
        elif kind=='query':event['query'][0]+=1
        elif kind=='before-search':event['search_before']['changes']=[[0,2]]
        elif kind=='search-index':event['search']['changes'][0][0]=0xffffffff
        elif kind=='search-value':event['search']['changes'][0][1]+=1
        elif kind=='search-length':event['search']['length']+=1
        elif kind=='path-index':path['cells']['changes'][0][0]=4099
        elif kind=='path-value':path['cells']['changes'][0][1][0]+=1
        elif kind=='path-length':path['cells']['length']=515
        elif kind=='path-duplicate':path['cells']['changes']*=2
        elif kind=='frequency':event['frequencies']['ll'][0]+=1
        elif kind=='phase':event['phase']='reverse'
        elif kind=='padding':event['padding_pass']=1
        elif kind=='sequence':next(r for r in rows if r.get('stage')=='sequence')['sequences'][0][3]+=1
        else:event['query'][0]=True
        data=b''.join((json.dumps(r)+'\n').encode() for r in rows)
        reject(kind,lambda d=data:validate_output(d,request,kinds))
    for name,data in [('no-newline',good[:-1]),('extra-record',good+b'{}\n'),('no-trailer',b'\n'.join(good.splitlines()[:-1])+b'\n')]:
        reject(name,lambda d=data:validate_output(d,request,kinds))
    require(not children,'no children')
    return dict(status='passed',recipes=16,captured_events=event_count,rejected=len(rejected),controls=rejected,child_processes=0,
        qualification='Lossless complete-state projection of both padding passes of all sixteen immutable normal C captures; wider source bounds remain source-only until new capture.')


if __name__=='__main__':print(json.dumps(check(),sort_keys=True))
