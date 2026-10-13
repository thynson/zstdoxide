#!/usr/bin/env python3
"""Losslessly deduplicate the accepted ordinary tree capture archive.

This changes storage only. Every member of the initial archive is reconstructed
and verified, including binaries, failed v1 audits and all four observation sets.
The original archive identity remains in the original report and capture manifest.
"""
from pathlib import Path, PurePosixPath
import argparse
import hashlib
import io
import json
import tarfile
ROOT = Path(__file__).resolve().parents[1]
ORIGINAL_SHA = '67cd7e56765da0a9a532b157d7b373ed049aea3306f85a3626b3bda1ffb6da54'
DEST = ROOT / 'docs/evidence/2026-10-13-tree-frames-storage-v2.tar.gz'
def sha(b): return hashlib.sha256(b).hexdigest()
def encode(v): return (json.dumps(v, sort_keys=True, indent=2)+'\n').encode()
def need(v, reason):
    if not v: raise ValueError(reason)
RECONSTRUCT = '''from pathlib import Path, PurePosixPath
import hashlib,json,sys
base=Path(__file__).resolve().parent;out=Path(sys.argv[1]).resolve()
if out.exists() or out.is_relative_to(base) or base.is_relative_to(out):raise ValueError("fresh disjoint output required")
rows=json.loads((base/"file-map.json").read_bytes());out.mkdir(parents=True)
for name,row in rows.items():
 p=PurePosixPath(name)
 if p.is_absolute() or ".." in p.parts or str(p)!=name:raise ValueError("canonical relative name")
 target=out/name;target.parent.mkdir(parents=True,exist_ok=True);h=hashlib.sha256();size=0
 with target.open("xb") as f:
  for key in row["chunks"]:
   if len(key)!=64 or any(c not in "0123456789abcdef" for c in key):raise ValueError("chunk key")
   b=(base/"chunks"/(key+".bin")).read_bytes()
   if hashlib.sha256(b).hexdigest()!=key:raise ValueError("chunk digest")
   f.write(b);h.update(b);size+=len(b)
 if size!=row["bytes"] or h.hexdigest()!=row["sha256"]:raise ValueError("file digest")
print("Reconstructed",len(rows),"exact files into",out)
'''
def main(original):
    need(sha(original.read_bytes())==ORIGINAL_SHA,'accepted original archive')
    need(not DEST.exists(),'fresh storage archive')
    chunks={};mapping={}
    with tarfile.open(original,'r|gz') as t:
        for m in t:
            p=PurePosixPath(m.name)
            need(m.isfile() and not p.is_absolute() and '..' not in p.parts and str(p)==m.name and m.name not in mapping and m.size<=64<<20,'bounded canonical original member')
            f=t.extractfile(m);h=hashlib.sha256();size=0;keys=[]
            for b in iter(lambda:f.read(4096),b''):
                key=sha(b);chunks[key]=b;keys.append(key);h.update(b);size+=len(b)
            need(size==m.size,'original member size')
            mapping[m.name]=dict(bytes=size,sha256=h.hexdigest(),chunks=keys)
    need(len(mapping)==4117,'complete original member inventory')
    report=dict(status='passed',original_archive_sha256=ORIGINAL_SHA,original_members=len(mapping),original_member_bytes=sum(r['bytes'] for r in mapping.values()),unique_chunks=len(chunks),unique_bytes=sum(map(len,chunks.values())),policy='Storage-only lossless 4096-byte deduplication. Every original regular member is retained, including all compiled objects and all four observation sets. Reconstruct using the included script; original gzip container metadata is not reproduced.')
    members={'file-map.json':encode(mapping),'report.json':encode(report),'reconstruct.py':RECONSTRUCT.encode(),**{'chunks/'+k+'.bin':b for k,b in chunks.items()}}
    with tarfile.open(DEST,'w:gz') as t:
        for n,b in members.items():
            info=tarfile.TarInfo(n);info.size=len(b);info.mode=0o644;t.addfile(info,io.BytesIO(b))
    recovered={}
    with tarfile.open(DEST,'r|gz') as t:
        for m in t:
            need(m.isfile() and m.name in members and m.name not in recovered,'exact deduplicated archive')
            b=t.extractfile(m).read();need(b==members[m.name],'exact archived member');recovered[m.name]=b
    need(set(recovered)==set(members),'complete deduplicated inventory')
    for n,r in mapping.items():
        h=hashlib.sha256();size=0
        for key in r['chunks']:
            b=recovered['chunks/'+key+'.bin'];h.update(b);size+=len(b)
        need(size==r['bytes'] and h.hexdigest()==r['sha256'],'reconstructed original '+n)
    report.update(archive=DEST.name,archive_bytes=DEST.stat().st_size,archive_sha256=sha(DEST.read_bytes()),generator_sha256=sha(Path(__file__).read_bytes()))
    DEST.with_name('2026-10-13-tree-frames-storage-v2.json').write_bytes(encode(report));print(json.dumps(report,sort_keys=True))
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('original',type=Path);main(p.parse_args().original)
