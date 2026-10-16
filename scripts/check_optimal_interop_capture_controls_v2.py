from pathlib import Path
import copy,hashlib,json,os,shutil,struct,sys,tempfile
ROOT=Path(__file__).resolve().parents[1];sys.path.insert(0,str(ROOT/'scripts'))
import check_optimal_interop_capture_v3 as audit
import check_optimal_interop_recipes as inputs
from oracles import optimal_interop as generator
CAPTURE=Path(sys.argv[1]).resolve()
attempts=[]
def no_process(event,args):
 if event in ('subprocess.Popen','os.system','os.fork','os.posix_spawn','os.exec','os.spawn'):
  attempts.append(event);raise RuntimeError('child execution forbidden')
sys.addaudithook(no_process)
original=json.loads((CAPTURE/'evidence.json').read_text())
# Full fresh regeneration and replay first. Frozen input objects may then be
# reused for adversarial evidence tests; no artifact checks are bypassed.
manifest=inputs.check();cases=generator.recipes()
inputs.check=lambda:manifest
generator.recipes=lambda:cases
positive=audit.check(CAPTURE)
controls=[]
def reject(label,action):
 try:action()
 except (ValueError,KeyError,IndexError,TypeError,OSError):controls.append(label)
 else:raise ValueError('accepted forged evidence: '+label)
records={r['label']:r for r in original['commands']}
pinned=original['upstream_source_sha256']
expected_sources=sorted(n for n in pinned if n.endswith('.c') and any(n.startswith('lib/'+p+'/') for p in ('common','compress','decompress')))
def blob(row,kind):return (CAPTURE/row[kind]['path']).read_bytes()
for label,modify in [
 ('recovered-sanitizer',lambda e,r:r['sanitized-compile-0']['argv'].remove('-fno-sanitize-recover=all')),
 ('missing-unit-instrumentation',lambda e,r:r['sanitized-compile-5']['argv'].remove('-fsanitize=address,undefined')),
 ('missing-helper-instrumentation',lambda e,r:r['sanitized-link']['argv'].remove('-fsanitize=address,undefined')),
 ('changed-optimization',lambda e,r:r['normal-compile-1']['argv'].__setitem__(r['normal-compile-1']['argv'].index('-O2'),'-O3')),
 ('injected-library',lambda e,r:r['normal-link']['argv'].insert(-2,'/tmp/untrusted.a')),
 ('changed-cwd',lambda e,r:r['case-000-rust-encode-safe'].update(cwd='/tmp')),
 ('unbounded-exchange',lambda e,r:r['case-000-rust-encode-safe'].update(timeout_seconds=180)),
 ('wrong-executable',lambda e,r:r['case-000-rust-encode-safe']['argv'].__setitem__(0,r['case-000-rust-encode-all']['argv'][0])),
 ('unsafe-default-build',lambda e,r:r['rust-build-safe']['argv'].append('--all-features')),
 ('missing-all-features',lambda e,r:r['rust-build-all']['argv'].remove('--all-features')),
 ('loader-injection',lambda e,r:e['environment'].update(DYLD_INSERT_LIBRARIES='/tmp/unused')),
 ('sanitizer-environment',lambda e,r:e['environment'].update(ASAN_OPTIONS='detect_leaks=0:halt_on_error=0')),
 ('changed-toolchain',lambda e,r:e['environment'].update(RUSTUP_TOOLCHAIN='stable')),
 ('dependency-omission',lambda e,r:e['builds']['normal']['dependencies'].pop(next(iter(e['builds']['normal']['dependencies'])))),
 ('binary-identity',lambda e,r:e['builds']['all'].update(executable_sha256='0'*64)),
 ('duration-type',lambda e,r:r['normal-link'].update(seconds=True)),
 ('duration-unbounded',lambda e,r:r['normal-link'].update(seconds=999999)),
 ('schema-type',lambda e,r:e.update(schema_version=True)),
]:
 e=copy.deepcopy(original);r={v['label']:v for v in e['commands']};modify(e,r)
 reject(label,lambda:audit.audit_commands(e,r,blob,CAPTURE,expected_sources,pinned))
with tempfile.TemporaryDirectory(prefix='zstdoxide-optimal-public-forged-') as temporary:
 target=Path(temporary)/'capture'
 shutil.copytree(CAPTURE,target,copy_function=os.link)
 # Break the evidence hardlink before any edit. Mutated binary artifacts below
 # use the same unlink-first discipline; original capture bytes stay immutable.
 report=target/'evidence.json';report.unlink()
 def check_changed(label,modify):
  e=copy.deepcopy(original);changed=[]
  def rewrite(command,kind,transform):
   row=next(r for r in e['commands'] if r['label']==command);name=row[kind]['path'];p=target/name
   data=transform((CAPTURE/name).read_bytes());p.unlink();p.write_bytes(data);changed.append(name)
   row[kind].update(bytes=len(data),sha256=hashlib.sha256(data).hexdigest())
  try:
   modify(e,rewrite)
   report.write_text(json.dumps(e,sort_keys=True,indent=2)+'\n')
   reject(label,lambda:audit.check(target))
  finally:
   for name in changed:
    p=target/name;p.unlink();os.link(CAPTURE/name,p)
 for label,modify in [
  ('incomplete-status',lambda e,w:e.update(status='running')),
  ('wrong-upstream',lambda e,w:e.update(upstream_revision='0'*40)),
  ('support-archive',lambda e,w:e.update(support_archive_used=True)),
  ('changed-source-after',lambda e,w:e['source_after'].update({'src/lib.rs':'0'*64})),
  ('missing-source-key',lambda e,w:(e['source_before'].pop('src/lib.rs'),e['source_after'].pop('src/lib.rs'))),
  ('missing-optimal-plan-owner',lambda e,w:(e['source_before'].pop('src/optimal_plan.rs'),e['source_after'].pop('src/optimal_plan.rs'))),
  ('missing-optimal-token-owner',lambda e,w:(e['source_before'].pop('src/optimal_state.rs'),e['source_after'].pop('src/optimal_state.rs'))),
  ('reviewed-source-forgery',lambda e,w:(e['source_before'].update({'examples/reference_greedy_oracle.rs':'0'*64}),e['source_after'].update({'examples/reference_greedy_oracle.rs':'0'*64}))),
  ('missing-command',lambda e,w:e['commands'].pop()),
  ('duplicate-command',lambda e,w:e['commands'].__setitem__(1,copy.deepcopy(e['commands'][0]))),
  ('boolean-exit',lambda e,w:e['commands'][0].update(exit_code=False)),
  ('artifact-traversal',lambda e,w:e['commands'][0]['stdout'].update(path='../outside')),
  ('artifact-boolean-size',lambda e,w:e['commands'][0]['stdout'].update(bytes=True)),
  ('artifact-digest',lambda e,w:e['commands'][0]['stdout'].update(sha256='0'*64)),
  ('decoded-total',lambda e,w:e['totals'].update(decoded_bytes=e['totals']['decoded_bytes']-1)),
  ('equivalence-count',lambda e,w:e.update(equivalence_groups=24)),
  ('actual-request',lambda e,w:w('case-001-rust-encode-safe','stdin',lambda b:b[:-1]+bytes([b[-1]^1]))),
  ('actual-c-scalar',lambda e,w:w('case-001-c-encode-normal','stdout',lambda b:b[:12]+struct.pack('<I',10)+b[16:])),
  ('actual-decoded-byte',lambda e,w:w('case-001-c-normal-decode-safe','stdout',lambda b:b[:-1]+bytes([b[-1]^1]))),
  ('actual-negative-sanitizer',lambda e,w:w('sanitized-negative-old-protocol-magic','stderr',lambda b:b+b'AddressSanitizer: forged diagnostic\n')),
 ]:check_changed(label,modify)
 # Changing a valid unused target preserves local admission and is reflected
 # coherently in both response records. Historical scalar identity must catch it.
 index=next(i for i,c in enumerate(cases) if c.name=='historical-unsupported-strategy-3')
 def historical(e,write):
  for mode in ('safe','all'):
   write(f'case-{index:03}-rust-encode-{mode}','stdout',lambda b:b[:32]+struct.pack('<I',900)+b[36:])
   e['cases'][index]['rust'][mode]['resolved'][5]=900
 check_changed('coherent-historical-scalar',historical)
 index=next(i for i,c in enumerate(cases) if c.name=='lazy-d1_chain_empty')
 for field,offset,value in [(6,36,3),(5,32,900)]:
  def successor(e,write):
   for mode in ('safe','all'):
    write(f'case-{index:03}-rust-encode-{mode}','stdout',lambda b:b[:offset]+struct.pack('<I',value)+b[offset+4:])
    e['cases'][index]['rust'][mode]['resolved'][field]=value
  check_changed('coherent-new-request-field-'+str(field),successor)

 # Cover both newly added tree requests and the fourth-digit request grammar.
 for case_index,field,offset,value in [(886,6,36,5),(1196,3,24,7)]:
  def changed_tree(e,write):
   for mode in ('safe','all'):
    write(f'case-{case_index:03}-rust-encode-{mode}','stdout',lambda b:b[:offset]+struct.pack('<I',value)+b[offset+4:])
    e['cases'][case_index]['rust'][mode]['resolved'][field]=value
  check_changed('coherent-tree-tuple-'+str(case_index),changed_tree)

 # New source-aware Optimal tuples are independently reconstructed, including
 # complete short-key matching and default-level/equivalent explicit requests.
 for name,field,offset,value in [('optimal-minimum-3-target-1',4,28,4),('optimal-default-11-64-uniform',2,20,8),('optimal-observed-fresh_wide_horizon',5,32,1)]:
  case_index=next(i for i,c in enumerate(cases) if c.name==name)
  def changed_optimal(e,write):
   for mode in ('safe','all'):
    write(f'case-{case_index:03}-rust-encode-{mode}','stdout',lambda b:b[:offset]+struct.pack('<I',value)+b[offset+4:])
    e['cases'][case_index]['rust'][mode]['resolved'][field]=value
  check_changed('coherent-optimal-tuple-'+name,changed_optimal)

audit.require(not attempts and len(controls)==46,'complete no-child controls')
audit.require(audit.check(CAPTURE)==positive,'original capture unchanged and still valid')
print(json.dumps({'optimized':not __debug__,'positive':positive,'controls':controls,'child_attempts':len(attempts)},sort_keys=True,indent=2))
