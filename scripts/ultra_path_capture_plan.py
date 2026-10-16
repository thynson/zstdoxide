"""Pure bounded capture plan for the scripted upstream Ultra path observer."""
from pathlib import Path
import hashlib
import difflib
import json
import struct
from functools import lru_cache
from oracles.ultra_paths import REVISION, parse_packet
from oracles.ultra_scalar import need as require
from oracles.ultra_path_recipes import recipes
from oracles.ultra_path_validation import Replay
from oracles.ultra_path_instrumentation import instrument

ROOT = Path(__file__).resolve().parents[1]
HELPER = 'scripts/oracles/ultra_paths.c'
MANIFEST = 'tests/fixtures/reference-ultra-path-recipes/recipes.json'
INPUTS = ['scripts/ultra_path_capture_plan.py', 'scripts/capture_ultra_paths.py', 'scripts/check_ultra_path_capture.py', 'scripts/check_ultra_path_model.py', 'scripts/check_ultra_path_baseline.py', 'scripts/check_ultra_path_capture_plan.py', 'scripts/oracles/ultra_paths.py', 'scripts/oracles/ultra_path_recipes.py', 'scripts/oracles/ultra_path_validation.py', 'scripts/oracles/ultra_path_instrumentation.py', 'scripts/oracles/ultra_path_script_inputs.json', 'scripts/oracles/ultra_frequency.py', 'scripts/oracles/ultra_scalar.py', 'scripts/oracles/optimal_price_validation.py', 'scripts/oracles/optimal_prices_v2.py', 'scripts/oracles/tree_validation.py', 'scripts/oracles/tree_direct.py', 'scripts/oracles/optimal_paths.py', 'scripts/oracles/optimal_path_validation.py', 'scripts/oracles/optimal_path_recipes.py', 'scripts/oracles/optimal_path_script_inputs.json', 'scripts/oracles/optimal_path_wide_validation.py', 'scripts/oracles/optimal_path_wide_recipes.py', 'scripts/oracles/optimal_paths_wide.py', 'scripts/oracles/optimal_path_instrumentation.py', 'scripts/oracles/optimal_paths_wide.c', 'scripts/oracles/ultra_paths.c', 'tests/fixtures/reference-ultra-path-recipes/recipes.json', 'docs/reference-ultra-path-protocol.md', 'docs/reference-ultra-path-adapter-review.md', 'tests/upstream/manifest.json', 'tests/fixtures/reference-optimal-paths/cases.bin']

def sha(data):
    return hashlib.sha256(data).hexdigest()


def manifest():
    result = []
    for name, request in recipes().items():
        replay = Replay(request)
        replay.run()
        result.append(dict(name=name, packet_sha256=sha(request.packet()), source_sha256=sha(request.original.source),
            source_bytes=len(request.original.source), policy=request.policy, script_positions=len(request.original.scripts), events=len(replay.events),
            queries=sum(e['stage']=='query' for e in replay.events), sequences=len(replay.sequences), alternatives=len(replay.witnesses), coverage=sorted(replay.coverage)))
    return dict(schema_version=1, revision=REVISION, cases=result,
        qualification='Frozen source-only callback inputs and independent branch expectations before C execution.')


@lru_cache(maxsize=1)
def manifest_bytes():
    return (json.dumps(manifest(),sort_keys=True,indent=2)+'\n').encode()


def source_manifest():
    result = {}
    for name in INPUTS:
        p = ROOT / name
        require(p.is_file() and not p.is_symlink(), 'regular source input: ' + name)
        result[name] = sha(p.read_bytes())
    require((ROOT/MANIFEST).read_bytes() == manifest_bytes(), 'exact source-only manifest')
    require(result['tests/upstream/manifest.json']=='3bb5ac1c2a0407b71c5e5dbaabac6b39074f5b7f851801a2056856a910f3ccaa','pinned source inventory')
    return result


def malformed_packets():
    base = recipes()['up_p2_wide_initial_4094'].packet()
    start = 32 + 4112
    result = {'short':base[:31], 'truncated':base[:-1], 'trailing':base+b'!', 'magic':b'!'+base[1:]}
    for name,offset,value in [('short-source',8,8),('source-cap',8,8193),('script-cap',12,65),
        ('minimum-low',16,2),('minimum-high',16,8),('target',20,4096),('mode',24,2),('policy',28,1),
        ('zero-position',start,0),('tail-position',start,4105),('zero-count',start+4,0),('count-cap',start+4,9),
        ('zero-offset',start+8,0),('unavailable-distance',start+8,5),('short-length',start+12,2),('tail-length',start+12,4112)]:
        result[name]=base[:offset]+struct.pack('<I',value)+base[offset+4:]
    result['source-mismatch']=base[:33]+b'b'+base[34:]
    for name,data in result.items():
        try:parse_packet(data)
        except ValueError:pass
        else:raise ValueError('negative packet accepted: '+name)
    return result


def negative_diagnostic(name):
    if name=='short':reason='packet size'
    elif name=='truncated':reason='candidate packet range'
    elif name=='trailing':reason='exact packet extent'
    elif name=='magic':reason='packet magic'
    elif name in ('short-source','source-cap','script-cap','minimum-low','minimum-high','target','mode','policy'):reason='request bounds'
    elif name in ('zero-position','tail-position','zero-count','count-cap'):reason='script bounds/order'
    elif name in ('zero-offset','unavailable-distance','short-length','tail-length'):reason='candidate intent/extent'
    elif name=='source-mismatch':reason='direct source bytes'
    else:raise ValueError('unknown malformed case')
    return 'ultra path oracle: '+reason+'\n'


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
    small=(ROOT/'scripts/oracles/optimal_paths_wide.c').read_bytes()
    wide=(ROOT/HELPER).read_bytes()
    diff=''.join(difflib.unified_diff(small.decode().splitlines(True),wide.decode().splitlines(True),fromfile='optimal_paths_wide.c',tofile='ultra_paths.c')).encode()
    return {'original_zstd_opt.c':original,'observed_zstd_opt.c':transformed,
            'instrumentation.json':(json.dumps(changes,sort_keys=True,indent=2)+'\n').encode(),
            'adapter-diff.patch':diff,'adapter-digests.json':(json.dumps(dict(small_sha256=sha(small),wide_sha256=sha(wide)),sort_keys=True)+'\n').encode()}


def plan(upstream,output,compiler,units):
    common=[str(compiler),'-std=c11','-g','-DDEBUGLEVEL=2','-DZSTD_DISABLE_ASM','-DZSTD_LEGACY_SUPPORT=0',
        '-UZSTD_MULTITHREAD','-DXXH_NAMESPACE=ZSTD_','-I',str(output/'generated'),'-I',str(upstream/'lib'),
        '-I',str(upstream/'lib/common'),'-I',str(upstream/'lib/compress')]
    rows=[]
    def add(label,argv,expected=0,diagnostic=None):
        rows.append(dict(label=label,argv=[str(v)for v in argv],expected_exit=expected,
            expected_diagnostic=diagnostic,timeout_seconds=120,stdout_limit=(128<<20)if '-run-up_'in label else(2<<20),stderr_limit=1<<20))
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
        executable=output/('c-'+mode)/'optimal-path'
        add(mode+'-link',base+[*objects,'-o',executable])
        for name in requests():add(mode+'-run-'+name,[executable,output/'requests'/(name+'.bin')],1 if name.startswith('negative-')else 0,negative_diagnostic(name[9:])if name.startswith('negative-')else None)
    require(len({r['label']for r in rows})==len(rows),'unique commands')
    return rows


def validate_plan(rows,upstream,output,compiler,units):
    require(json.dumps(rows,sort_keys=True)==json.dumps(plan(upstream,output,compiler,units),sort_keys=True),'exact bounded command inventory')
    return len(rows)
