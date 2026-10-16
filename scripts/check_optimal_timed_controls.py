#!/usr/bin/env python3
"""Offline malformed timing/barrier controls; never invokes codec or build tools."""
from pathlib import Path
import copy,hashlib,json,shutil,sys
sys.dont_write_bytecode=True
import benchmark_optimal_frames as m
original=Path(sys.argv[1]).resolve();out=Path(sys.argv[2]).resolve()
if out.exists():raise ValueError('fresh controls output required')
out.mkdir();baseline=m.shared.load_json(original/'evidence.json');m.require(baseline['timed']is True,'timed campaign required');records=[];children=[]
def no_child(event,args):
 if event in ('subprocess.Popen','os.system','os.fork','os.posix_spawn','os.exec','os.spawn'):
  children.append(event);raise RuntimeError('child execution forbidden')
sys.addaudithook(no_child)
def mutate_rows(p,e,fn):
 label='run-safe';f=p/(label+'.stdout');rows=[m.shared.parse_json(x)for x in f.read_text().splitlines()];fn(rows)
 f.write_text('\n'.join(json.dumps(r,sort_keys=True,allow_nan=False)for r in rows)+'\n')
 next(c for c in e['commands']if c['label']==label)['stdout_sha256']=m.sha(f)
def component(e,name,fn):
 item=e['cpu_barrier']['completed'][name];v=m.shared.parse_json(item['json']);fn(v);item['json']=json.dumps(v,sort_keys=True,allow_nan=False)+'\n';item['sha256']=hashlib.sha256(item['json'].encode()).hexdigest()
def test(name,change,positive=False):
 p=out/name;p.mkdir()
 for f in original.iterdir():
  if f.is_file():shutil.copyfile(f,p/f.name)
 shutil.copytree(original/'frames',p/'frames');e=copy.deepcopy(baseline);change(p,e)
 try:m.validate(p,e)
 except (ValueError,KeyError,TypeError,IndexError,OSError,ZeroDivisionError)as error:accepted=False;diagnostic=str(error)
 else:accepted=True;diagnostic=None
 passed=accepted==positive;records.append(dict(name=name,expected='accepted'if positive else'rejected',actual='accepted'if accepted else'rejected',passed=passed,diagnostic=diagnostic))
 (p/'evidence.json').write_text(json.dumps(e,indent=2,allow_nan=False)+'\n');print(name,'PASS'if passed else'FAIL',flush=True)
test('positive',lambda p,e:None,True)
for name,fn in [
 ('sample-missing',lambda r:r[1]['samples'].pop()),
 ('iterations-zero',lambda r:r[1]['samples'][0].update(iterations=0)),
 ('iterations-bool',lambda r:r[1]['samples'][0].update(iterations=True)),
 ('elapsed-bool',lambda r:r[1]['samples'][0].update(elapsed_ns=True)),
 ('elapsed-float',lambda r:r[1]['samples'][0].update(elapsed_ns=100000000.0)),
 ('elapsed-too-short',lambda r:r[1]['samples'][0].update(elapsed_ns=99999999)),
 ('sample-extra-field',lambda r:r[1]['samples'][0].update(unobserved_clock=1)),
 ('metadata-not-timed',lambda r:r[0].update(timed=False)),
]:test(name,lambda p,e,fn=fn:mutate_rows(p,e,fn))
test('summary-median',lambda p,e:e['summary']['safe'][0]['timing'].update(median_ns_per_byte=0))
test('summary-throughput',lambda p,e:e['summary']['safe'][0]['timing'].update(median_mib_per_second=0))
test('missing-barrier',lambda p,e:e.update(cpu_barrier=None))
test('pending-cpu-job',lambda p,e:e['cpu_barrier'].update(task_owned_jobs_remaining=['test']))
test('naive-timestamp',lambda p,e:e['cpu_barrier'].update(recorded_utc='2026-10-16T00:00:00'))
test('missing-prerequisite',lambda p,e:e['cpu_barrier']['completed'].pop('matrix'))
test('prerequisite-hash',lambda p,e:e['cpu_barrier']['completed']['matrix'].update(sha256='0'*64))
for name,part,fn in [
 ('incomplete-matrix','matrix',lambda r:r.update(status='running')),
 ('missing-feature','matrix',lambda r:r.update(isolated_features=18)),
 ('failed-matrix-command','matrix',lambda r:r.update(failed_commands=1)),
 ('missing-native-case','native',lambda r:r.update(recipes=1268)),
 ('platform-incomplete','platform',lambda r:r.update(interpreted_test_executions=1)),
 ('platform-running','platform',lambda r:r.update(processes_remaining=1)),
 ('clock-free-source-change','clock_free',lambda r:r['source'].update({'src/optimal_state.rs':'0'*64})),
 ('clock-free-missing-feature','clock_free',lambda r:r['configs'].pop()),
 ('clock-free-was-timed','clock_free',lambda r:r.update(timed=True)),
]:test(name,lambda p,e,part=part,fn=fn:component(e,part,fn))
test('restored-positive',lambda p,e:None,True)
m.require(not children and all(r['passed']for r in records),'complete no-child timing controls')
m.validate(original,baseline)
report=dict(status='passed',optimized=not __debug__,checks=len(records),rejected=sum(r['expected']=='rejected'for r in records),child_attempts=len(children),source_sha256=m.sha(__file__),original=str(original),records=records)
(out/'summary.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({k:v for k,v in report.items()if k not in ('records',)},sort_keys=True))
