#!/usr/bin/env python3
"""Project independently audited retained Ultra2 phases and native complete frames."""
import argparse
import json
import hashlib
import tarfile
from pathlib import Path
import struct
import sys
sys.dont_write_bytecode = True
import check_ultra2_retained_capture as auditor
from oracles.ultra2_retained import need
from oracles.ultra2_retained_recipes_v2 import recipes
from pack_ultra_fresh_fixture import words, delta, observed_delta, STAGES, KEYS, sha, enc

ROOT = Path(__file__).resolve().parents[1]
DEST = ROOT/'tests/fixtures/reference-ultra2-retained'
NATIVE_REPORT = 'docs/evidence/2026-10-22-ultra2-retained-native.json'
NATIVE_REPORT_SHA = 'e2df0c64a4acf5e3e01e5693b285b90006ef2c5937dd7a2256a5be8ec0f616d3'


def frequency(row):
    return [v for k in KEYS for v in row['tables'][k]]+[row['sums'][k] for k in KEYS]+[row['cache'][k] for k in KEYS]+[int(row['predefined'])]


def digest_file(path):
    h=hashlib.sha256()
    with path.open('rb') as stream:
        for data in iter(lambda:stream.read(65536),b''):h.update(data)
    return h.hexdigest()


def verified_capture(capture):
    # Inherit the completed model/audit qualification only through its pinned
    # immutable archive; independently rehash every regular captured file.
    report_path=ROOT/NATIVE_REPORT
    need(digest_file(report_path)==NATIVE_REPORT_SHA,'pinned completed native report')
    report=auditor.load(report_path.read_bytes());archive=report_path.parent/report['archive']
    need(report['status']=='passed' and digest_file(archive)==report['archive_sha256']
         and archive.stat().st_size==report['archive_bytes'],'complete native archive identity')
    mapping=None
    with tarfile.open(archive,'r|gz') as stream:
        for entry in stream:
            if entry.name=='file-map.json':
                need(entry.isfile() and entry.size<=8<<20,'bounded native archive map')
                data=stream.extractfile(entry).read();need(sha(data)==report['file_map_sha256'],'bound native file map')
                mapping=auditor.load(data);break
    need(mapping is not None,'native archive map present')
    captured={name.removeprefix('capture/'):row for name,row in mapping.items() if name.startswith('capture/')}
    actual={p.relative_to(capture).as_posix() for p in capture.rglob('*')
            if p.is_file() and not p.is_symlink() and not p.is_relative_to(capture/'special')}
    need(actual==set(captured),'complete unchanged regular native inventory')
    for name,row in captured.items():
        p=capture/name
        need(p.is_file() and not p.is_symlink() and p.resolve().is_relative_to(capture)
             and p.stat().st_size==row['bytes'] and digest_file(p)==row['sha256'],'archived native input '+name)
    evidence=auditor.load((capture/'evidence.json').read_bytes())
    need(evidence['status']=='passed' and evidence['source_unchanged'] is True
         and evidence['source']==report['source'],'completed native source identity')
    for name,value in evidence['source'].items():need(digest_file(ROOT/name)==value,'unchanged qualified native source')
    return evidence['audit'],dict(report_sha256=NATIVE_REPORT_SHA,archive_sha256=report['archive_sha256'],
        rehashed_regular_files=len(captured),qualification='Complete native model replay and coherent controls are inherited from the pinned successful archive. This projection independently rehashes every captured regular file; it does not claim another native/model execution.')


def project(capture):
    verdict, inherited = verified_capture(capture)
    need(verdict['recipes']==32 and verdict['logical_events_per_padding_pass']==229657,
         'complete qualified native capture')
    pack = bytearray(b'ZU2RFX01'+words(32));catalog=[];total=0
    for name,recipe in recipes().items():
        request=recipe.request;r=request.original
        need(auditor.read(capture,'requests/'+name+'.bin',32836)==request.packet(),'sealed input')
        raw=auditor.read(capture,'logs/normal-run-'+name+'.stdout',256<<20)
        need(raw==auditor.read(capture,'logs/sanitized-run-'+name+'.stdout',256<<20),'native build equality')
        # Exact transcript bytes were independently bound to the qualified archive above.
        records=[auditor.load(line) for line in raw.splitlines() if b'"padding_pass":0' in line]
        need(records and all(row['padding_pass']==0 for row in records),'one complete padding run')
        frame=auditor.read(capture,f'artifacts/normal/{name}.0.zst',32832)
        for mode in ('normal','sanitized'):
            for padding in (0,1):
                need(auditor.read(capture,f'artifacts/{mode}/{name}.{padding}.zst',32832)==frame,'all four complete frames')
        start=len(pack);encoded=name.encode('ascii')
        pack.extend(words(len(encoded),len(r.source),r.window_log,r.hash_log,r.chain_log,
            r.search_log,r.min_match,r.target,r.literal_mode,len(r.blocks),len(frame),4099,1,2))
        for mode in ('normal','sanitized'):
            ledger=auditor.load(auditor.read(capture,f'artifacts/{mode}/{name}.0.json',2048))
            need(len(ledger['allocations'])==2 and ledger['released'] is True,'two released allocator owners')
            pack.extend(words(*ledger['allocations'],ledger['peak_bytes']))
        pack.extend(encoded);pack.extend(r.source);pack.extend(frame)
        paths=[[0]*7 for _ in range(4099)];counts=[0]*386
        search_size=(1<<r.hash_log)+(1<<r.chain_log)+(1<<r.window_log if r.min_match==3 else 0)+2
        search=[0]*search_size;cursor=0;blocks=[];owners=[[1,4,8],[0,0,0]];previous=0
        for block,length in enumerate(r.blocks):
            begin=records[cursor];cursor+=1
            need(begin['record']=='block_begin' and begin['block']==block and begin['start']==sum(r.blocks[:block])
                 and begin['length']==length and begin['previous_owner']==previous and begin['owners']==owners,
                 'complete native block entry')
            end_index=cursor
            while end_index<len(records) and records[end_index]['record']!='block_publish':end_index+=1
            need(end_index<len(records),'actual publication boundary');publish=records[end_index]
            need(publish['block']==block and publish['kind']==recipe.kinds[block],'actual frozen publication')
            phases=sum(row['record']=='pass_begin' for row in records[cursor:end_index])
            pack.extend(words(begin['start'],length,publish['kind'],publish['previous_owner_before'],
                publish['previous_owner_after'],begin['frontier'],phases,*publish['before_repeats'],
                *publish['candidate_repeats'],*(v for owner in publish['owners'] for v in owner),
                publish['tail'],publish['sequences']))
            gate=records[cursor];cursor+=1
            need(gate['record']=='eligibility' and gate['block']==block,'block eligibility')
            pack.extend(words(*(int(p) for p in gate['predicates']),int(gate['seed']),gate['current'],
                gate['dictionary_limit'],gate['low_limit'],gate['next_to_update'],gate['source_bytes'],
                *gate['original_repeats'],gate['sequence_prefix']))
            phase_catalog=[]
            for phase in range(phases):
                phase_begin=records[cursor];cursor+=1;events=[]
                while cursor<end_index and records[cursor]['record']=='event':events.append(records[cursor]);cursor+=1
                phase_end=records[cursor];cursor+=1
                need(phase_begin['record']=='pass_begin' and phase_end['record']=='pass_end' and
                     phase_begin['phase']==phase_end['phase'] and phase_begin['block']==phase_end['block']==block,
                     'complete chronological phase')
                initial=frequency(phase_begin['frequency']);need(initial==counts,'retained initial frequency')
                pack.extend(words(int(phase_begin['phase']=='encoding'),phase_begin['origin'],
                                  *phase_begin['repeats'],len(events),*initial))
                sequences=[]
                for index,event in enumerate(events):
                    need(event['block']==block and event['index']==index and event['pass_name']==phase_begin['phase'],
                         'ordered block/phase events')
                    pack.extend(words(STAGES.index(event['stage']),int(event['phase']=='reverse'),event['ip'],event['anchor'],
                        *(0xffffffff if event[k] is None else event[k] for k in ('cur','last','tail')),*event['repeats']))
                    for key in ('stretch','reverse','query'):
                        pack.extend(words(int(event[key] is not None)))
                        if event[key] is not None:pack.extend(words(*event[key]))
                    if event['query'] is not None:
                        pack.extend(words(len(event['matches'])))
                        for pair in event['matches']:pack.extend(words(*pair))
                    alternate=event['alternative'];pack.extend(words(int(alternate is not None)))
                    if alternate is not None:
                        need(len(alternate)==12 and all(type(v)is int and -(1<<63)<=v<1<<63 for v in alternate),'signed alternative domain')
                        pack.extend(struct.pack('<12q',*alternate))
                    observed_delta(pack,event['cells'],paths,7)
                    current=[v for k in KEYS for v in event['frequencies'][k]]+[event['sums'][k] for k in KEYS]+[event['base_prices'][k] for k in KEYS]+[int(event['predefined'])]
                    counts=delta(pack,current,counts,1)
                    need(event['sequences'][:len(sequences)]==sequences,'append-only phase tokens')
                    added=event['sequences'][len(sequences):];pack.extend(words(len(event['sequences']),len(added)))
                    for row in added:pack.extend(words(*row))
                    sequences=event['sequences']
                    if event['query'] is not None:need(event['search_before']==dict(length=search_size,changes=[]),'exact prior complete search')
                    need(index==0 or event['query'] is not None or not event['search']['changes'],'search mutations only in queries/entry')
                    observed_delta(pack,event['search'],search,1)
                need(phase_end['sequences']==sequences and frequency(phase_end['frequency'])==counts,'complete terminal state')
                pack.extend(words(*phase_end['candidate_repeats'],len(sequences),phase_end['tail'],*counts))
                reset=cursor<end_index and records[cursor]['record']=='reset';pack.extend(words(int(reset)))
                need(reset==(phase_begin['phase']=='seeding') and (not reset or block==0),'first discarded pass only')
                if reset:
                    row=records[cursor];cursor+=1;need(frequency(row['frequency'])==counts,'retained reset frequencies')
                    pack.extend(words(row['discarded_sequences'],row['discarded_literals'],row['discarded_tail'],
                        *row['caller_repeats'],*row['discarded_repeats'],row['origin_before'],row['origin_after'],
                        row['low_limit'],row['dictionary_limit'],row['next_to_update'],int(row['tables_preserved']),*counts))
                total+=len(events)
                phase_catalog.append(dict(phase=phase_begin['phase'],origin=phase_begin['origin'],events=len(events),
                    alternatives=sum(e['alternative'] is not None for e in events),sequences=len(sequences),tail=phase_end['tail']))
            need(cursor==end_index and phase_end['phase']=='encoding' and len(sequences)==publish['sequences']
                 and phase_end['tail']==publish['tail'] and phase_end['candidate_repeats']==publish['candidate_repeats'],
                 'actual tokens alone reach publication')
            cursor+=1;owners=publish['owners'];previous=publish['previous_owner_after']
            blocks.append(dict(start=begin['start'],length=length,kind=publish['kind'],phases=phase_catalog))
        need(cursor==len(records),'complete chronological exhaustion')
        catalog.append(dict(name=name,offset=start,bytes=len(pack)-start,blocks=blocks,frame_bytes=len(frame),
            frame_sha256=sha(frame),packet_sha256=sha(request.packet()),transcript_sha256=sha(raw)))
        print('Projected '+name,file=sys.stderr,flush=True)
    need(total==229657 and len(pack)<=128<<20,'bounded complete projection')
    provenance=dict(schema_version=1,capture_evidence_sha256=sha((capture/'evidence.json').read_bytes()),
        pack_bytes=len(pack),pack_sha256=sha(pack),events=total,cases=catalog,inherited_native_qualification=inherited,
        qualification='Complete independently audited native retained Ultra2 first/later-block lifecycle and frame projection. Physical parser/search/frequency state persists across discarded reset and actual publication. No production candidate provider or public admission.')
    return bytes(pack),enc(provenance)


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('capture',type=Path);p.add_argument('--write',action='store_true');a=p.parse_args()
    pack,provenance=project(a.capture.resolve(strict=True))
    if a.write:
        need(not DEST.exists(),'fresh immutable fixture');DEST.mkdir();(DEST/'cases.bin').write_bytes(pack);(DEST/'provenance.json').write_bytes(provenance)
    need((DEST/'cases.bin').read_bytes()==pack and (DEST/'provenance.json').read_bytes()==provenance,'exact complete projection reproduction')
    print(json.dumps(dict(status='passed',cases=32,events=229657,pack_bytes=len(pack),pack_sha256=sha(pack)),sort_keys=True))

if __name__=='__main__':main()
