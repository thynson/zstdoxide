#!/usr/bin/env python3
"""Offline independent validator controls; never invokes Cargo/C/bench executables."""
import copy,hashlib,importlib.util,json,pathlib,shutil,sys
sys.dont_write_bytecode=True
ROOT=pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/'scripts'))
import benchmark_retained_ultra_frames as m
original=pathlib.Path(sys.argv[1]).resolve()
out=pathlib.Path(sys.argv[2]).resolve()
if out.exists():raise SystemExit('fresh controls output required')
out.mkdir()
baseline=m.shared.load_json(original/'evidence.json')
records=[]
child_attempts = 0
def forbidden_process(*args,**kwargs):
 global child_attempts
 child_attempts += 1
 raise RuntimeError('child process forbidden')
m.subprocess.Popen=forbidden_process

def command(e,label):return next(c for c in e['commands'] if c['label']==label)
def changed_log(p,e,label,rows):
 name=label+'.stdout';(p/name).write_text('\n'.join(json.dumps(r,sort_keys=True) for r in rows)+'\n')
 command(e,label)['stdout_sha256']=m.sha(p/name)
def modify_rows(p,e,fn,configs=None):
 for config in configs or e['configs']:
  label='run-'+config; rows=[m.shared.parse_json(x) for x in (p/(label+'.stdout')).read_text().splitlines()]
  fn(rows);changed_log(p,e,label,rows)
 # Keep summary current to isolate structural/semantic row validation.
 values={c:[m.shared.parse_json(x) for x in (p/('run-'+c+'.stdout')).read_text().splitlines()][1:] for c in e['configs']}
 e['summary']=m.summarize(values,e['timed'])
def test(name,mutate,accept=False):
 p=out/name;p.mkdir()
 for file in original.iterdir():
  if file.is_file():shutil.copyfile(file,p/file.name)
 shutil.copytree(original/'frames',p/'frames')
 e=copy.deepcopy(baseline)
 mutate(p,e)
 try:m.validate(p,e)
 except (ValueError,KeyError,TypeError,IndexError,FileNotFoundError,ZeroDivisionError) as ex:
  passed=not accept;result='rejected';detail=str(ex)
 else:passed=accept;result='accepted';detail=None
 records.append(dict(name=name,expected='accepted' if accept else 'rejected',actual=result,passed=passed,diagnostic=detail))
 # Evidence is the exact mutated input. Preserve all small files/frames for reproducibility.
 (p/'evidence.json').write_text(json.dumps(e,indent=2,allow_nan=False)+'\n')
 print(name,result,'PASS' if passed else 'FAIL',flush=True)

test('positive',lambda p,e:None,True)
test('cargo-configs-empty',lambda p,e:e.update(cargo_configs=[]))
test('cargo-config-hash',lambda p,e:e['cargo_configs'][0].update(sha256='0'*64))
test('source-hash',lambda p,e:e['source'].update({'src/fast_encode.rs':'0'*64}))
test('config-duplicate',lambda p,e:e['configs'].append('safe'))
test('config-order',lambda p,e:e['configs'].reverse())
test('command-order',lambda p,e:e['commands'].__setitem__(slice(5,7),e['commands'][5:7][::-1]))
test('command-exit',lambda p,e:command(e,'run-safe').update(exit_code=1))
test('command-exit-bool',lambda p,e:command(e,'run-safe').update(exit_code=False))
test('command-exception',lambda p,e:command(e,'run-safe').update(exception='TimeoutExpired'))

def bool_environment(p,e):
 e['base_environment']['USER']=False
 for c in e['commands']:c['environment']['USER']=False
test('coherent-environment-bool',bool_environment)
test('build-target-argv',lambda p,e:command(e,'build-safe')['argv'].__setitem__(-1,'i686-unknown-linux-gnu'))
test('run-timing-env',lambda p,e:command(e,'run-safe')['environment'].update(ZSTDOXIDE_RETAINED_ULTRA_BENCH_TIMED='1'))
test('run-frame-dir-env',lambda p,e:command(e,'run-safe')['environment'].update(ZSTDOXIDE_RETAINED_ULTRA_BENCH_FRAMES='/tmp/wrong'))
test('compiler-hash',lambda p,e:e.update(compiler_sha256='0'*64))
test('executable-hash',lambda p,e:e['executables']['safe'].update(sha256='0'*64))
test('log-hash',lambda p,e:command(e,'run-safe').update(stdout_sha256='0'*64))
test('missing-log',lambda p,e:(p/'run-safe.stdout').unlink())
test('source-size',lambda p,e:modify_rows(p,e,lambda r:r[1].update(source_bytes=32769)))
test('source-fnv',lambda p,e:modify_rows(p,e,lambda r:r[1].update(source_fnv1a64='0'*16)))
test('resolved-tuple',lambda p,e:modify_rows(p,e,lambda r:r[1]['resolved'].__setitem__(0,20)))
test('row-bool',lambda p,e:modify_rows(p,e,lambda r:r[1].update(source_bytes=True)))
test('allocation-attempts',lambda p,e:modify_rows(p,e,lambda r:r[1]['allocation'].update(attempts=1)))
test('allocation-leak',lambda p,e:modify_rows(p,e,lambda r:r[1]['allocation'].update(final_live_bytes=1)))
test('allocation-owner',lambda p,e:modify_rows(p,e,lambda r:r[1]['allocation'].update(owners_dropped=0)))
test('allocation-float',lambda p,e:modify_rows(p,e,lambda r:r[1]['allocation'].update(peak_bytes=602041.0)))
test('smoke-samples',lambda p,e:modify_rows(p,e,lambda r:r[1]['samples'].append({'iterations':1,'elapsed_ns':100000000})))
test('summary-ratio',lambda p,e:e['summary']['safe'][0].update(compressed_size_ratio=1))
frame='retained-ultra-ur_p2_wide_frames_uniform_16385-retained-ultra.zst'
test('frame-missing',lambda p,e:(p/'frames/safe'/frame).unlink())
test('frame-extra',lambda p,e:(p/'frames/safe/extra.zst').write_bytes(b'x'))
test('frame-hash',lambda p,e:e['frames']['safe'][frame].update(sha256='0'*64))
test('frame-mutated',lambda p,e:(p/'frames/safe'/frame).write_bytes(b'x'))

def coherent_one_backend(p,e):
 fp=p/'frames/all'/frame;data=bytearray(fp.read_bytes());data[-1]^=1;fp.write_bytes(data)
 e['frames']['all'][frame]={'bytes':len(data),'sha256':m.sha(fp)}
 modify_rows(p,e,lambda r:next(v for v in r[1:] if (v['workload'],v['profile'])==('retained-ultra-ur_p2_wide_frames_uniform_16385','retained-ultra')).update(frame_fnv1a64=m.shared.fnv(data)),['all'])
test('coherent-all-frame-only',coherent_one_backend)

def coherent_all_frames(p,e):
 for config in e['configs']:
  fp=p/'frames'/config/frame;data=bytearray(fp.read_bytes());data[-1]^=1;fp.write_bytes(data)
  e['frames'][config][frame]={'bytes':len(data),'sha256':m.sha(fp)}
  modify_rows(p,e,lambda rows:rows[1].update(frame_fnv1a64=m.shared.fnv(data)),[config])
test('coherent-all-native-baseline-forgery',coherent_all_frames)
for name,index,value in [('wrong-strategy',6,5),('wrong-chain',1,13),('wrong-budget',3,7),('changed-target',5,4094),('boolean-strategy',6,True)]:
 test(name,lambda p,e,i=index,v=value:modify_rows(p,e,lambda rows:rows[1]['resolved'].__setitem__(i,v)))
test('smoke-barrier-forbidden',lambda p,e:e.update(cpu_barrier={}))
test('incomplete',lambda p,e:e.update(status='running'))
test('source-check-bool',lambda p,e:e.update(source_unchanged=1))
test('restored-positive',lambda p,e:None,True)
summary=dict(scope='Offline copied retained Ultra benchmark evidence controls; no subprocess or timing. Complete frame identity is independently pinned to native-audited baseline; allocator counters require backend equality and consistency, not an external owner oracle.',python_optimized=not __debug__,child_processes=child_attempts,original=str(original),source_sha256={name:m.sha(ROOT/name) for name in ['scripts/benchmark_retained_ultra_frames.py','scripts/check_retained_ultra_frame_baseline.py','examples/retained_ultra_benchmark.rs','scripts/benchmark_search.py']},runner_sha256=m.sha(__file__),checks=len(records),passed=sum(r['passed'] for r in records),records=records)
(out/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print(json.dumps({'checks':summary['checks'],'passed':summary['passed']}))
if summary['passed']!=summary['checks'] or child_attempts:raise SystemExit(1)
