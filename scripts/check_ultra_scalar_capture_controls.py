#!/usr/bin/env python3
"""Replay Ultra scalar capture and reject changed evidence without children."""
from pathlib import Path
import copy,json,os,shutil,sys
sys.dont_write_bytecode=True
import check_ultra_scalar_capture as audit
from oracles import ultra_scalar as m
original=Path(sys.argv[1]).resolve();out=Path(sys.argv[2]).resolve()
m.need(not out.exists(),'fresh controls output');out.mkdir()
base=m.load((original/'evidence.json').read_bytes());children=[];records=[]
def no_child(event,args):
    if event in ('subprocess.Popen','os.system','os.fork','os.posix_spawn','os.exec','os.spawn'):
        children.append(event);raise RuntimeError('child execution forbidden')
sys.addaudithook(no_child)
def copy_input(src,dst):
    # Object/executable files remain immutable in these controls. Hard links
    # conserve disk; source/log/evidence inputs that may change are independent.
    if Path(src).relative_to(original).parts[0] in ('c-normal','c-sanitized'):
        os.link(src,dst)
    else:shutil.copyfile(src,dst)
    return dst
def log(p,e,label,stream,transform):
    path=p/'logs'/(label+'.'+stream);path.write_bytes(transform(path.read_bytes()))
    c=next(c for c in e['commands']if c['label']==label)
    c[stream]=dict(path=str(path.relative_to(p)),bytes=path.stat().st_size,sha256=m.sha(path.read_bytes()))
def scalar(p,e,transform):
    for mode in ('normal','sanitized'):
        def alter(data):
            rows=[m.load(x)for x in data.splitlines()];transform(rows)
            return ('\n'.join(json.dumps(r,sort_keys=True)for r in rows)+'\n').encode()
        log(p,e,mode+'-run-scalar-boundaries','stdout',alter)
def test(name,change,positive=False):
    p=out/name;shutil.copytree(original,p,copy_function=copy_input);e=copy.deepcopy(base);change(p,e)
    (p/'evidence.json').write_text(json.dumps(e,indent=2)+'\n')
    try:audit.audit(p)
    except (ValueError,KeyError,TypeError,IndexError,OSError)as error:accepted=False;diagnostic=str(error)
    else:accepted=True;diagnostic=None
    records.append(dict(name=name,expected='accepted'if positive else'rejected',accepted=accepted,passed=accepted==positive,diagnostic=diagnostic));print(name,'PASS'if accepted==positive else'FAIL',flush=True)

test('positive',lambda p,e:None,True)
test('unfinished',lambda p,e:e.update(status='running'))
test('schema-bool',lambda p,e:e.update(schema_version=True))
test('wrong-revision',lambda p,e:e.update(upstream_revision='0'*40))
test('source-changed-flag',lambda p,e:e.update(source_unchanged=False))
test('compiler-hash',lambda p,e:e.update(compiler_sha256='0'*64))
test('source-closure',lambda p,e:e['source'].pop('scripts/oracles/ultra_scalar.py'))
test('source-copy',lambda p,e:(p/'source/scripts/oracles/ultra_scalar.py').write_text('changed\n'))
test('reference-hash',lambda p,e:e['upstream_source_sha256'].update({'lib/compress/zstd_opt.c':'0'*64}))
test('environment-injection',lambda p,e:e['environment'].update(LD_PRELOAD='foreign.so'))
test('sanitizer-environment',lambda p,e:e['environment'].update(ASAN_OPTIONS='halt_on_error=0'))
test('missing-command',lambda p,e:e['commands'].pop())
test('reordered-command',lambda p,e:e['commands'].reverse())
test('exit-bool',lambda p,e:e['commands'][0].update(exit_code=False))
test('command-timeout',lambda p,e:e['commands'][0].update(seconds=1000))
test('unrecorded-stdin',lambda p,e:e['commands'][0].update(stdin='input'))
test('changed-compile-command',lambda p,e:next(c for c in e['commands']if c['label']=='sanitized-compile-0')['argv'].remove('-fsanitize=address,undefined'))
test('changed-plan',lambda p,e:e['plan'][0].update(stdout_limit=1))
test('missing-artifact',lambda p,e:e['artifacts'].pop(next(iter(e['artifacts']))))
test('artifact-hash',lambda p,e:next(iter(e['artifacts'].values())).update(sha256='0'*64))
test('changed-request',lambda p,e:(p/'requests/scalar-boundaries.bin').write_bytes(b'!'))
test('extra-request',lambda p,e:(p/'requests/extra.bin').write_bytes(b'!'))
test('extra-log',lambda p,e:(p/'logs/extra.stdout').write_bytes(b'!'))
test('log-hash',lambda p,e:e['commands'][0]['stdout'].update(sha256='0'*64))
test('changed-revision-output',lambda p,e:log(p,e,'reference-revision','stdout',lambda b:b'0'*40+b'\n'))
test('changed-predefines',lambda p,e:log(p,e,'sanitized-predefines','stdout',lambda b:b.replace(b'#define DEBUGLEVEL 2',b'#define DEBUGLEVEL 0')))
test('negative-output',lambda p,e:log(p,e,'normal-run-negative-mode','stdout',lambda b:b'{}\n'))
test('negative-diagnostic',lambda p,e:log(p,e,'normal-run-negative-mode','stderr',lambda b:b'ultra scalar oracle: mutated storage\n'))
test('coherent-price',lambda p,e:scalar(p,e,lambda r:r[1].update(value=1)))
test('coherent-float-price',lambda p,e:scalar(p,e,lambda r:r[1].update(value=0.0)))
test('coherent-mode',lambda p,e:scalar(p,e,lambda r:r[1].update(mode=2)))
test('missing-observation',lambda p,e:scalar(p,e,lambda r:r.pop(1)))
test('changed-end-guard',lambda p,e:scalar(p,e,lambda r:r[-1].update(guards_unchanged=False)))
test('coherent-signed-increment',lambda p,e:scalar(p,e,lambda r:next(x for x in r if x.get('tag')==4 and x['value']<0).update(value=0)))
test('restored-positive',lambda p,e:None,True)
m.need(not children and all(r['passed']for r in records),'all no-child controls')
audit.audit(original)
report=dict(status='passed',optimized=not __debug__,checks=len(records),rejected=sum(r['expected']=='rejected'for r in records),child_attempts=len(children),source_sha256=m.sha(Path(__file__).read_bytes()),original=str(original),original_evidence_sha256=m.sha((original/'evidence.json').read_bytes()),records=records)
(out/'summary.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({k:v for k,v in report.items()if k!='records'},sort_keys=True))
