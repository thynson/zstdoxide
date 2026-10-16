#!/usr/bin/env python3
"""Project fully audited native Ultra path observations into complete deltas."""
from pathlib import Path
import argparse,json,struct,sys
sys.dont_write_bytecode=True
import check_ultra_path_capture as audit
from oracles.ultra_path_recipes import recipes
from oracles.ultra_path_validation import event_count
from oracles.ultra_scalar import need,sha,load
STAGES=['begin','query','origin','seed','literal','relax','select','reverse_start','reverse','sequence','refresh','complete','alternative']
KEYS=('lit','ll','ml','of');NONE=0xffffffff

def project(capture,out):
    need(not out.exists(),'fresh projection output');result=audit.audit(capture);pack=bytearray(b'ZUPFX01\0'+struct.pack('<I',58));catalog=[]
    def words(*values):
        need(all(type(v)is int and 0<=v<=NONE for v in values),'u32 values');pack.extend(struct.pack('<'+'I'*len(values),*values))
    def optional(value):
        words(int(value is not None))
        if value is not None:words(*value)
    for name,request in recipes().items():
        original=request.original;source=original.source;count=event_count(request);path_cells=min(len(source)+3,4099);start=len(pack);encoded=name.encode()
        words(len(encoded),len(source),original.min_match,original.target,original.literal_mode,path_cells,count,len(original.scripts),request.policy);pack.extend(encoded);pack.extend(source)
        for position,candidates in original.scripts:
            words(position,len(candidates))
            for pair in candidates:words(*pair)
        previous=[[0]*7 for _ in range(path_cells)];frequencies=[0]*386;sequences=[];alternatives=0;trace=capture/'logs'/('normal-run-'+name+'.stdout')
        with trace.open('rb')as stream:
            metadata=load(stream.readline());need(metadata['price_policy']==request.policy,'metadata policy')
            for index in range(count):
                event=load(stream.readline());need(event['record']=='event'and event['padding_pass']==0 and event['index']==index and event['price_policy']==request.policy,'first complete native pass')
                words(STAGES.index(event['stage']),int(event['phase']=='reverse'),event['ip'],event['anchor'],*(NONE if event[k]is None else event[k]for k in ('cur','last','tail')),*event['repeats'])
                optional(event['stretch']);optional(event['reverse']);optional(event['query'])
                if event['query']is not None:
                    words(len(event['matches']))
                    for pair in event['matches']:words(*pair)
                alternative=event['alternative'];words(int(alternative is not None))
                if alternative is not None:
                    need(len(alternative)==12 and all(type(v)is int and -(1<<63)<=v<1<<63 for v in alternative),'signed alternative record');pack.extend(struct.pack('<12q',*alternative));alternatives+=1
                changes=event['cells']['changes'];need(event['cells']['length']==path_cells,'path extent');words(len(changes));last=-1
                for at,cell in changes:
                    need(last<at<path_cells and len(cell)==7 and previous[at]!=cell,'canonical changed complete cells');words(at,*cell);previous[at]=cell;last=at
                current=[v for key in KEYS for v in event['frequencies'][key]]+[event['sums'][k]for k in KEYS]+[event['base_prices'][k]for k in KEYS]+[int(event['predefined'])];need(len(current)==386,'complete frequencies');changed=[(i,v)for i,v in enumerate(current)if frequencies[i]!=v];words(len(changed))
                for pair in changed:words(*pair)
                frequencies=current;new=event['sequences'];need(new[:len(sequences)]==sequences,'append-only sequence prefix');appended=new[len(sequences):];words(len(new),len(appended))
                for row in appended:words(*row)
                sequences=new
        catalog.append(dict(name=name,policy=request.policy,offset=start,bytes=len(pack)-start,events=count,alternatives=alternatives,sequences=len(sequences),packet_sha256=sha(request.packet()),transcript_sha256=sha(trace.read_bytes())))
    need(len(pack)<=32<<20 and sum(c['events']for c in catalog)==65672 and sum(c['alternatives']for c in catalog)==72,'complete bounded corpus')
    out.mkdir();(out/'cases.bin').write_bytes(pack);provenance=dict(schema_version=1,bytes=len(pack),pack_sha256=sha(pack),capture_evidence_sha256=sha((capture/'evidence.json').read_bytes()),projector_sha256=sha(Path(__file__).read_bytes()),audit=result,catalog=catalog)
    (out/'provenance.json').write_text(json.dumps(provenance,sort_keys=True,indent=2)+'\n');print(json.dumps(dict(status='passed',bytes=len(pack),sha256=sha(pack),cases=len(catalog))))
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('capture',type=Path);p.add_argument('--output',type=Path,required=True);a=p.parse_args();project(a.capture,a.output)
