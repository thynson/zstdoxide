#!/usr/bin/env python3
"""Project immutable audited C search maps; no C execution or result synthesis."""
import argparse
import hashlib
import io
import json
from pathlib import Path, PurePosixPath
import struct
import tarfile
from oracles.optimal_match_recipes import recipes
from oracles.optimal_match_validation_v2 import validate
from oracles.optimal_matches import require
from oracles.tree_validation import parse

ROOT=Path(__file__).resolve().parents[1]
ARCHIVE='docs/evidence/2026-10-14-optimal-match-capture.tar.gz'
ARCHIVE_SHA='610321786cee1dd978aed0fb2510d926d4ea14d19e0e38d6cdcd75e9e1ec2e74'
DEST=ROOT/'tests/fixtures/reference-optimal-matches'
MAGIC=b'ZOMFX01\0'


def sha(data):return hashlib.sha256(data).hexdigest()
def enc(value):return (json.dumps(value,sort_keys=True,indent=2)+'\n').encode()


def project():
    path=ROOT/ARCHIVE
    require(path.is_file() and not path.is_symlink() and path.stat().st_size==7258565,'bounded archive')
    raw=path.read_bytes();require(sha(raw)==ARCHIVE_SHA,'immutable archive identity')
    prefix='optimal-match-capture/'
    members={};total=0
    with tarfile.open(fileobj=io.BytesIO(raw),mode='r:gz') as archive:
        for entry in archive:
            name=PurePosixPath(entry.name)
            require(entry.isfile() and entry.name.startswith(prefix) and not name.is_absolute()
                    and '..' not in name.parts and str(name)==entry.name and '\\' not in entry.name
                    and entry.name not in members,'regular unique archive member')
            total+=entry.size
            require(0<=entry.size<=8<<20 and total<=40<<20 and len(members)<464,'archive bounds')
            data=archive.extractfile(entry).read();require(len(data)==entry.size,'complete member')
            members[entry.name]=data
    require(len(members)==464,'complete archive inventory')
    stored=lambda name:members[prefix+name]
    manifest=parse(stored('file-manifest.json'))
    require(set(manifest)=={n[len(prefix):] for n in members if n!=prefix+'file-manifest.json'},'manifest inventory')
    for name,record in manifest.items():
        data=stored(name)
        require(record==dict(bytes=len(data),sha256=sha(data)),'every member identity')
    evidence=parse(stored('capture/evidence.json'));report=parse(stored('report.json'))
    require(evidence['status']=='failed' and evidence['error']=="ValueError('independent complete search state')"
            and evidence['source_unchanged'] is True and report['status']=='passed'
            and report['normal_instrumented_equal'] is True,'explicit reconciled capture qualification')
    for name in ['scripts/oracles/optimal_matches.py','scripts/oracles/optimal_match_recipes.py','scripts/oracles/tree_validation.py','scripts/oracles/tree_direct.py']:
        require(stored('capture/source/'+name)==(ROOT/name).read_bytes(),'frozen input dependency')
    name='scripts/oracles/optimal_match_validation_v2.py'
    require(stored('reconciliation/'+name)==(ROOT/name).read_bytes(),'reviewed corrected model')
    cases=recipes();pack=bytearray(MAGIC+struct.pack('<I',len(cases)));catalog=[]
    for recipe in cases:
        request=recipe.request
        require(stored('capture/requests/'+recipe.name+'.bin')==request.packet(),'exact captured packet')
        logs=[stored('capture/logs/'+mode+'-run-'+recipe.name+'.stdout') for mode in ['normal','sanitized']]
        require(logs[0]==logs[1],'normal/instrumented equality');validate(request,logs[0])
        rows=[parse(line) for line in logs[0].splitlines()][1:1+len(request.queries)]
        start=len(pack);name=recipe.name.encode('ascii')
        geometry=[request.window_log,request.hash_log,request.chain_log,request.search_log,request.min_match,request.target,request.hash3_log]
        pack.extend(struct.pack('<10I',len(name),len(request.source),len(rows),*geometry));pack.extend(name);pack.extend(request.source)
        for row in rows:
            words=list(row['query'])
            for key in ['before','after']:
                state=row[key];words.extend([state['frontier'],state['short_frontier']]);words.extend(state['heads'])
                words.extend(value for pair in state['nodes'] for value in pair);words.extend(state['short_heads'])
            words.append(len(row['matches']));words.extend(value for pair in row['matches'] for value in pair)
            pack.extend(struct.pack('<'+str(len(words))+'I',*words))
        catalog.append(dict(id=recipe.name,offset=start,bytes=len(pack)-start,source_sha256=sha(request.source),
                            request_sha256=sha(request.packet()),transcript_sha256=sha(logs[0]),queries=len(rows)))
    require(len(pack)<=16<<20,'bounded projected pack')
    provenance=dict(schema_version=1,archive=ARCHIVE,archive_sha256=ARCHIVE_SHA,pack_bytes=len(pack),
                    pack_sha256=sha(pack),cases=catalog,source_bytes=71072,queries=273,
                    qualification='Exact C observed maps from a v2-reconciled audit. The original failed v1 model comparison is retained in the pinned archive. No production sorted-search or public Optimal support.')
    return bytes(pack),enc(provenance)


def main(write=False):
    pack,provenance=project()
    if write:
        require(not DEST.exists(),'fresh fixture destination');DEST.mkdir(parents=True)
        (DEST/'cases.bin').write_bytes(pack);(DEST/'provenance.json').write_bytes(provenance)
    require((DEST/'cases.bin').read_bytes()==pack and (DEST/'provenance.json').read_bytes()==provenance,'exact fixture reconstruction')
    print(json.dumps(dict(status='passed',cases=29,queries=273,pack_bytes=len(pack),pack_sha256=sha(pack)),indent=2))


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--write',action='store_true');main(parser.parse_args().write)
