#!/usr/bin/env python3
"""Project fully audited native Ultra2 lifecycle states into an additive fixture."""
import argparse
import json
from pathlib import Path
import struct
import sys
sys.dont_write_bytecode=True
import check_ultra2_capture as auditor
from oracles.ultra2_recipes import recipes
from oracles.ultra2_protocol import Request
from oracles.ultra_scalar import need
from pack_ultra_fresh_fixture import words,delta,observed_delta,STAGES,KEYS,sha,enc

ROOT=Path(__file__).resolve().parents[1]
DEST=ROOT/'tests/fixtures/reference-ultra2'


def frequency(row):
    return [v for k in KEYS for v in row['tables'][k]]+[row['sums'][k]for k in KEYS]+[row['cache'][k]for k in KEYS]+[int(row['predefined'])]


def project(capture):
    verdict=auditor.audit(capture)
    need(verdict['recipes']==53 and verdict['logical_events_per_padding_pass']==394656,'qualified complete capture')
    pack=bytearray(b'ZU2FX01\0'+words(53));catalog=[];total=0
    for name,recipe in recipes().items():
        request=Request(recipe.request,recipe.seed);r=recipe.request
        need(auditor.read(capture,'requests/'+name+'.bin',16428)==request.packet(),'sealed request')
        raw=auditor.read(capture,'logs/normal-run-'+name+'.stdout',256<<20)
        need(raw==auditor.read(capture,'logs/sanitized-run-'+name+'.stdout',256<<20),'both native builds equal')
        auditor.validate_model_output(raw,request)
        rows=[auditor.load(line)for line in raw.splitlines()if b'"padding_pass":0'in line]
        need(rows and all(row['padding_pass']==0 for row in rows),'first complete padding run')
        eligibility=rows[0];need(eligibility['record']=='eligibility','entry record')
        phases=sum(row['record']=='pass_begin'for row in rows);start=len(pack);encoded=name.encode('ascii')
        pack.extend(words(len(encoded),len(r.source),r.window_log,r.hash_log,r.chain_log,r.search_log,r.min_match,r.target,r.literal_mode,int(recipe.seed),phases,4099))
        for mode in ('normal','sanitized'):
            a=auditor.read(capture,f'artifacts/{mode}/{name}.0.json',32768)
            need(a==auditor.read(capture,f'artifacts/{mode}/{name}.1.json',32768),'both padding ledgers equal')
            ledger=auditor.load(a);auditor.validate_ledger(ledger);pack.extend(words(*ledger['allocations'],ledger['peak_bytes']))
        pack.extend(encoded);pack.extend(r.source)
        e=eligibility;pack.extend(words(*(int(p)for p in e['predicates']),int(e['seed']),e['current'],e['dictionary_limit'],e['low_limit'],e['next_to_update'],e['source_bytes'],*e['original_repeats'],e['sequence_prefix']))
        paths=[[0]*7 for _ in range(4099)];counts=[0]*386
        search_size=(1<<r.hash_log)+(1<<r.chain_log)+(1<<r.window_log if r.min_match==3 else 0)+2
        search=[0]*search_size;cursor=1;phase_catalog=[]
        for phase in range(phases):
            begin=rows[cursor];cursor+=1;events=[]
            while cursor<len(rows)and rows[cursor]['record']=='event':events.append(rows[cursor]);cursor+=1
            end=rows[cursor];cursor+=1
            need(begin['record']=='pass_begin'and end['record']=='pass_end'and begin['phase']==end['phase'],'complete phase envelope')
            begin_counts=frequency(begin['frequency']);need(begin_counts==counts,'retained pre-rescale frequencies')
            pack.extend(words(int(begin['phase']=='encoding'),begin['origin'],*begin['repeats'],len(events)));pack.extend(words(*begin_counts))
            sequences=[]
            for index,event in enumerate(events):
                need(event['index']==index and event['pass_name']==begin['phase'],'ordered phase events')
                pack.extend(words(STAGES.index(event['stage']),int(event['phase']=='reverse'),event['ip'],event['anchor'],*(0xffffffff if event[k]is None else event[k]for k in ('cur','last','tail')),*event['repeats']))
                for key in ('stretch','reverse','query'):
                    pack.extend(words(int(event[key]is not None)))
                    if event[key]is not None:pack.extend(words(*event[key]))
                if event['query']is not None:
                    pack.extend(words(len(event['matches'])))
                    for pair in event['matches']:pack.extend(words(*pair))
                alternate=event['alternative'];pack.extend(words(int(alternate is not None)))
                if alternate is not None:pack.extend(struct.pack('<12q',*alternate))
                observed_delta(pack,event['cells'],paths,7)
                current=[v for k in KEYS for v in event['frequencies'][k]]+[event['sums'][k]for k in KEYS]+[event['base_prices'][k]for k in KEYS]+[int(event['predefined'])]
                counts=delta(pack,current,counts,1)
                need(event['sequences'][:len(sequences)]==sequences,'append-only phase sequence owner')
                appended=event['sequences'][len(sequences):];pack.extend(words(len(event['sequences']),len(appended)))
                for row in appended:pack.extend(words(*row))
                sequences=event['sequences']
                if event['query']is not None:need(event['search_before']==dict(length=search_size,changes=[]),'unchanged complete pre-query snapshot')
                need(index==0 or event['query']is not None or not event['search']['changes'],'only queries mutate search after begin')
                observed_delta(pack,event['search'],search,1)
            need(end['sequences']==sequences and frequency(end['frequency'])==counts,'exact terminal owners')
            pack.extend(words(*end['candidate_repeats'],len(sequences),end['tail'],*counts))
            reset=cursor<len(rows)and rows[cursor]['record']=='reset';pack.extend(words(int(reset)))
            need(reset==(begin['phase']=='seeding'),'reset only discarded pass')
            if reset:
                e=rows[cursor];cursor+=1
                need(frequency(e['frequency'])==counts,'reset retains complete frequency state')
                pack.extend(words(e['discarded_sequences'],e['discarded_literals'],e['discarded_tail'],*e['caller_repeats'],*e['discarded_repeats'],e['origin_before'],e['origin_after'],e['low_limit'],e['dictionary_limit'],e['next_to_update'],int(e['tables_preserved']),*counts))
            total+=len(events);phase_catalog.append(dict(phase=begin['phase'],origin=begin['origin'],events=len(events),sequences=len(sequences),tail=end['tail']))
        need(cursor==len(rows),'complete lifecycle exhaustion')
        catalog.append(dict(name=name,offset=start,bytes=len(pack)-start,phases=phase_catalog,packet_sha256=sha(request.packet()),transcript_sha256=sha(raw)))
    need(total==394656 and len(pack)<=128<<20,'bounded complete projection')
    provenance=dict(schema_version=1,capture_evidence_sha256=sha((capture/'evidence.json').read_bytes()),pack_bytes=len(pack),pack_sha256=sha(pack),events=total,cases=catalog,qualification='Complete audited native Ultra2 lifecycle projection. Physical path/search and frequency deltas persist across the discarded-pass reset. No entropy/frame output or public admission.')
    return bytes(pack),enc(provenance)


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('capture',type=Path);p.add_argument('--write',action='store_true');a=p.parse_args()
    pack,provenance=project(a.capture.resolve(strict=True))
    if a.write:
        need(not DEST.exists(),'fresh immutable fixture destination');DEST.mkdir();(DEST/'cases.bin').write_bytes(pack);(DEST/'provenance.json').write_bytes(provenance)
    need((DEST/'cases.bin').read_bytes()==pack and (DEST/'provenance.json').read_bytes()==provenance,'exact native projection reproduction')
    print(json.dumps(dict(status='passed',cases=53,events=394656,pack_bytes=len(pack),pack_sha256=sha(pack))))
if __name__=='__main__':main()
