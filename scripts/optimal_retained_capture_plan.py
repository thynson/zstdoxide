"""Pure bounded capture plan for the retained ordinary upstream Optimal observer."""
from pathlib import Path
import hashlib
import json
import struct
from functools import lru_cache
from oracles.optimal_retained import REVISION, require, parse_packet
from oracles.optimal_retained_recipes import recipes as retained_recipes

def recipes():
    return {name:recipe.request for name,recipe in retained_recipes().items()}
from plan_optimal_retained import manifest
from oracles.optimal_startup_instrumentation import instrument

ROOT = Path(__file__).resolve().parents[1]
HELPER = 'scripts/oracles/optimal_retained.c'
MANIFEST = 'tests/fixtures/reference-optimal-retained-recipes/recipes.json'
from optimal_startup_capture_plan_v2 import source_manifest as startup_sources
INPUTS = [
 'scripts/optimal_retained_capture_plan.py','scripts/capture_optimal_retained.py',
 'scripts/check_optimal_retained_capture.py','scripts/check_optimal_retained_capture_plan.py',
 'scripts/plan_optimal_retained.py','scripts/check_optimal_retained_model.py',
 'scripts/oracles/optimal_retained.py','scripts/oracles/optimal_retained_recipes.py',
 'scripts/oracles/optimal_retained_validation.py',HELPER,MANIFEST,
 'docs/reference-optimal-retained-protocol.md','docs/reference-optimal-retained-adapter-review.md']


def sha(data):
    return hashlib.sha256(data).hexdigest()


@lru_cache(maxsize=1)
def manifest_bytes():
    return (json.dumps(manifest(),sort_keys=True,indent=2)+'\n').encode()


def source_manifest():
    result = startup_sources()
    for name in INPUTS:
        p = ROOT / name
        require(p.is_file() and not p.is_symlink(), 'regular source input: ' + name)
        result[name] = sha(p.read_bytes())
    require((ROOT/MANIFEST).read_bytes() == manifest_bytes(), 'exact source-only manifest')
    require(result['tests/upstream/manifest.json']=='3bb5ac1c2a0407b71c5e5dbaabac6b39074f5b7f851801a2056856a910f3ccaa','pinned source inventory')
    return result


def malformed_packets():
    base = recipes()['retained_uniform'].packet()
    result = {'short':base[:43], 'truncated':base[:-1], 'trailing':base+b'!', 'magic':b'!'+base[1:]}
    for name,offset,value in [('short-source',8,127),('source-cap',8,1025),
        ('count-low',12,1),('count-high',12,5),('window-low',16,9),('window-high',16,13),
        ('hash-low',20,5),('hash-high',20,10),('chain-low',24,5),('chain-high',24,10),
        ('search-low',28,0),('search-high',28,5),('minimum-low',32,2),('minimum-high',32,8),
        ('target',36,4096),('mode',40,2),('block-low',44,63),('block-high',44,513),('schedule-total',44,65)]:
        result[name]=base[:offset]+struct.pack('<I',value)+base[offset+4:]
    for name,data in result.items():
        try:parse_packet(data)
        except ValueError:pass
        else:raise ValueError('negative packet accepted: '+name)
    return result


def requests():
    return {**{name:r.packet()for name,r in recipes().items()},**{'negative-'+name:p for name,p in malformed_packets().items()}}


def reference_sources(upstream):
    catalog = json.loads((ROOT / 'tests/upstream/manifest.json').read_bytes())
    require(catalog['upstream']['revision'] == REVISION, 'reference revision')
    sources = {r['path']: r['sha256'] for r in catalog['sources'] + catalog['excluded_sources']
               if r['path'].startswith('lib/') and Path(r['path']).suffix in ('.c', '.h')}
    for name, digest in sources.items():
        p = upstream / name
        require(p.is_file() and not p.is_symlink() and sha(p.read_bytes()) == digest, 'pinned reference source: ' + name)
    units = sorted(str(p.relative_to(upstream)) for folder in ('common', 'compress', 'decompress')
                   for p in (upstream / 'lib' / folder).glob('*.c'))
    require(len(units) == 26 and all(p in sources for p in units), '26 used codec units')
    return sources, [p for p in units if p != 'lib/compress/zstd_opt.c']


def generated(upstream):
    original=(upstream/'lib/compress/zstd_opt.c').read_bytes()
    transformed,changes=instrument(original)
    import difflib
    previous=(ROOT/'scripts/oracles/optimal_startup.c').read_text()
    current=(ROOT/HELPER).read_text()
    diff=''.join(difflib.unified_diff(previous.splitlines(True),current.splitlines(True),fromfile='optimal_startup.c',tofile='optimal_retained.c')).encode()
    return {'adapter-diff.patch':diff, 'original_zstd_opt.c':original,'observed_zstd_opt.c':transformed,
            'instrumentation.json':(json.dumps(changes,sort_keys=True,indent=2)+'\n').encode()}


def artifact_paths(mode, name):
    return [f'artifacts/{mode}/{name}.{suffix}' for suffix in ('0.zst','1.zst','0.json','1.json')]


def plan(upstream,output,compiler,units):
    common=[str(compiler),'-std=c11','-g','-DDEBUGLEVEL=2','-DZSTD_DISABLE_ASM','-DZSTD_LEGACY_SUPPORT=0',
        '-UZSTD_MULTITHREAD','-DXXH_NAMESPACE=ZSTD_','-I',str(output/'generated'),'-I',str(upstream/'lib'),
        '-I',str(upstream/'lib/common'),'-I',str(upstream/'lib/compress')]
    rows=[]
    def add(label,argv,expected=0):
        rows.append(dict(label=label,argv=[str(v)for v in argv],expected_exit=expected,
            timeout_seconds=120,stdout_limit=(256<<20)if '-run-retained_'in label else(2<<20),stderr_limit=1<<20))
    add('reference-revision',['git','-C',upstream,'rev-parse','HEAD'])
    add('reference-status',['git','-C',upstream,'status','--porcelain','--untracked-files=no'])
    add('compiler-version',[compiler,'--version']);add('compiler-target',[compiler,'-dumpmachine'])
    for mode in ['normal','sanitized']:
        flags=['-O2']if mode=='normal'else['-O1','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer']
        base=common+flags;helper=ROOT/HELPER
        add(mode+'-dependencies',base+['-MM',helper,*[upstream/p for p in units]])
        add(mode+'-predefines',base+['-dM','-E',helper]);objects=[]
        for i,unit in enumerate([*[upstream/p for p in units],helper]):
            obj=output/('c-'+mode)/f'unit-{i}.o'
            add(mode+f'-compile-{i}',base+(['-Wall','-Wextra','-Werror']if unit==helper else[])+['-c',unit,'-o',obj]);objects.append(obj)
        executable=output/('c-'+mode)/'optimal-retained'
        add(mode+'-link',base+[*objects,'-o',executable])
        for name in requests():
            add(mode+'-run-'+name,[executable,output/'requests'/(name+'.bin'),
                *[output/path for path in artifact_paths(mode,name)]],1 if name.startswith('negative-')else 0)
    require(len({r['label']for r in rows})==len(rows),'unique commands')
    return rows


def validate_plan(rows,upstream,output,compiler,units):
    require(json.dumps(rows,sort_keys=True)==json.dumps(plan(upstream,output,compiler,units),sort_keys=True),'exact bounded command inventory')
    return len(rows)


def environment(env):
    require(type(env) is dict and set(env) <= {'PATH','HOME','TMPDIR','DEVELOPER_DIR','SDKROOT',
        'LANG','LC_ALL','TZ','ASAN_OPTIONS','UBSAN_OPTIONS'}, 'execution environment closure')
    require(all(type(k) is str and type(v) is str for k,v in env.items()), 'environment strings')
    for key,value in {'LANG':'C','LC_ALL':'C','TZ':'UTC','ASAN_OPTIONS':'detect_leaks=0:halt_on_error=1',
                      'UBSAN_OPTIONS':'halt_on_error=1:print_stacktrace=1'}.items():
        require(env.get(key)==value,'required environment: '+key)


def command_gate(label, out, err, upstream, output, sources):
    # Called before subsequent commands, so an invalid dependency/predefine
    # observation cannot be followed by execution of an unqualified helper.
    require(not err, 'successful command stderr: '+label)
    if label == 'reference-revision':
        require(out.decode().strip()==REVISION,'actual pinned revision')
    elif label == 'reference-status':
        require(not out,'clean reference')
    elif label.endswith('-dependencies'):
        dependencies=set()
        for line in out.decode().replace('\\\n',' ').splitlines():
            require(':' in line,'dependency grammar')
            for text in line.split(':',1)[1].split():
                p=Path(text).resolve()
                require(p==ROOT/HELPER or p==output/'generated/observed_zstd_opt.c' or
                    p.is_relative_to(upstream) and str(p.relative_to(upstream)) in sources,
                    'pinned dependency closure')
                dependencies.add(p)
        require(output/'generated/observed_zstd_opt.c' in dependencies and ROOT/HELPER in dependencies,
                'embedded observer dependency')
    elif label.endswith('-predefines'):
        require(b'#define ZSTD_MULTITHREAD ' not in out and b'#define ZSTD_DISABLE_ASM' in out
                and b'#define DEBUGLEVEL 2' in out,'effective compilation switches')
    elif '-compile-' in label or label.endswith('-link'):
        require(not out,'quiet compilation')

    if label.endswith('-predefines'):
        expected=1 if label.startswith('sanitized-') else 0
        require(f'#define ZSTD_ADDRESS_SANITIZER {expected}\n'.encode() in out,
                'effective workspace sanitizer configuration')
        require(b'#define ZSTD_CWKSP_ASAN_REDZONE_SIZE 128\n' in out and
                b'#define ZSTD_ASAN_DONT_POISON_WORKSPACE' not in out,'original workspace redzones')
