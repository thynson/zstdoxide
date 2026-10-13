#!/usr/bin/env python3
"""Freeze audited tree observations and retain complete raw capture provenance."""
from pathlib import Path
import argparse
import hashlib
import io
import json
import tarfile
import time
ROOT=Path(__file__).resolve().parents[1]
DEST=ROOT/'tests/fixtures/reference-tree-frame-capture'
AUDITOR='scripts/check_tree_frame_capture_v2.py'
AUDITOR_SHA='32ab218ed5376831605569dacb58ad0586a8d47994629c4e089fe7f5601c10f1'
def sha(b):return hashlib.sha256(b).hexdigest()
def file_sha(p):
    h=hashlib.sha256()
    with p.open('rb') as f:
        for b in iter(lambda:f.read(1<<20),b''):h.update(b)
    return h.hexdigest()
def need(x,why):
    if not x:raise ValueError(why)
def freeze(directory):
    need(not DEST.exists() and not DEST.is_symlink(),'fresh collection')
    need(sha((ROOT/AUDITOR).read_bytes())==AUDITOR_SHA,'frozen complete four-pass auditor')
    import check_tree_frame_capture_v2 as checker
    coverage,artifacts,summaries=checker.audit(directory)
    root=directory.resolve();e=json.loads((root/'evidence.json').read_bytes())
    need(type(e) is dict and e['status']=='passed','complete raw capture')
    archive=ROOT/'docs/evidence/2026-10-13-tree-frames.tar.gz';report_path=archive.with_name('2026-10-13-tree-frames.json')
    need(not archive.exists() and not report_path.exists(),'fresh raw archive/report')
    raw={}
    for p in sorted(root.rglob('*')):
        need(not p.is_symlink(),'no raw symlinks')
        if p.is_file():
            need(p.resolve().is_relative_to(root) and p.stat().st_size<=64<<20,'bounded raw file')
            raw[str(p.relative_to(root))]=dict(bytes=p.stat().st_size,sha256=file_sha(p))
    need(sum(v['bytes'] for v in raw.values())<=1<<30 and len(raw)<=5000,'bounded raw archive')
    source_names={AUDITOR,'scripts/check_tree_frame_capture.py','scripts/check_tree_frame_capture_controls.py','scripts/freeze_tree_frames.py','scripts/capture_tree_frames.py'}
    import capture_tree_frames as controller
    source_names.update(controller.PINS)
    source={n:dict(bytes=(ROOT/n).stat().st_size,sha256=file_sha(ROOT/n)) for n in sorted(source_names)}
    # Independent controls run concurrently with the expensive full replay.
    # Wait only for their explicit successful output, never infer success from
    # elapsed time. The caller runs this process asynchronously.
    deadline=time.monotonic()+600
    while True:
        ready=True
        for mode in ['normal','optimized']:
            p=Path('/tmp/zstdoxide-tree-frames-forgery-final-'+mode+'.log')
            raw_log=p.read_bytes() if p.is_file() else b''
            need(b'Traceback (most recent call last)' not in raw_log,'forgery control failed')
            try:result=json.loads(raw_log.splitlines()[-1])
            except (ValueError,IndexError):ready=False;continue
            need(result.get('status')=='passed' and result.get('rejections')==41,'explicit forgery completion')
        if ready:break
        need(time.monotonic()<deadline,'forgery control completion timeout')
        time.sleep(2)
    extras={}
    for mode in ['normal','optimized']:
        for stem in ['audit','audit-v2','forgery-final']:
            p=Path('/tmp/zstdoxide-tree-frames-'+stem+'-'+mode+'.log')
            need(p.is_file(),'retained audit/control log '+str(p));extras[p.name]=p.read_bytes()
    for mode in ['normal','optimized']:
        log=extras['zstdoxide-tree-frames-forgery-final-'+mode+'.log']
        control=json.loads(log.splitlines()[-1]);need(control['status']=='passed' and control['rejections']==41 and control['child_attempts']==0,'forgery gates')
        accepted=json.loads(extras['zstdoxide-tree-frames-audit-v2-'+mode+'.log'].splitlines()[-1]);need(checker.exact(accepted,coverage),'normal/optimized complete audits')
        need(b'aggregate log byte cap' in extras['zstdoxide-tree-frames-audit-'+mode+'.log'],'retained predecessor cap failure')
    snapshot=dict(raw=raw,source=source,logs={n:dict(bytes=len(b),sha256=sha(b)) for n,b in extras.items()})
    snapshot_bytes=(json.dumps(snapshot,sort_keys=True,indent=2)+'\n').encode()
    with tarfile.open(archive,'w:gz') as t:
        for prefix,base,items in [('raw',root,raw),('source',ROOT,source)]:
            for name,meta in items.items():
                p=base/name;need(file_sha(p)==meta['sha256'],'unchanged archive input');info=tarfile.TarInfo(prefix+'/'+name);info.size=meta['bytes'];info.mode=0o644
                with p.open('rb') as f:t.addfile(info,f)
        for name,b in {'snapshot.json':snapshot_bytes,**{'logs/'+n:b for n,b in extras.items()}}.items():
            info=tarfile.TarInfo(name);info.size=len(b);info.mode=0o644;t.addfile(info,io.BytesIO(b))
    expected={'raw/'+n:m for n,m in raw.items()}|{'source/'+n:m for n,m in source.items()}|{'logs/'+n:dict(bytes=len(b),sha256=sha(b)) for n,b in extras.items()}|{'snapshot.json':dict(bytes=len(snapshot_bytes),sha256=sha(snapshot_bytes))}
    seen=set()
    with tarfile.open(archive,'r|gz') as t:
        for m in t:
            need(m.isfile() and m.name in expected and m.name not in seen,'exact regular archive member');seen.add(m.name);h=hashlib.sha256();count=0;f=t.extractfile(m)
            for b in iter(lambda:f.read(1<<20),b''):count+=len(b);h.update(b)
            need(expected[m.name]==dict(bytes=count,sha256=h.hexdigest()),'complete archive bytes')
    need(seen==set(expected),'complete archive inventory')
    pack=bytearray();index={}
    for name,b in sorted(artifacts.items()):index[name]=dict(offset=len(pack),bytes=len(b),sha256=sha(b));pack.extend(b)
    need(len(pack)<=256<<20,'frozen artifact pack bound')
    manifest=dict(schema_version=1,upstream_revision=e['reference_revision'],checker_sha256=AUDITOR_SHA,controller_sha256=checker.CONTROLLER_SHA,coverage=coverage,cases=summaries,artifacts=index,pack_bytes=len(pack),pack_sha256=sha(pack),raw_archive=archive.name,raw_archive_sha256=file_sha(archive))
    DEST.mkdir()
    (DEST/'artifacts.bin').write_bytes(pack);(DEST/'capture.json').write_text(json.dumps(manifest,sort_keys=True,indent=2)+'\n')
    (DEST/'README.md').write_text('# Frozen ordinary binary-tree observations\n\n245 source-only recipes, normal/instrumented builds and two padding passes.\nThe canonical artifact pack retains one byte-identical observation/frame set;\nall raw commands, outputs and alternatives are in the named evidence archive.\nFifteen partial headers are not complete frames. Public admission is separate.\n')
    (DEST/'NOTICE').write_text('Observed with facebook/zstd v1.5.7, revision f8745da6ff1ad1e7bab384bd1f9d742439278e99.\nUpstream format/constants/source attribution: see LICENSE.zstd.\nOnly the pinned upstream executable is an oracle; Rust does not bind to C.\n')
    report=dict(status='passed',coverage=coverage,archive=archive.name,archive_bytes=archive.stat().st_size,archive_sha256=file_sha(archive),archive_members=len(expected),raw_files=len(raw),raw_bytes=sum(v['bytes'] for v in raw.values()),source=source,forgery_rejections_per_mode=41,collection_manifest_sha256=file_sha(DEST/'capture.json'),collection_pack_sha256=sha(pack),collection_pack_bytes=len(pack),qualification='All 1162 commands and four observation passes audited. v1 remains preserved with its 256 MiB aggregate-cap rejection; v2 uses 512 MiB while retaining 64 MiB stream bounds and all semantic/provenance rules. No public tuple resolution claim.')
    report_path.write_text(json.dumps(report,sort_keys=True,indent=2)+'\n');print(json.dumps(report|{'source':'retained in report'},sort_keys=True))
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('directory',type=Path);args=p.parse_args();freeze(args.directory)
