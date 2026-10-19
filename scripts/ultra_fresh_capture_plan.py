"""Pure bounded capture plan for the complete fresh ordinary upstream Ultra observer."""
from pathlib import Path
import hashlib
import json
import struct
from functools import lru_cache
from oracles.ultra_fresh import REVISION, require, parse_packet
from oracles.ultra_fresh_recipes import recipes as source_recipes

@lru_cache(maxsize=1)
def retained_recipes():
    return source_recipes()

@lru_cache(maxsize=1)
def recipes():
    return {name:recipe.request for name,recipe in retained_recipes().items()}
from plan_ultra_fresh import manifest
from oracles.ultra_fresh_instrumentation import instrument

ROOT = Path(__file__).resolve().parents[1]
HELPER = 'scripts/oracles/ultra_fresh.c'
MANIFEST = 'tests/fixtures/reference-ultra-fresh-recipes/capture-plan.json'
# This explicit closure is generated and reviewed before the first native run.
INPUTS = ['docs/reference-ultra-fresh-adapter-review.md', 'docs/reference-ultra-fresh-model.md', 'docs/reference-ultra-fresh-protocol.md', 'scripts/capture_ultra_fresh.py', 'scripts/check_ultra_fresh_capture.py', 'scripts/check_ultra_fresh_capture_controls.py', 'scripts/check_ultra_fresh_capture_plan.py', 'scripts/oracles/fast_frame_publication_support.py', 'scripts/oracles/optimal_fresh_wide.c', 'scripts/oracles/optimal_fresh_wide.py', 'scripts/oracles/optimal_fresh_wide_recipes.py', 'scripts/oracles/optimal_fresh_wide_validation.py', 'scripts/oracles/optimal_match_broad_validation.py', 'scripts/oracles/optimal_match_validation_v2.py', 'scripts/oracles/optimal_matches.py', 'scripts/oracles/optimal_matches_broad.py', 'scripts/oracles/optimal_path_instrumentation.py', 'scripts/oracles/optimal_path_script_inputs.json', 'scripts/oracles/optimal_path_validation.py', 'scripts/oracles/optimal_path_wide_validation.py', 'scripts/oracles/optimal_paths.py', 'scripts/oracles/optimal_paths_wide.py', 'scripts/oracles/optimal_price_validation.py', 'scripts/oracles/optimal_prices_v2.py', 'scripts/oracles/optimal_retained.py', 'scripts/oracles/optimal_retained_validation_v2.py', 'scripts/oracles/optimal_retained_wide.py', 'scripts/oracles/optimal_retained_wide_recipes.py', 'scripts/oracles/optimal_startup.py', 'scripts/oracles/optimal_startup_instrumentation.py', 'scripts/oracles/optimal_startup_recipes.py', 'scripts/oracles/tree_direct.py', 'scripts/oracles/tree_validation.py', 'scripts/oracles/ultra_frequency.py', 'scripts/oracles/ultra_fresh.c', 'scripts/oracles/ultra_fresh.py', 'scripts/oracles/ultra_fresh_instrumentation.py', 'scripts/oracles/ultra_fresh_recipes.py', 'scripts/oracles/ultra_fresh_validation.py', 'scripts/oracles/ultra_path_instrumentation.py', 'scripts/oracles/ultra_path_script_inputs.json', 'scripts/oracles/ultra_path_validation.py', 'scripts/oracles/ultra_paths.py', 'scripts/oracles/ultra_scalar.py', 'scripts/plan_ultra_fresh.py', 'scripts/ultra_fresh_capture_plan.py', 'tests/fixtures/reference-ultra-fresh-recipes/capture-plan.json', 'tests/fixtures/reference-ultra-fresh-recipes/recipes.json', 'tests/upstream/manifest.json']



def sha(data):
    return hashlib.sha256(data).hexdigest()


@lru_cache(maxsize=1)
def manifest_bytes():
    return (json.dumps(manifest(),sort_keys=True,indent=2)+'\n').encode()


def source_manifest():
    result = {}
    for name in INPUTS:
        p = ROOT / name
        require(p.is_file() and not p.is_symlink(), 'regular source input: ' + name)
        result[name] = sha(p.read_bytes())
    predictions = json.loads((ROOT/MANIFEST).read_bytes())['cases']
    cases = retained_recipes()
    require(len(predictions)==len(cases)==44 and {r['name'] for r in predictions}==set(cases), 'complete prediction inventory')
    for row in predictions:
        recipe = cases[row['name']]
        require(row['packet_sha256']==sha(recipe.request.packet()) and row['policy']==recipe.request.policy
                and row['hypothesized_kinds']==list(recipe.kinds), 'prediction request identity')
    require(result['tests/upstream/manifest.json']=='3bb5ac1c2a0407b71c5e5dbaabac6b39074f5b7f851801a2056856a910f3ccaa','pinned source inventory')
    return result


@lru_cache(maxsize=1)
def malformed_packets():
    base = recipes()['uf_p2_fresh_wide_uniform'].packet()
    short = recipes()['uf_p2_alternative_seed_0'].packet()
    result = {'short':base[:123], 'oversize':base+b'!', 'truncated':base[:-1],
        'trailing':short+b'!', 'magic':b'!'+base[1:],
        'nested-magic':base[:12]+b'!'+base[13:]}
    for name, offset, value in [('policy-one',8,1),('policy-three',8,3),('policy-max',8,0xffffffff),
        ('short-source',20,63),('source-cap',20,16385),('count-low',24,0),('count-high',24,2),
        ('window-low',28,9),('window-high',28,17),('window-expiry',28,13),
        ('hash-low',32,5),('hash-high',32,13),('chain-low',36,5),('chain-high',36,13),
        ('search-low',40,0),('search-high',40,7),('minimum-low',44,2),('minimum-high',44,8),
        ('target',48,4096),('mode',52,2),('block-low',56,63),('block-high',56,16385),('schedule-total',56,65)]:
        result[name] = base[:offset]+struct.pack('<I',value)+base[offset+4:]
    for name,data in result.items():
        try:parse_packet(data)
        except ValueError:pass
        else:raise ValueError('negative packet accepted: '+name)
    return result


def diagnostic(name):
    reason = {'short':'packet size', 'oversize':'packet size',
        'truncated':'complete retained source extent', 'trailing':'complete retained source extent',
        'magic':'packet magic', 'nested-magic':'nested packet magic',
        'policy-one':'price policy', 'policy-three':'price policy', 'policy-max':'price policy',
        'window-expiry':'source fits retained window', 'block-low':'block length',
        'block-high':'block length', 'schedule-total':'exact schedule total',
        'fs-symlink':'packet open', 'fs-missing':'packet open',
        'fs-directory':'regular packet', 'fs-fifo':'regular packet'}.get(name,'request admission')
    require(name in malformed_packets() or name in special_inputs(), 'known negative diagnostic')
    return 'Ultra fresh observer: '+reason+'\n'


def special_inputs():
    return ('fs-symlink','fs-missing','fs-directory','fs-fifo')


@lru_cache(maxsize=1)
def requests():
    return {**{name:r.packet()for name,r in recipes().items()},
            **{'negative-'+name:p for name,p in malformed_packets().items()}}


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
    previous=(ROOT/'scripts/oracles/optimal_fresh_wide.c').read_text()
    current=(ROOT/HELPER).read_text()
    diff=''.join(difflib.unified_diff(previous.splitlines(True),current.splitlines(True),fromfile='optimal_fresh_wide.c',tofile='ultra_fresh.c')).encode()
    return {'adapter-diff.patch':diff, 'adapter-digests.json':(json.dumps(dict(previous_sha256=sha(previous.encode()),current_sha256=sha(current.encode())),sort_keys=True)+'\n').encode(), 'original_zstd_opt.c':original,'observed_zstd_opt.c':transformed,
            'instrumentation.json':(json.dumps(changes,sort_keys=True,indent=2)+'\n').encode()}


def artifact_paths(mode, name):
    return [f'artifacts/{mode}/{name}.{suffix}' for suffix in ('0.zst','1.zst','0.json','1.json')]


def plan(upstream,output,compiler,units):
    common=[str(compiler),'-std=c11','-g','-DDEBUGLEVEL=2','-DZSTD_DISABLE_ASM','-DZSTD_LEGACY_SUPPORT=0',
        '-UZSTD_MULTITHREAD','-DXXH_NAMESPACE=ZSTD_','-I',str(output/'generated'),'-I',str(upstream/'lib'),
        '-I',str(upstream/'lib/common'),'-I',str(upstream/'lib/compress')]
    rows=[]
    def add(label,argv,expected=0,expected_diagnostic=None):
        rows.append(dict(label=label,argv=[str(v)for v in argv],expected_exit=expected,expected_diagnostic=expected_diagnostic,
            timeout_seconds=120,stdout_limit=(256<<20)if '-run-uf_'in label else(2<<20),stderr_limit=1<<20))
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
        executable=output/('c-'+mode)/'ultra-fresh'
        add(mode+'-link',base+[*objects,'-o',executable])
        for name in requests():
            add(mode+'-run-'+name,[executable,output/'requests'/(name+'.bin'),
                *[output/path for path in artifact_paths(mode,name)]],1 if name.startswith('negative-')else 0, diagnostic(name[9:]) if name.startswith('negative-') else None)
        for name in special_inputs():
            add(mode+'-run-negative-'+name,[executable,output/'special'/name,
                *[output/path for path in artifact_paths(mode,'negative-'+name)]],1,diagnostic(name))
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
