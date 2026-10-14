#!/usr/bin/env python3
"""Source-only controls for fresh ordinary startup composition; no subprocesses."""
import dataclasses
import json
import struct
import subprocess
from oracles.optimal_startup import Request,parse_packet,require
from oracles.optimal_startup_recipes import recipes
from oracles.optimal_startup_validation import Replay,encoded,validate_output


def main():
    children=[]
    def blocked(*args,**kwargs):children.append(1);raise ValueError('no child processes in source-only controls')
    original=subprocess.Popen;subprocess.Popen=blocked
    rejected=0
    def reject(fn):
        nonlocal rejected
        try:fn()
        except(ValueError,TypeError):rejected+=1
        else:raise RuntimeError('invalid input accepted')
    try:
        cases=recipes();coverage=set();phases=set();rows=[]
        for name,request in cases.items():
            require(parse_packet(request.packet())==request,'packet round trip')
            replay=Replay(request);events=replay.run();coverage.update(replay.coverage);phases.update(replay.search_phases)
            queries=[e for e in events if e['stage']=='query']
            require(queries and queries[0]['query'][0:4]==[1,1,4,8],'actual fresh query state')
            require(events[-1]['repeats']==list(replay.wire_repeats),'final committed repeats')
            rows.append(dict(name=name,events=len(events),queries=len(queries),sequences=len(replay.sequences),coverage=sorted(replay.coverage),search_phases=sorted(replay.search_phases)))
        require('fresh_unavailable_repeats'in coverage and 'tree'in phases and 'repeat'in phases,'startup eligibility and ordinary search branches')
        base=cases['startup_uniform'];packet=base.packet()
        for key,value in [('source',bytearray(base.source)),('source',b'a'*63),('source',b'a'*513),('window_log',9),('window_log',13),('hash_log',5),('hash_log',10),('chain_log',5),('chain_log',10),('search_log',0),('search_log',5),('min_match',2),('min_match',8),('target',4096),('target',True),('literal_mode',2)]:reject(lambda k=key,v=value:dataclasses.replace(base,**{k:v}).validate())
        for data in [b'',packet[:39],packet[:-1],packet+b'!',b'!'+packet[1:],packet[:8]+struct.pack('<I',512)+packet[12:]]:reject(lambda d=data:parse_packet(d))
        good=encoded(base);validate_output(good,base);lines=good.splitlines()
        for field in ['price','repeat','frontier','head','node','short','query','phase','frequency','sequence']:
            records=[json.loads(line)for line in lines];query=next(r for r in records if r.get('stage')=='query');event=next(r for r in records if r.get('stage')=='sequence')
            if field=='price':event['cells'][0][0]+=1
            elif field=='repeat':event['repeats'][0]+=1
            elif field=='frontier':query['search']['frontier']+=1
            elif field=='head':query['search']['heads'][0]=999
            elif field=='node':query['search']['nodes'][0][0]=999
            elif field=='short':query['search']['short_heads']=[0]
            elif field=='query':query['query'][3]=7
            elif field=='phase':query['phase']='reverse'
            elif field=='frequency':event['sums']['ml']+=1
            elif field=='sequence':event['sequences'][0][2]-=1
            forged=b''.join((json.dumps(r)+'\n').encode()for r in records);reject(lambda d=forged:validate_output(d,base))
        for data in [good[:-1],good+b'{}\n',b'\n'.join(lines[:-1])+b'\n']:reject(lambda d=data:validate_output(d,base))
        require(not children,'no child processes')
        print(json.dumps(dict(status='passed',qualification='Predictive source-only replay; no C startup qualification.',cases=rows,coverage=sorted(coverage),search_phases=sorted(phases),rejected=rejected,child_processes=0),sort_keys=True))
    finally:subprocess.Popen=original

if __name__=='__main__':main()
