#!/usr/bin/env python3
"""Losslessly archive a completed larger retained capture with full hash replay."""
import argparse,gzip,hashlib,json,tarfile
from pathlib import Path
from oracles.optimal_retained_frames import require
ROOT=Path(__file__).resolve().parents[1]

def sha(path):
 h=hashlib.sha256()
 with path.open('rb')as stream:
  for block in iter(lambda:stream.read(1<<20),b''):h.update(block)
 return h.hexdigest()

def archive(root):
 root=root.resolve(strict=True);e=json.loads((root/'evidence.json').read_text())
 require(e['status']=='passed' and e['source_unchanged']is True and e['audit']['recipes']==19,'complete independently audited capture')
 require(all(c['exit_code']==c['expected_exit']for c in e['commands']),'complete expected outcomes')
 for mode in ['normal','optimized']:
  v=json.loads((root/'qualification'/('controls-'+mode+'.json')).read_text())
  require(v['status']=='passed' and v['positives']==2 and v['rejected']==75 and v['child_processes']==0,'complete offline controls')
 for name,digest in e['source'].items():require(sha(ROOT/name)==sha(root/'source'/name)==digest,'unchanged executed source')
 paths=sorted(p for p in root.rglob('*')if p.is_file());files=[];total=0
 for p in paths:
  require(not p.is_symlink() and p.stat().st_size<=256<<20,'regular bounded member')
  total+=p.stat().st_size;require(total<=1<<30,'bounded expanded archive')
  files.append(dict(path=p.relative_to(root).as_posix(),bytes=p.stat().st_size,sha256=sha(p)))
 dest=ROOT/'docs/evidence/2026-10-20-optimal-retained-frames-capture.tar.gz';report=dest.with_suffix('').with_suffix('.json')
 require(not dest.exists() and not report.exists(),'fresh archive and report')
 with dest.open('xb')as raw,gzip.GzipFile(filename='',mode='wb',fileobj=raw,mtime=0)as zipped,tarfile.open(fileobj=zipped,mode='w|')as tar:
  for p,row in zip(paths,files,strict=True):
   info=tarfile.TarInfo(row['path']);info.size=row['bytes'];info.mode=0o644
   with p.open('rb')as stream:tar.addfile(info,stream)
 with tarfile.open(dest,'r|gz')as tar:
  for row in files:
   entry=tar.next();require(entry is not None and entry.isfile() and entry.name==row['path'] and entry.size==row['bytes'],'exact archive inventory')
   h=hashlib.sha256()
   stream=tar.extractfile(entry)
   for b in iter(lambda:stream.read(1<<20),b''):h.update(b)
   require(h.hexdigest()==row['sha256'],'independent archived bytes')
  require(tar.next()is None,'exact archive EOF')
 require(all(sha(p)==row['sha256'] for p,row in zip(paths,files,strict=True)),'unchanged originals')
 result=dict(schema_version=1,status='passed',archive=dest.name,archive_bytes=dest.stat().st_size,archive_sha256=sha(dest),files=files,expanded_bytes=total,qualification=e['audit'],archive_driver_sha256=sha(Path(__file__)))
 report.write_text(json.dumps(result,sort_keys=True,indent=2)+'\n');print(json.dumps({k:v for k,v in result.items()if k not in ('files','qualification')},sort_keys=True))
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('capture',type=Path);archive(p.parse_args().capture)
