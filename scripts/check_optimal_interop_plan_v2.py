#!/usr/bin/env python3
"""Check exact public Optimal command-label inventory before native execution."""
from pathlib import Path
import argparse,hashlib,json,sys

def check():
    children=[]
    def guard(event,args):
        if event in ('subprocess.Popen','os.system','os.fork','os.posix_spawn'):
            children.append(event);raise ValueError('preflight forbids children')
    sys.addaudithook(guard)
    import capture_optimal_interop as capture
    import check_optimal_interop_capture_v2 as audit
    checker,generator,protocol,manifest=capture.inputs()
    for name,digest in audit.REVIEWED.items():
        p=checker.ROOT/name
        checker.require(p.is_file()and not p.is_symlink()and hashlib.sha256(p.read_bytes()).hexdigest()==digest,'reviewed capture/audit identity')
    paths=set(checker.ROOT/n for n in capture.PINS)
    paths.update((checker.ROOT/'src').rglob('*.rs'))
    paths.update(p for p in (checker.ROOT/'test-support').rglob('*') if p.suffix in ('.rs','.toml'))
    paths.update((checker.ROOT/'scripts/oracles').glob('*.py'))
    paths.update(checker.ROOT/n for n in ('Cargo.toml','Cargo.lock','rust-toolchain.toml','tests/upstream/manifest.json','scripts/capture_optimal_interop.py','scripts/check_reference_fast_interop.py','scripts/import_reference_fast_fuzz.py'))
    paths.update(checker.ROOT/n for n in generator.PINS);paths.add(checker.MANIFEST)
    names=sorted(str(p.relative_to(checker.ROOT)) for p in paths)
    def closure(values):
        checker.require(hashlib.sha256(json.dumps(values,sort_keys=True,separators=(',',':')).encode()).hexdigest()=='2d5c16173b3858bf94f4b2f628c9c831ac74d80770f6e8f5bf8404e5fcaaf232','complete implemented source inventory')
    closure(names)
    cases=generator.recipes();labels=['reference-revision','reference-status','compiler-version','compiler-target','rustc-version','rustc-path']
    for mode in ['normal','sanitized']:
        labels += [mode+'-dependencies',mode+'-predefines']+[f'{mode}-compile-{i}'for i in range(26)]+[mode+'-link']
    labels += ['rust-build-safe','rust-build-all']
    checker.require(len(labels)==66,'complete metadata/build inventory')
    negatives=protocol.negative_packets();checker.require(len(negatives)==39,'unchanged malformed inventory')
    for mode in ['normal','sanitized','safe','all']:
        labels += [mode+'-negative-'+name for name,_ in negatives]
    for index,case in enumerate(cases):
        prefix=f'case-{index:03}'
        labels += [prefix+'-rust-encode-'+mode for mode in ['safe','all']]
        if case.expected=='success':
            for mode in ['normal','sanitized']:
                labels += [prefix+'-c-'+mode+'-decode-'+rust for rust in ['safe','all']]
                labels.append(prefix+'-c-encode-'+mode)
                labels += [prefix+'-rust-'+rust+'-decode-'+mode for rust in ['safe','all']]
    checker.require(len(labels)==len(set(labels))==14350,'exact unique command labels')
    expected=dict(recipes=1269,admitted=1159,resource=83,unsupported=27,commands=14350,protocol_commands=14284,malformed_commands=156,decoded_frames_each_direction=4636,decoded_bytes=797537304,equivalence_groups=8)
    checker.require(manifest['recipe_count']==expected['recipes']and manifest['classifications']==dict(success=expected['admitted'],resource=expected['resource'],unsupported=expected['unsupported']),'fixed classification plan')
    checker.require(8*sum(len(c.source)for c in cases if c.expected=='success')==expected['decoded_bytes'],'independent decoded-byte inventory')
    checker.require(len(manifest['expected_rust_frame_equivalence'])==8,'exact equivalence plan')
    controller=Path(capture.__file__).read_text();auditor=Path(audit.__file__).read_text()
    for fragment in ['totals = dict(recipes=1269','(1159, 27, 83)','equivalence_groups=8','schema_version=6']:
        checker.require(fragment in controller,'reviewed controller totals: '+fragment)
    for fragment in ['len(records) == 14350','len(used) == 14284','recipes=1269','(1159, 27, 83, 4636, 4636, 797537304)']:
        checker.require(fragment in auditor,'reviewed auditor totals: '+fragment)
    rejected=[]
    def reject(name,call):
        try:call()
        except ValueError:rejected.append(name)
        else:raise ValueError('forgery accepted '+name)
    original=capture.PINS
    try:
        capture.PINS=dict(original);capture.PINS['scripts/oracles/optimal_interop.py']='0'*64
        reject('generator-pin',capture.inputs)
    finally:capture.PINS=original
    original_digest=checker.GENERATOR_SHA256
    try:
        checker.GENERATOR_SHA256='0'*64;reject('checker-generator-pin',checker.check)
    finally:checker.GENERATOR_SHA256=original_digest
    for raw in [b'{"x":1,"x":2}',b'{"x":NaN}',b'{"x":Infinity}']:
        reject('strict-json',lambda data=raw:checker.decode(data))
    for bad in [labels[:-1],labels+[labels[-1]],list(reversed(labels))]:
        reject('exact-labels',lambda value=bad:checker.require(value==labels,'exact command order'))
    for bad in [names[:-1], names+[names[-1]], [n for n in names if n!='src/optimal_plan.rs']]:
        reject('source-closure',lambda value=bad:closure(value))
    checker.require(not children and len(rejected)==11,'complete offline controls')
    encoded=(json.dumps(labels,indent=2)+'\n').encode()
    return dict(status='passed',source_closure_sha256='2d5c16173b3858bf94f4b2f628c9c831ac74d80770f6e8f5bf8404e5fcaaf232',source_paths=names,plan=expected,command_labels_sha256=hashlib.sha256(encoded).hexdigest(),reviewed_sha256=audit.REVIEWED,rejected=rejected,child_processes=0,qualification='Source-only exact command-label/count and reviewed-source preflight. Complete invocation/dependency/environment and response replay follows actual execution.'),encoded

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--output',type=Path,required=True);a=p.parse_args();r,labels=check();a.output.mkdir(exist_ok=False);(a.output/'labels.json').write_bytes(labels);(a.output/'evidence.json').write_text(json.dumps(r,indent=2)+'\n');print(json.dumps(r,sort_keys=True))
