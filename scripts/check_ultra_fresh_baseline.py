#!/usr/bin/env python3
"""Reconcile every original fresh native state and bound preliminary Ultra replay."""
from pathlib import Path
import dataclasses,hashlib,json,struct,sys
sys.dont_write_bytecode=True
from oracles.ultra_fresh import Request,parse_packet,MAX_PACKET
from oracles.ultra_fresh_validation import observations,records,validate_output
from oracles.optimal_fresh_wide import Request as Original
from oracles.optimal_fresh_wide_recipes import recipes
from oracles.ultra_scalar import need,load,sha
from pack_optimal_fresh_wide_fixtures import captured

def canonical(value):return json.dumps(value,sort_keys=True,separators=(',',':'))
def encoded(rows):return b''.join((canonical(row)+'\n').encode()for row in rows)
def check():
    children=[]
    def guard(event,args):
        if event in('subprocess.Popen','os.system','os.posix_spawn','os.fork'):
            children.append(event);raise ValueError('child execution forbidden')
    sys.addaudithook(guard)
    stored,archive_sha=captured();catalog=[];total=0;coverage=set()
    for name,recipe in recipes().items():
        request=Request(0,recipe.request);packet=request.packet()
        need(parse_packet(packet)==request,'policy packet round trip')
        need(packet[12:]==stored['requests/'+name+'.bin'],'unchanged original packet')
        raw=stored['logs/normal-run-'+name+'.stdout'];need(raw==stored['logs/sanitized-run-'+name+'.stdout'],'original build equality')
        lines=iter(raw.splitlines());events=0
        for expected in observations(request,recipe.kinds):
            expected.pop('price_policy');expected.pop('alternative',None)
            if expected['record']=='metadata':expected['protocol']='ZOPFW03'
            line=next(lines,None);need(line is not None,'complete native inventory')
            need(canonical(load(line))==canonical(expected),'complete original native state')
            events+=expected['record']=='event'
        need(next(lines,None)is None,'native EOF');total+=events
        ultra=Request(2,recipe.request);need(parse_packet(ultra.packet())==ultra,'Ultra packet round trip')
        rows,branches=records(ultra,recipe.kinds);coverage.update(branches)
        logical=[row for row in rows if row['record']=='event']
        catalog.append(dict(name=name,whole_packet_sha256=sha(packet),ultra_packet_sha256=sha(ultra.packet()),native_transcript_sha256=sha(raw),native_events_both_passes=events,ultra_predicted_events=len(logical),ultra_predicted_alternatives=sum(row['alternative']is not None for row in logical),ultra_predicted_sequences=rows[-1]['sequences'],ultra_rows_sha256=sha(encoded(rows)),coverage=sorted(branches)))
    need(len(catalog)==20 and total==129220,'complete twenty-case original inventory')
    rejected=[]
    def reject(name,call):
        try:call()
        except (ValueError,TypeError):rejected.append(name)
        else:raise ValueError('accepted forgery '+name)
    request=Request(2,Original(b'a'*64,(64,)));packet=request.packet()
    for name,data in [('empty',b''),('short',packet[:123]),('truncated',packet[:-1]),('extra',packet+b'!'),('magic',b'!'+packet[1:]),('nested-magic',packet[:12]+b'!'+packet[13:]),('policy1',packet[:8]+struct.pack('<I',1)+packet[12:]),('policy3',packet[:8]+struct.pack('<I',3)+packet[12:]),('oversize',b'x'*(MAX_PACKET+1))]:
        reject(name,lambda d=data:parse_packet(d))
    for name,changed in [('bool-policy',dataclasses.replace(request,policy=True)),('float-policy',dataclasses.replace(request,policy=2.0)),('wrong-request',dataclasses.replace(request,original=None))]:
        reject(name,changed.validate)
    for offset,value in [(20,16385),(24,2),(28,9),(32,13),(36,13),(40,7),(44,2),(48,4096),(52,2),(56,63)]:
        data=packet[:offset]+struct.pack('<I',value)+packet[offset+4:]
        reject('nested-'+str(offset),lambda d=data:parse_packet(d))
    good=list(observations(request,(2,)));validate_output(encoded(good),request,(2,))
    import copy
    for kind in ['policy','stage','query','search-before','search-value','search-length','path-index','path-value','frequency','alternative','sequence','owner','candidate','kind','padding','bool']:
        rows=copy.deepcopy(good);query=next(r for r in rows if r.get('stage')=='query');origin=next(r for r in rows if r.get('stage')=='origin');pub=next(r for r in rows if r['record']=='block_publish')
        if kind=='policy':query['price_policy']=0
        elif kind=='stage':query['stage']='literal'
        elif kind=='query':query['query'][0]+=1
        elif kind=='search-before':query['search_before']['changes']=[[0,1]]
        elif kind=='search-value':query['search']['changes'].append([0,999])
        elif kind=='search-length':query['search']['length']+=1
        elif kind=='path-index':origin['cells']['changes'][0][0]=4099
        elif kind=='path-value':origin['cells']['changes'][0][1][0]+=1
        elif kind=='frequency':query['frequencies']['ll'][0]+=1
        elif kind=='alternative':origin['alternative']=[0]*12
        elif kind=='sequence':next(r for r in rows if r.get('stage')=='sequence')['sequences'][0][3]+=1
        elif kind=='owner':pub['previous_owner_after']^=1
        elif kind=='candidate':pub['candidate_repeats'][0]+=1
        elif kind=='kind':pub['kind']=0
        elif kind=='padding':query['padding_pass']=1
        else:query['query'][0]=True
        reject(kind,lambda r=rows:validate_output(encoded(r),request,(2,)))
    need(not children,'no child execution')
    return dict(status='passed',optimized=not __debug__,archive_sha256=archive_sha,cases=catalog,native_events_both_passes=total,preliminary_ultra_events=sum(r['ultra_predicted_events']for r in catalog),preliminary_ultra_alternatives=sum(r['ultra_predicted_alternatives']for r in catalog),coverage=sorted(coverage),rejected=rejected,child_processes=0,qualification='Complete original twenty-case ordinary native search/path reconciliation. Ultra values are preliminary independent model predictions on unchanged old sources, not new native observations, entropy predictions or public admission.')
if __name__=='__main__':print(json.dumps(check(),sort_keys=True,indent=2))
