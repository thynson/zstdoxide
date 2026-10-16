#!/usr/bin/env python3
"""Freeze, qualify and measure complete private frequency lifecycles."""
from pathlib import Path
import argparse,hashlib,json,os,platform,shutil,subprocess,sys,time
sys.dont_write_bytecode=True
from check_ultra_frequency_benchmark import ROOT,expectations,parse,check
from oracles.ultra_scalar import need,load,sha
BACKENDS=(('safe',[]),('frequencies',['--features','unsafe-optimal-frequencies']),('all',['--all-features']))

def sources():
    paths=[p for folder in ('src','tests','examples','test-support')for p in (ROOT/folder).rglob('*.rs')]
    paths += list((ROOT/'scripts/oracles').glob('*.py'))
    paths += [ROOT/p for p in ['Cargo.toml','Cargo.lock','rust-toolchain.toml','test-support/allocator/Cargo.toml','scripts/check_ultra_frequency_benchmark.py','scripts/benchmark_ultra_frequencies.py','tests/fixtures/reference-ultra-frequencies/cases.bin','tests/fixtures/reference-ultra-frequencies/provenance.json']]
    return {str(p.relative_to(ROOT)):sha(p.read_bytes())for p in sorted(paths)}

def environment():
    result={k:v for k,v in os.environ.items()if k in ['PATH','HOME','TMPDIR','CARGO_HOME','RUSTUP_HOME','SDKROOT','DEVELOPER_DIR']}
    result.update(RUSTUP_TOOLCHAIN='nightly-2026-10-08',CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_TARGET_DIR='/tmp/zstdoxide-ultra-frequency-benchmark-target');return result

def run(out,label,argv,env):
    start=time.monotonic();stdout=out/(label+'.stdout');stderr=out/(label+'.stderr')
    with stdout.open('wb')as a,stderr.open('wb')as b:r=subprocess.run(argv,cwd=ROOT,env=env,stdout=a,stderr=b,timeout=600)
    record=dict(label=label,argv=argv,exit_code=r.returncode,seconds=time.monotonic()-start,stdout=stdout.name,stderr=stderr.name,stdout_sha256=sha(stdout.read_bytes()),stderr_sha256=sha(stderr.read_bytes()))
    need(r.returncode==0,'command failed '+label);return record

def prepare(out):
    need(not out.exists(),'fresh preparation output');out.mkdir();source=sources();env=environment();expected=expectations();(out/'expected.json').write_text(json.dumps(expected,indent=2)+'\n');records=[];binaries=[]
    report=dict(status='running',source=source,environment=env,platform=platform.platform(),expected_sha256=sha((out/'expected.json').read_bytes()),commands=records,binaries=binaries)
    def save():(out/'evidence.json').write_text(json.dumps(report,indent=2)+'\n')
    save()
    try:
        records.append(run(out,'rustc',['rustc','--version','--verbose'],env));records.append(run(out,'cargo',['cargo','--version'],env))
        for name,flags in BACKENDS:
            print('BUILD',name,flush=True);row=run(out,'build-'+name,['cargo','build','--offline','--locked','--release','--example','ultra_frequency_benchmark',*flags,'--message-format=json'],env);records.append(row)
            artifacts=[r for r in parse((out/row['stdout']).read_bytes())if r.get('reason')=='compiler-artifact'and r.get('target',{}).get('name')=='ultra_frequency_benchmark'and r.get('executable')]
            need(len(artifacts)==1,'exact executable artifact');binary=out/('benchmark-'+name);shutil.copy2(artifacts[0]['executable'],binary);binaries.append(dict(name=name,path=binary.name,sha256=sha(binary.read_bytes()),features=flags))
            row=run(out,'qualify-'+name,[str(binary)],env);records.append(row);check(parse((out/row['stdout']).read_bytes()),expected,name!='safe',False);save()
        need(sources()==source,'source unchanged');report.update(status='passed',source_unchanged=True);save()
    except BaseException as error:report.update(status='failed',error=repr(error));save();raise

def measure(prepared,out,repeat,barriers):
    need(not out.exists(),'fresh measurement output');prior=load((prepared/'evidence.json').read_bytes());need(prior['status']=='passed'and prior['source']==sources(),'qualified unchanged source')
    expected=expectations();need(expected==load((prepared/'expected.json').read_bytes())and sha((prepared/'expected.json').read_bytes())==prior['expected_sha256'],'independent complete states')
    for row in prior['commands']:
        need(row['exit_code']==0 and all(sha((prepared/row[k]).read_bytes())==row[k+'_sha256']for k in ['stdout','stderr']),'preparation closure')
    for binary in prior['binaries']:need(sha((prepared/binary['path']).read_bytes())==binary['sha256'],'immutable executable')
    need(len(barriers)==3 and len(set(barriers))==3,'matrix/platform/thumb completion barriers')
    completed=[]
    for path in barriers:
        data=path.read_bytes();doc=load(data);need(doc['status']=='passed'and doc['source_unchanged'],'completed known worker')
        need(all(prior['source'].get(p)==h for p,h in doc['source'].items()),'same qualified worker source')
        completed.append(dict(path=str(path),sha256=sha(data),data=doc))
    out.mkdir();(out/'barriers.json').write_text(json.dumps(completed,indent=2)+'\n');records=[];report=dict(status='running',source=prior['source'],prepared_evidence_sha256=sha((prepared/'evidence.json').read_bytes()),barriers_sha256=sha((out/'barriers.json').read_bytes()),repeat=repeat,commands=records,qualification='Complete private frequency lifecycle latency, including Global allocation, updates/queries, final snapshot copy, preallocated result-buffer writes and owner drop. Fixture decoding, independent comparisons, allocation monitoring and formatting are outside clocks. Known task workers completed before timing; general host idle state is not claimed.')
    def save():(out/'evidence.json').write_text(json.dumps(report,indent=2)+'\n')
    save()
    try:
        for name in (['all','safe']if repeat else['safe','frequencies','all']):
            print('MEASURE',name,flush=True);binary=next(b for b in prior['binaries']if b['name']==name);row=run(out,name,[str(prepared/binary['path']),'--timed'],environment());records.append(row);samples=check(parse((out/row['stdout']).read_bytes()),expected,name!='safe',True);need(len(samples)==108,'exact sample inventory');save()
        need(sources()==prior['source'],'source unchanged');report.update(status='passed',source_unchanged=True);save()
    except BaseException as error:report.update(status='failed',error=repr(error));save();raise

def main():
    p=argparse.ArgumentParser();p.add_argument('mode',choices=['prepare','measure']);p.add_argument('output',type=Path);p.add_argument('--prepared',type=Path);p.add_argument('--repeat',action='store_true');p.add_argument('--barrier',type=Path,action='append',default=[]);a=p.parse_args()
    if a.mode=='prepare':need(a.prepared is None and not a.repeat and not a.barrier,'prepare arguments');prepare(a.output.resolve())
    else:need(a.prepared is not None,'preparation required');measure(a.prepared.resolve(),a.output.resolve(),a.repeat,a.barrier)
if __name__=='__main__':main()
