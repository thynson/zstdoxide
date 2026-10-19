#!/usr/bin/env python3
"""Retain a complete audited ordinary Ultra campaign with verified regular members."""
import argparse
import gzip
import hashlib
import json
from pathlib import Path
import tarfile
from oracles.ultra_scalar import need, load, sha

ROOT = Path(__file__).resolve().parents[1]


def file_sha(path):
    digest=hashlib.sha256()
    with path.open('rb') as stream:
        for chunk in iter(lambda:stream.read(1<<20),b''):digest.update(chunk)
    return digest.hexdigest()


def archive(capture):
    capture=capture.resolve(strict=True)
    evidence=load((capture/'evidence.json').read_bytes())
    verdict=load((capture/'independent-audit-v2.json').read_bytes())
    need(verdict['status']=='passed' and evidence['source_unchanged']is True
         and verdict['audit']['observed_events_per_build']==396952
         and verdict['capture_evidence_sha256']==file_sha(capture/'evidence.json')
         and verdict['auditor_sha256']==file_sha(ROOT/'scripts/check_ultra_fresh_capture_v2.py'), 'complete successor audit identity')
    dest=ROOT/'docs/evidence/2026-10-19-ultra-fresh-capture.tar.gz'
    report=dest.with_name('2026-10-19-ultra-fresh-capture.json')
    need(not dest.exists() and not report.exists(), 'fresh capture archive')
    files=[];total=0
    for path in sorted(capture.rglob('*')):
        name=path.relative_to(capture).as_posix()
        if name=='special' or name.startswith('special/'):continue
        if path.is_dir():continue
        need(path.is_file() and not path.is_symlink(), 'regular capture member')
        size=path.stat().st_size;total+=size
        need(size<=256<<20 and total<=3<<30, 'bounded expanded archive')
        files.append(dict(path=name,bytes=size,sha256=file_sha(path)))
    with dest.open('xb') as raw, gzip.GzipFile(filename='',mode='wb',fileobj=raw,mtime=0) as zipped, tarfile.open(fileobj=zipped,mode='w|',format=tarfile.PAX_FORMAT) as tar:
        for row in files:
            info=tarfile.TarInfo(row['path']);info.size=row['bytes'];info.mode=0o644;info.mtime=0
            with (capture/row['path']).open('rb') as stream:tar.addfile(info,stream)
    with tarfile.open(dest,'r|gz') as tar:
        for row in files:
            member=tar.next();need(member is not None and member.isfile() and member.name==row['path'] and member.size==row['bytes'], 'exact archive member')
            digest=hashlib.sha256();stream=tar.extractfile(member)
            for chunk in iter(lambda:stream.read(1<<20),b''):digest.update(chunk)
            need(digest.hexdigest()==row['sha256'], 'verified archived bytes')
        need(tar.next()is None, 'exact archive EOF')
    result=dict(status='passed',archive=dest.name,archive_bytes=dest.stat().st_size,archive_sha256=file_sha(dest),
        expanded_bytes=total,files=files,qualification=verdict['audit'],
        archiver_sha256=sha(Path(__file__).read_bytes()), auditor_sha256=verdict['auditor_sha256'],
        original_capture_status=evidence['status'], original_capture_error=evidence.get('error'),
        omitted_inputs='Only the special/ descriptor-control directory is omitted: one directory, one FIFO, one local request symlink and an intentionally missing path. Their exact creation code, invocation rows and diagnostic logs are retained; all regular requests, sources, generated instrumentation, build products, frames and ledgers are archived.')
    report.write_text(json.dumps(result,sort_keys=True,indent=2)+'\n')
    print(json.dumps(dict(status='passed',members=len(files),archive_bytes=result['archive_bytes'],archive_sha256=result['archive_sha256'])))


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('capture',type=Path);archive(p.parse_args().capture)
