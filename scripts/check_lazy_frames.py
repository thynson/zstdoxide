#!/usr/bin/env python3
"""Replay immutable ordinary lazy-family frame/state observations without execution."""
from pathlib import Path
import argparse
import hashlib
import json

ROOT = Path(__file__).resolve().parents[1]
COLLECTION = ROOT / 'tests/fixtures/reference-lazy-frame-capture'
CHECKER = 'scripts/check_lazy_frame_capture.py'
CHECKER_SHA = 'bbfdd37373f29f86564be9a533636fc9d55e4b10cecc17d1d0a016d39b1be584'
MANIFEST_SHA = '0c9579981f10ec11097fca94f2c3a75223674083d80feeee83ef70bcaf302740'


def require(value, reason):
    if not value:
        raise ValueError(reason)


def check(collection=COLLECTION):
    require(hashlib.sha256((ROOT/CHECKER).read_bytes()).hexdigest()==CHECKER_SHA,'frozen offline capture auditor')
    import check_lazy_frame_capture as checker
    _,_,_,v,generator,cases=checker.inputs()
    root=collection.resolve()
    require(root.is_dir() and {p.name for p in root.iterdir()}=={'capture.json','artifacts.bin','README.md','NOTICE'},'exact frozen collection')
    raw=checker.read(root,'capture.json',2<<20)
    require(hashlib.sha256(raw).hexdigest()==MANIFEST_SHA,'frozen accepted capture identity')
    manifest=v.parse_json(raw)
    require(manifest['upstream_revision']==generator.REVISION and manifest['checker_sha256']==CHECKER_SHA
            and manifest['controller_sha256']==checker.CONTROLLER_SHA,'source provenance')
    pack=checker.read(root,'artifacts.bin',256<<20)
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
    predecessors=checker.inputs()[0].predecessor_artifacts()
    summaries=[]
    for c in cases:
        frame=artifacts[c.name+'.zst'];whole=artifacts.get(c.name+'.whole.zst')
        observed=records(c.name+'.stdout')
        s=v.validate_case(c,observed,frame,whole)
        if c.predecessor is not None:
            name=c.predecessor
            old=[v.parse_json(line) for line in predecessors[name+'.stdout'].splitlines()]
            require(checker.exact(v.predecessor_records(observed,name),old),'unchanged complete predecessor observations')
            require(frame==predecessors[name+'.zst'],'unchanged complete predecessor frame')
            if whole is not None:require(whole==predecessors[name+'.whole.zst'],'unchanged predecessor whole frame')
        s.update(stdout_sha256=hashlib.sha256(artifacts[c.name+'.stdout']).hexdigest(),frame_sha256=hashlib.sha256(frame).hexdigest(),frame_bytes=len(frame),
                 whole_sha256=hashlib.sha256(whole).hexdigest() if whole else None,whole_bytes=len(whole) if whole else None)
        summaries.append(s)
    for i,(pledge,window,depth) in enumerate(generator.HEADERS):v.validate_partial(records('header-'+str(i)+'.stdout'),pledge,window,depth)
    require(checker.exact(summaries,manifest['cases']),'complete frozen replay summaries')
    return manifest, artifacts


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check',required=True,action='store_true')
    parser.add_argument('--collection',type=Path,default=COLLECTION)
    args=parser.parse_args();manifest,_=check(args.collection)
    print(json.dumps(manifest['coverage'],sort_keys=True))


if __name__=='__main__':main()
