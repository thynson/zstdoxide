#!/usr/bin/env python3
"""Project immutable audited C path traces into canonical bounded binary deltas."""
import hashlib
import json
from pathlib import Path
import struct
import tarfile
from oracles.optimal_paths import require
from oracles import optimal_path_recipes as small_recipes, optimal_path_validation as small_model
from oracles import optimal_path_wide_recipes as wide_recipes, optimal_path_wide_validation as wide_model

ROOT = Path(__file__).resolve().parents[1]
DEST = ROOT/'tests/fixtures/reference-optimal-paths'
STAGES = ['begin','query','origin','seed','literal','relax','select','reverse_start','reverse','sequence','refresh','complete']
KEYS = ['lit','ll','ml','of']
NONE = 0xffffffff


def sha(data):return hashlib.sha256(data).hexdigest()


def archive(stem):
    report=json.loads((ROOT/'docs/evidence'/('2026-10-14-'+stem+'.json')).read_text())
    path=ROOT/'docs/evidence'/report['archive']
    require(path.stat().st_size==report['archive_bytes'] and sha(path.read_bytes())==report['archive_sha256'],'immutable capture archive')
    expected={f['path']:f for f in report['files']}
    require(len(expected)==len(report['files']),'unique archived files')
    retained={};total=0
    with tarfile.open(path,'r:gz')as tar:
        members=tar.getmembers()
        require(len(members)==len(expected) and {m.name for m in members}==set(expected),'exact archive inventory')
        for member in members:
            require(member.isfile() and member.size<=128<<20,'bounded regular member')
            total+=member.size;require(total<=768<<20,'bounded archive expansion')
            data=tar.extractfile(member).read()
            require(len(data)==expected[member.name]['bytes'] and sha(data)==expected[member.name]['sha256'],'archived artifact identity')
            if member.name=='evidence.json' or member.name.startswith('requests/') or ('-run-path_'in member.name and member.name.endswith('.stdout')):
                retained[member.name]=data
    evidence=json.loads(retained['evidence.json'])
    require(evidence['status']=='passed' and evidence['source_unchanged'] is True and evidence['audit']['normal_instrumented_equal'] is True,'audited capture completion')
    for name,digest in evidence['source'].items():
        require(sha((ROOT/name).read_bytes())==digest,'same independent replay source')
    return retained,report['archive_sha256']


def project():
    pack=bytearray(b'ZOPFX01\0'+struct.pack('<I',27));cases=[];archives={}
    def words(*values):
        require(all(type(v)is int and 0<=v<=NONE for v in values),'u32 projection')
        pack.extend(struct.pack('<'+'I'*len(values),*values))
    def optional(value):
        words(int(value is not None))
        if value is not None:words(*value)
    for kind,stem,recipes,model in [(0,'optimal-path-capture',small_recipes.recipes,small_model),(1,'optimal-path-wide-capture',wide_recipes.recipes,wide_model)]:
        captured,digest=archive(stem);archives[stem]=digest
        for name,request in recipes().items():
            require(captured['requests/'+name+'.bin']==request.packet(),'exact archived request')
            raw=captured['logs/normal-run-'+name+'.stdout']
            require(raw==captured['logs/sanitized-run-'+name+'.stdout'],'both C builds agree')
            model.validate_output(raw,request)
            lines=raw.splitlines();count=(len(lines)-2)//2;path_cells=min(len(request.source)+3,4099)
            start=len(pack);encoded_name=name.encode('ascii')
            words(len(encoded_name),len(request.source),request.min_match,request.target,request.literal_mode,path_cells,count,len(request.scripts),kind)
            pack.extend(encoded_name);pack.extend(request.source)
            for position,candidates in request.scripts:
                words(position,len(candidates))
                for pair in candidates:words(*pair)
            previous=[[0]*7 for _ in range(path_cells)];frequencies=[0]*386;sequences=[];seen=0
            for line in lines[1:1+count]:
                event=json.loads(line)
                require(event['record']=='event' and event['padding_pass']==0 and event['index']==seen,'first complete padding pass')
                seen+=1
                words(STAGES.index(event['stage']),int(event['phase']=='reverse'),event['ip'],event['anchor'],
                    *(NONE if event[k]is None else event[k] for k in ('cur','last','tail')),*event['repeats'])
                optional(event['stretch']);optional(event['reverse']);optional(event['query'])
                if event['query']is not None:
                    words(len(event['matches']))
                    for pair in event['matches']:words(*pair)
                if kind==0:
                    require(len(event['cells'])==path_cells,'complete small path extent')
                    changes=[[i,c]for i,c in enumerate(event['cells'])if c!=previous[i]]
                else:
                    require(event['cells']['length']==path_cells,'fixed wider path extent')
                    changes=event['cells']['changes']
                words(len(changes));prior=-1
                for index,cell in changes:
                    require(prior<index<path_cells and len(cell)==7 and previous[index]!=cell,'canonical complete cell change')
                    words(index,*cell);previous[index]=cell;prior=index
                current=[v for key in KEYS for v in event['frequencies'][key]]+[event['sums'][k]for k in KEYS]+[event['base_prices'][k]for k in KEYS]+[int(event['predefined'])]
                require(len(current)==386,'complete frequency view')
                changed=[(i,v)for i,v in enumerate(current)if v!=frequencies[i]];words(len(changed))
                for pair in changed:words(*pair)
                frequencies=current
                output=event['sequences'];require(output[:len(sequences)]==sequences,'append-only exact sequences')
                appended=output[len(sequences):];words(len(output),len(appended))
                for row in appended:words(*row)
                sequences=output
            require(seen==count,'complete event projection')
            cases.append(dict(name=name,kind=kind,offset=start,bytes=len(pack)-start,events=count,source_bytes=len(request.source),
                packet_sha256=sha(request.packet()),transcript_sha256=sha(raw),sequences=len(sequences)))
            if kind:wide_model._expected_json.cache_clear()
    require(len(cases)==27 and len(pack)<=16<<20,'bounded complete fixture pack')
    DEST.mkdir(parents=True,exist_ok=True)
    (DEST/'cases.bin').write_bytes(pack)
    report=dict(schema_version=1,archive_sha256=archives,cases=cases,pack_bytes=len(pack),pack_sha256=sha(pack),
        events=sum(c['events']for c in cases),qualification='One exact C copy after both builds/padding passes and independent replay; complete path/frequency states reconstruct from canonical deltas. No production path parser admission.')
    (DEST/'provenance.json').write_text(json.dumps(report,sort_keys=True,indent=2)+'\n')
    print(json.dumps({k:report[k]for k in ('pack_bytes','pack_sha256','events')},sort_keys=True))


if __name__=='__main__':project()
