#!/usr/bin/env python3
"""Project fixed C Greedy trajectories and frames; never run a codec."""
from pathlib import Path
import argparse
import hashlib
import json
import struct

ROOT=Path(__file__).resolve().parents[1]
DESTINATION=ROOT/'tests/fixtures/reference-greedy-frames'
CHECKER='scripts/check_greedy_frames.py'
CHECKER_SHA='3c82513af6e9ab804b40be9b37781b6849f5b0eb3031edbfcc08f340d5f36e2c'


def require(value,reason):
    if not value:raise ValueError(reason)


def sha(data):return hashlib.sha256(data).hexdigest()


def blob(value):return struct.pack('<I',len(value))+value


def sampling(value):
    return struct.pack('<4IQ8I',value['next_to_update'],value['skipping'],value['row_hash_log'],value['salt_entropy'],
                       int(value['salt'],16),*value['cache'])


def projection():
    require(sha((ROOT/CHECKER).read_bytes())==CHECKER_SHA,'frozen ordinary collection checker')
    import check_greedy_frames as checker
    manifest,artifacts=checker.check()
    import check_greedy_frame_capture as capture
    _,_,_,validator,generator,cases=capture.inputs()
    result=bytearray(b'ZGRFR01\0'+struct.pack('<II',len(cases),manifest['coverage']['positive_blocks']))
    for case in cases:
        rows=[validator.parse_json(line) for line in artifacts[case.name+'.stdout'].splitlines()]
        begin=rows[0]; complete=next(r for r in rows if r['type']=='complete')
        whole=next((r for r in rows if r['type']=='whole_compare'),None)
        name=case.name.encode('ascii')
        body=bytearray(struct.pack('<H',len(name))+name)
        body.extend(struct.pack('<8BI',case.window_log,case.hash_log,case.chain_log,case.search_log,case.min_match,
                                int(case.window_log>14),0,0,case.target))
        body.extend(blob(case.source));body.extend(struct.pack('<Q',int(begin['source_fnv64'],16)))
        body.extend(blob(artifacts[case.name+'.zst']));body.extend(struct.pack('<Q',int(complete['frame_fnv64'],16)))
        body.append(int(whole is not None))
        if whole:
            body.extend(blob(artifacts[case.name+'.whole.zst']));body.extend(struct.pack('<Q',int(whole['frame_fnv64'],16)))
        body.extend(struct.pack('<I',len(case.schedule)))
        for size in case.schedule:body.extend(struct.pack('<I',size))
        blocks=[r for r in rows if r['type']=='block'];body.extend(struct.pack('<I',len(blocks)))
        for block in blocks:
            candidate=block['candidate']
            record=bytearray(struct.pack('<4IBBH',block['call'],block['source_start'],block['source_bytes'],block['window']['dict_limit']-2,
                                         block['kind'],int(candidate['fresh']),0))
            for repeats in (block['semantic_before'],block['semantic_after'],block['before']['raw_repeats'],block['after']['raw_repeats']):
                record.extend(struct.pack('<3I',*repeats))
            record.extend(sampling(block['sampling_before']));record.extend(sampling(block['sampling_after']))
            for key in ('first_table','second_table'):
                table=block[key];record.extend(struct.pack('<BBHI',table['log'],table['element_bytes'],0,len(table['changed'])))
                for change in table['changed']:record.extend(struct.pack('<3I',*change))
                record.extend(struct.pack('<2Q',int(table['before_fnv64'],16),int(table['after_fnv64'],16)))
            if candidate['fresh']:
                record.extend(struct.pack('<3I',*block['candidate_state']['raw_repeats']))
                record.extend(struct.pack('<3I',*candidate['semantic_repeats']))
                record.extend(struct.pack('<IB3xI',candidate['trailing'],candidate['long_length_type'],candidate['long_length_position']))
                record.extend(blob(bytes.fromhex(candidate['literal_bytes'])))
                record.extend(struct.pack('<I',len(candidate['sequences'])))
                for ll,ml,offset,distance,raw_ll,raw_ml in candidate['sequences']:
                    record.extend(struct.pack('<6I',ll,ml,distance,offset,raw_ll,raw_ml))
            body.extend(blob(record))
        result.extend(blob(body))
    data=bytes(result)
    require(len(data)<=64<<20,'bounded packed trajectories')
    provenance={'schema_version':1,'binary_format':'ZGRFR01: checked little-endian ordinary Greedy trajectories and complete C frames',
                'upstream_revision':generator.REVISION,'cases':138,'positive_blocks':395,'source_bytes':7365784,'complete_frames':151,
                'excluded_partial_headers':15,'generator':str(Path(__file__).relative_to(ROOT)),'generator_sha256':sha(Path(__file__).read_bytes()),
                'checker':CHECKER,'checker_sha256':CHECKER_SHA,'source_manifest_sha256':checker.MANIFEST_SHA,
                'source_pack_sha256':manifest['pack_sha256'],'fixture':{'path':'cases.bin','bytes':len(data),'sha256':sha(data)},
                'policy':'Test-only projection of frozen C inputs/observations. No codec execution. Full entropy/owner/cache observations and zero-call records remain in the original collection. Sampling caches are preserved, not a requirement for Rust caching.'}
    return {'cases.bin':data,'provenance.json':(json.dumps(provenance,sort_keys=True,indent=2)+'\n').encode()}


def publish(files,destination=DESTINATION):
    # Check all destinations before creating any output; never overwrite a prior
    # expectation or follow an existing symlink through a destination directory.
    require(not destination.exists() and not destination.is_symlink(),'fresh projection destination required')
    require(set(files)=={'cases.bin','provenance.json'},'exact projection output names')
    destination.mkdir(parents=True)
    for name,data in files.items():
        with (destination/name).open('xb') as stream:stream.write(data)


def main():
    parser=argparse.ArgumentParser(description=__doc__);action=parser.add_mutually_exclusive_group(required=True)
    action.add_argument('--write',action='store_true');action.add_argument('--check',action='store_true')
    args=parser.parse_args();files=projection()
    if args.write:publish(files)
    else:
        for name,data in files.items():require((DESTINATION/name).read_bytes()==data,'immutable projected bytes: '+name)
    print(json.dumps({'cases':138,'blocks':395,'frames':151,'packed_bytes':len(files['cases.bin']),'packed_sha256':sha(files['cases.bin']),'codec_executions':0},sort_keys=True))


if __name__=='__main__':main()
