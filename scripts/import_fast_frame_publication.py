#!/usr/bin/env python3
"""Check saved normal Fast frame evidence offline; explicitly regenerate with C.

Default/--check performs no compiler or subprocess operation. --upstream requires
an explicit fresh external --output; --sanitize additionally rebuilds all used
upstream C with ASan/UBSan. No action rewrites the saved fixture capture.
"""
from pathlib import Path
import argparse
import json
import sys

ROOT=Path(__file__).resolve().parents[1]
SUPPORT=ROOT/'scripts/oracles'
sys.path.insert(0,str(SUPPORT))
import fast_frame_publication_support as inspect

COLLECTION=ROOT/'tests/upstream/fast-frame-publication'
GENERATOR='tests/upstream/fast-frame-publication/frame_oracle.c'
GENERATOR_SHA256='d6d3d6219e5dbe9f66d79c6c5e2e2fd0d3626791293fae61c1c900097d57ce20'
DATA_SHA256='9f58da099048aad79883825e8fe8aeeaa19202e6c2af85817ce91e1210af65ec'
FRAME_SHA256='ab5e81455f2ac480a65b80a686a0fc2eae5f2eb41eedd4de97816ad6963b186d'
JSON_SHA256='56c899b0156a762a943f24e650072a4ad4354a0433d0c45313c69118e5a81a7b'
CAPTURE_SHA256='d1458675582213f7c31ab2b902e963997d2ad16fe9a75145a2f0071237b05b3a'
SOURCE_MAP_SHA256='9afb7ab18f5ed1a1e8407b56e2861d221f2df93c075782fac17a11d7e88941bb'
REPO_INPUTS={'scripts/oracles/fast_frame_publication_support.py','scripts/oracles/fast_frame_publication_live.py',
             'tests/upstream/fast-frame-publication/cases.rs'}


def digest(path):
    return inspect.sha(path.read_bytes())


def checked_path(base, relative):
    inspect.need(type(relative) is str and relative and not Path(relative).is_absolute(), 'relative manifest path')
    path=(base/relative).resolve()
    inspect.need(path.is_relative_to(base.resolve()), 'manifest path escapes owner')
    inspect.need(path.is_file() and not (base/relative).is_symlink(), 'regular manifest file')
    return path


def check(collection):
    collection=collection.resolve()
    manifest_path=collection/'manifest.json'
    inspect.need(manifest_path.is_file() and manifest_path.stat().st_size<=1<<20, 'manifest size cap')
    manifest=inspect.parse_json(manifest_path.read_bytes())
    inspect.shape(manifest, {'schema_version','upstream_revision','generator','generator_sha256',
                  'importer','importer_sha256','repository_input_sha256','source_sha256','fixtures','capture','coverage','policy'}, 'manifest')
    inspect.need(type(manifest['schema_version']) is int and manifest['schema_version']==1
                 and manifest['upstream_revision']==inspect.REVISION, 'manifest schema/revision')
    inspect.need(manifest['generator']==GENERATOR and manifest['generator_sha256']==GENERATOR_SHA256,
                 'fixed generator provenance')
    inspect.need(digest(collection/'frame_oracle.c')==GENERATOR_SHA256, 'unchanged captured C helper')
    inspect.need(manifest['importer']=='scripts/import_fast_frame_publication.py'
                 and manifest['importer_sha256']==digest(Path(__file__)), 'importer hash binding')
    inspect.need(inspect.sha(inspect.canonical(manifest['capture']))==CAPTURE_SHA256, 'fixed capture provenance')
    inspect.need(set(manifest['repository_input_sha256'])==REPO_INPUTS, 'support source set')
    for relative,expected in manifest['repository_input_sha256'].items():
        inspect.need(digest(checked_path(ROOT,relative))==expected, 'support source hash binding')
    inspect.need(inspect.sha(inspect.canonical(manifest['source_sha256']))==SOURCE_MAP_SHA256,
                 'fixed complete upstream source-hash map')
    inspect.need(type(manifest['fixtures']) is list and len(manifest['fixtures'])==2, 'two packed data artifacts')
    fixture=manifest['fixtures'][0]
    inspect.shape(fixture, {'path','size','sha256','uncompressed_size','uncompressed_sha256'}, 'fixture metadata')
    inspect.need(fixture['path']=='observations.json.gz' and fixture['sha256']==DATA_SHA256
                 and fixture['uncompressed_sha256']==JSON_SHA256, 'fixed data provenance')
    data=checked_path(collection,fixture['path'])
    inspect.need(type(fixture['size']) is int and fixture['size']==data.stat().st_size<=inspect.MAX_COMPRESSED_BYTES,
                 'bounded compressed artifact size')
    raw,value=inspect.unpack(data.read_bytes())
    inspect.need(digest(data)==DATA_SHA256 and inspect.sha(raw)==JSON_SHA256, 'captured artifact hashes')
    inspect.need(type(fixture['uncompressed_size']) is int and len(raw)==fixture['uncompressed_size'], 'expanded artifact size')
    frame=manifest['fixtures'][1]
    inspect.shape(frame, {'path','size','sha256'}, 'binary fixture metadata')
    inspect.need(frame['path']=='frames.bin' and frame['sha256']==FRAME_SHA256, 'fixed binary frame provenance')
    path=checked_path(collection,frame['path'])
    inspect.need(type(frame['size']) is int and frame['size']==path.stat().st_size<=3<<20, 'binary frame cap')
    frames=path.read_bytes()
    inspect.need(inspect.sha(frames)==FRAME_SHA256, 'binary frame pack identity')
    inspect.scalar_types(value)
    inspect.need((collection/'cases.rs').read_bytes()==inspect.rust_index(value), 'generated Rust frame index')
    value=inspect.materialize(value,frames)
    coverage=inspect.validate_dataset(value)
    inspect.need(manifest['coverage']=={k:v for k,v in coverage.items() if k!='cases'}, 'exact coverage metadata')
    return manifest,value,coverage


def overlap(a,b):
    return a==b or a.is_relative_to(b) or b.is_relative_to(a)


def pack_capture(directory):
    artifacts=directory/'artifacts'
    value={'schema_version':1,'upstream_revision':inspect.REVISION,'cases':[],'headers':[]}
    for row in inspect.parse_json((artifacts/'recipes.json').read_bytes()):
        name=row['id']
        value['cases'].append({'recipe':row,'records':inspect.parse_json((artifacts/(name+'.records.json')).read_bytes()),
                 'frame_hex':(artifacts/(name+'.zst')).read_bytes().hex(),
                 'whole_frame_hex':(artifacts/(name+'.whole.zst')).read_bytes().hex() if row['whole_compare'] else None})
    for i in range(11): value['headers'].append(inspect.parse_json((artifacts/f'header-{i:02d}.json').read_bytes()))
    inspect.validate_dataset(value)
    return value


def regenerate(collection,reference,output,sanitize,compiler,saved):
    reference=reference.resolve(); output=output.resolve(); collection=collection.resolve()
    for protected in (ROOT.resolve(),reference,collection):
        inspect.need(not overlap(output,protected), 'external output overlaps repository/reference/collection')
    inspect.need(not output.exists(), 'regeneration requires a fresh output directory')
    inspect.need(reference.is_dir() and not overlap(reference,ROOT.resolve()), 'external pinned reference required')
    # Importing this module is reserved for the explicit regeneration branch.
    import fast_frame_publication_live as live
    output.mkdir(parents=True)
    live.HERE=output; live.COLLECTION=collection; live.INPUT_HASHES=live.source_inputs()
    before={p.name:digest(p) for p in collection.iterdir() if p.is_file()}
    report={'result':'running','reference':str(reference),'output':str(output),'source_inputs':live.INPUT_HASHES,
            'commands':[],'comparisons':[],'saved_fixture_before':before}
    def save(): (output/'regeneration.json').write_bytes(inspect.canonical(report))
    save()
    try:
        modes=['normal','sanitized'] if sanitize else ['normal']
        for mode in modes:
            build=output/('build-'+mode); capture=output/('capture-'+mode)
            live.build(reference,build,mode,compiler)
            live.capture(reference,build,capture)
            regenerated=pack_capture(capture)
            inspect.need(inspect.normalize_host(regenerated)==inspect.normalize_host(saved),
                         mode+' exact saved frame/state/header comparison')
            comparison={'mode':mode,'complete_frame_decodes':27,'decoded_bytes':2275346,'partial_headers':11,
                        'all_logical_records_and_frame_bytes_equal':True,
                        'observed_pointer_widths':sorted({r[1]['pointer_bits'] for r in regenerated['headers']}),
                        'capture_sha256':digest(capture/'capture.json'),'build_sha256':digest(build/'build.json')}
            report['comparisons'].append(comparison); save()
        after={p.name:digest(p) for p in collection.iterdir() if p.is_file()}
        inspect.need(before==after and live.INPUT_HASHES==live.source_inputs(), 'saved fixtures/source changed')
        report.update(result='passed',saved_fixture_after=after)
    except BaseException as error:
        report.update(result='failed',failure_type=type(error).__name__,failure=str(error))
        raise
    finally:
        report['commands']=[dict(path=str(p.relative_to(output)),sha256=digest(p)) for p in sorted(output.rglob('*.command.json'))]
        save()
    return report


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check',action='store_true',help='offline hash and semantic validation (also the default)')
    parser.add_argument('--collection',type=Path,default=COLLECTION)
    parser.add_argument('--upstream',type=Path,help='explicit clean built v1.5.7 checkout for C regeneration')
    parser.add_argument('--output',type=Path,help='fresh external directory; never overlaps protected paths')
    parser.add_argument('--sanitize',action='store_true',help='also rebuild all used C with ASan/UBSan')
    parser.add_argument('--compiler',help='explicit compiler for regeneration only')
    args=parser.parse_args()
    if args.check and (args.upstream is not None or args.output is not None or args.sanitize or args.compiler is not None): parser.error('--check is strictly offline and rejects regeneration selectors')
    if args.upstream is None and (args.output is not None or args.sanitize or args.compiler is not None): parser.error('--output/--sanitize requires --upstream')
    if args.upstream is not None and args.output is None: parser.error('--upstream requires fresh --output')
    manifest,value,coverage=check(args.collection)
    if args.upstream is not None:
        report=regenerate(args.collection,args.upstream,args.output,args.sanitize,args.compiler or 'clang',value)
        print(json.dumps({'result':report['result'],'comparisons':report['comparisons'],'evidence':str(args.output/'regeneration.json')},sort_keys=True))
    else:
        print(json.dumps({'result':'passed','mode':'offline-no-compiler-or-process','coverage':{k:v for k,v in coverage.items() if k!='cases'}},sort_keys=True))


if __name__=='__main__':
    main()
