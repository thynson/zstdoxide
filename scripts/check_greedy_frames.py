#!/usr/bin/env python3
"""Replay immutable ordinary Greedy frame/state observations without execution."""
from pathlib import Path
import argparse
import hashlib
import json

ROOT = Path(__file__).resolve().parents[1]
COLLECTION = ROOT / 'tests/fixtures/reference-greedy-frame-capture'
CHECKER = 'scripts/check_greedy_frame_capture.py'
CHECKER_SHA = '4ee3c35c99544410b280360da4b7150210d67f99335dd38268f82704a2577f1b'
MANIFEST_SHA = '4a6c6b89452f0c22a7f4d9fe9e45255e48c21ab259cfce62a4981e513e63560e'


def require(value, reason):
    if not value:
        raise ValueError(reason)


def check(collection=COLLECTION):
    require(hashlib.sha256((ROOT/CHECKER).read_bytes()).hexdigest()==CHECKER_SHA,'frozen offline capture auditor')
    import check_greedy_frame_capture as checker
    _,_,_,v,generator,cases=checker.inputs()
    root=collection.resolve()
    require(root.is_dir() and {p.name for p in root.iterdir()}=={'capture.json','artifacts.bin','README.md','NOTICE'},'exact frozen collection')
    raw=checker.read(root,'capture.json',2<<20)
    require(hashlib.sha256(raw).hexdigest()==MANIFEST_SHA,'frozen accepted capture identity')
    manifest=v.parse_json(raw)
    require(manifest['upstream_revision']==generator.REVISION and manifest['checker_sha256']==CHECKER_SHA
            and manifest['controller_sha256']==checker.CONTROLLER_SHA,'source provenance')
    pack=checker.read(root,'artifacts.bin',64<<20)
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
        s=v.validate_case(c,records(c.name+'.stdout'),frame,whole)
        s.update(stdout_sha256=hashlib.sha256(artifacts[c.name+'.stdout']).hexdigest(),frame_sha256=hashlib.sha256(frame).hexdigest(),frame_bytes=len(frame),
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
