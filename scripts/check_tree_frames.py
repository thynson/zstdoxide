#!/usr/bin/env python3
"""Replay immutable ordinary binary-tree frame/state observations without execution."""
from pathlib import Path
import argparse
import hashlib
import json
import gzip
import io

ROOT = Path(__file__).resolve().parents[1]
COLLECTION = ROOT / 'tests/fixtures/reference-tree-frame-capture'
CHECKER = 'scripts/check_tree_frame_capture_v2.py'
CHECKER_SHA = '32ab218ed5376831605569dacb58ad0586a8d47994629c4e089fe7f5601c10f1'
MANIFEST_SHA = '63e7b7c852e01a3618968e162f018271c062367e64c429ab4fe22bef70d9f01d'


def require(value, reason):
    if not value:
        raise ValueError(reason)


def check(collection=COLLECTION):
    require(hashlib.sha256((ROOT/CHECKER).read_bytes()).hexdigest()==CHECKER_SHA,'frozen offline capture auditor')
    import check_tree_frame_capture_v2 as checker
    _,_,_,v,generator,cases=checker.inputs()
    root=collection.resolve()
    require(root.is_dir() and {p.name for p in root.iterdir()}=={'capture.json','artifacts.bin.gz','README.md','NOTICE'},'exact frozen collection')
    raw=checker.read(root,'capture.json',2<<20)
    require(hashlib.sha256(raw).hexdigest()==MANIFEST_SHA,'frozen accepted capture identity')
    manifest=v.parse_json(raw)
    require(manifest['upstream_revision']==generator.REVISION and manifest['checker_sha256']==CHECKER_SHA
            and manifest['controller_sha256']==checker.CONTROLLER_SHA,'source provenance')
    compressed=checker.read(root,'artifacts.bin.gz',64<<20)
    require(hashlib.sha256(compressed).hexdigest()=='74edf1be33c9e1e062e1739fdf002979b1bdac4f39a254094607ee8e77bb0e15','frozen compressed storage')
    with gzip.GzipFile(fileobj=io.BytesIO(compressed)) as stream:
        pack=stream.read((256<<20)+1)
    require(len(pack)<=256<<20,'bounded decompressed pack')
    require(len(pack)==manifest['pack_bytes'] and hashlib.sha256(pack).hexdigest()==manifest['pack_sha256'],'complete frozen observation/frame pack')
    names={'metadata.stdout'}|{'header-'+str(i)+'.stdout' for i in range(len(generator.HEADERS))}
    for c in cases:
        names|={c.name+'.stdout',c.name+'.zst'}
        if c.whole_compare: names.add(c.name+'.whole.zst')
    require(set(manifest['artifacts'])==names,'complete frozen artifact inventory')
    cursor=0; artifacts={}
    for name in sorted(names):
        item=manifest['artifacts'][name]
        require(type(item['offset']) is int and item['offset']==cursor and type(item['bytes']) is int
                and 0<=item['bytes']<=len(pack)-cursor,'consecutive checked pack extent')
        data=pack[cursor:cursor+item['bytes']];cursor+=item['bytes']
        require(hashlib.sha256(data).hexdigest()==item['sha256'],'artifact extent hash')
        artifacts[name]=data
    require(cursor==len(pack),'no unused pack bytes')
    def records(name):return [v.parse_json(line) for line in artifacts[name].splitlines()]
    metadata=records('metadata.stdout');require(len(metadata)==1,'metadata count');v.validate_metadata(metadata[0])
    summaries=[]
    for c in cases:
        frame=artifacts[c.name+'.zst'];whole=artifacts.get(c.name+'.whole.zst')
        observed=records(c.name+'.stdout')
        s=v.validate_case(c,observed,frame,whole)
        s.update(padding=106,stdout_sha256=hashlib.sha256(artifacts[c.name+'.stdout']).hexdigest(),frame_sha256=hashlib.sha256(frame).hexdigest(),frame_bytes=len(frame),
                 whole_sha256=hashlib.sha256(whole).hexdigest() if whole else None,whole_bytes=len(whole) if whole else None)
        summaries.append(s)
    for i,(pledge,window) in enumerate(generator.HEADERS):v.validate_partial(records('header-'+str(i)+'.stdout'),pledge,window)
    require(checker.exact(summaries,manifest['cases']),'complete frozen replay summaries')
    return manifest, artifacts


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check',required=True,action='store_true')
    parser.add_argument('--collection',type=Path,default=COLLECTION)
    args=parser.parse_args();manifest,_=check(args.collection)
    print(json.dumps(manifest['coverage'],sort_keys=True))


if __name__=='__main__':main()
