"""Pure bounded policy-tagged frequency command inventory; no native execution."""
from pathlib import Path
import json,struct
from oracles import ultra_frequency as m
from oracles import ultra_scalar as scalar
from oracles.ultra_frequency_recipes import recipes,manifest
from optimal_price_capture_plan import malformed_packets as old_malformed
ROOT=Path(__file__).resolve().parents[1]
HELPER='scripts/oracles/ultra_frequency.c'
MANIFEST='tests/fixtures/reference-ultra-frequency-recipes/recipes.json'
INPUTS=['scripts/ultra_frequency_capture_plan.py','scripts/capture_ultra_frequencies.py','scripts/check_ultra_frequency_capture.py','scripts/check_ultra_frequency_capture_plan.py',
'scripts/oracles/ultra_frequency.py','scripts/oracles/ultra_frequency_recipes.py','scripts/oracles/ultra_scalar.py','scripts/oracles/optimal_prices_v2.py','scripts/oracles/optimal_price_recipes.py','scripts/oracles/optimal_price_validation.py','scripts/optimal_price_capture_plan.py','scripts/check_ultra_frequency_model.py',HELPER,MANIFEST,'tests/fixtures/reference-optimal-prices/cases.bin','tests/upstream/manifest.json','docs/reference-ultra-frequency-protocol.md','docs/reference-ultra-frequency-adapter-review.md']
sha=scalar.sha

def source_manifest():
    result={}
    for name in INPUTS:
        p=ROOT/name;m.need(p.is_file()and not p.is_symlink(),'regular input '+name);result[name]=sha(p.read_bytes())
    m.need((ROOT/MANIFEST).read_text()==json.dumps(manifest(),sort_keys=True,indent=2)+'\n','frozen complete recipe manifest')
    m.need(result['tests/upstream/manifest.json']=='3bb5ac1c2a0407b71c5e5dbaabac6b39074f5b7f851801a2056856a910f3ccaa','pinned upstream inventory')
    m.need(result['tests/fixtures/reference-optimal-prices/cases.bin']=='c15e68d7c570b262cd3efb2e0be021cca3d0fc83afe6c26eb3527805b67978a4','original native whole-mode fixture')
    return result

REASONS={'short':'regular bounded packet','truncated':'exact packet extent','trailing':'exact packet extent','magic':'version 1 magic',
'source-limit':'header bounds','zero-count':'header bounds','count-limit':'header bounds','mode':'header bounds','init-tag':'initialize first',
'init-range':'source range','init-reserved':'initialization schedule','query-zero-offset':'query domain','query-short-match':'query domain',
'query-LL-limit':'query domain','query-range':'source range','update-reserved':'update count/reserved','dirty-query':'query lifecycle',
'policy-one':'header bounds','policy-limit':'header bounds','unknown-tag':'known tag'}

def malformed_packets():
    result={}
    for name,packet in old_malformed().items():
        new=m.MAGIC+packet[8:20]+struct.pack('<I',2)+packet[20:]
        if name=='magic':new=b'!'+new[1:]
        result[name]=new
    original=next(r for _,r,_ in recipes()if r.policy==2).packet()
    for name,policy in [('policy-one',1),('policy-limit',3)]:result[name]=original[:20]+struct.pack('<I',policy)+original[24:]
    source_length=struct.unpack_from('<I',original,8)[0];offset=24+source_length
    result['unknown-tag']=original[:offset]+struct.pack('<I',6)+original[offset+4:]
    m.need(set(result)==set(REASONS),'complete negative diagnostic inventory')
    for name,data in result.items():
        try:m.parse(data)
        except ValueError:pass
        else:raise ValueError('invalid packet accepted '+name)
    return result

def requests():return {**{name:r.raw_packet()for name,r,_ in recipes()},**{'negative-'+name:data for name,data in malformed_packets().items()}}

def outcomes():
    result={}
    for name,r,_ in recipes():
        try:r.validate()
        except ValueError:result[name]=(1,'fractional literal mass')
        else:result[name]=(0,None)
    result.update({'negative-'+name:(1,reason)for name,reason in REASONS.items()});return result

def reference_sources(upstream):
    catalog=json.loads((ROOT/'tests/upstream/manifest.json').read_bytes())
    m.need(catalog['upstream']['revision']==m.REVISION,'reference revision')
    sources={r['path']:r['sha256']for r in catalog['sources']+catalog['excluded_sources']if r['path'].startswith('lib/')and Path(r['path']).suffix in ('.c','.h')}
    for name,digest in sources.items():
        p=upstream/name;m.need(p.is_file()and not p.is_symlink()and sha(p.read_bytes())==digest,'pinned reference '+name)
    units=sorted(str(p.relative_to(upstream))for folder in ('common','compress','decompress')for p in (upstream/'lib'/folder).glob('*.c'))
    m.need(len(units)==26 and all(p in sources for p in units),'26used codec units')
    return sources,[p for p in units if p!='lib/compress/zstd_opt.c']

def plan(upstream,output,compiler,units):
    helper=ROOT/HELPER
    common=[str(compiler),'-std=c11','-g','-DDEBUGLEVEL=2','-DZSTD_DISABLE_ASM','-DZSTD_LEGACY_SUPPORT=0','-UZSTD_MULTITHREAD','-DXXH_NAMESPACE=ZSTD_',
            '-I',str(upstream/'lib'),'-I',str(upstream/'lib/common'),'-I',str(upstream/'lib/compress')]
    rows=[]
    def add(label,argv,expected=0,diagnostic=None):
        rows.append(dict(label=label,argv=[str(v)for v in argv],expected_exit=expected,expected_diagnostic=diagnostic,timeout_seconds=120,stdout_limit=2<<20,stderr_limit=1<<20))
    add('reference-revision',['git','-C',upstream,'rev-parse','HEAD'])
    add('reference-status',['git','-C',upstream,'status','--porcelain','--untracked-files=no'])
    add('compiler-version',[compiler,'--version']);add('compiler-target',[compiler,'-dumpmachine'])
    for mode in ('normal','sanitized'):
        flags=['-O2']if mode=='normal'else['-O1','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer']
        base=common+flags
        add(mode+'-dependencies',base+['-MM',helper,*[upstream/p for p in units]])
        add(mode+'-predefines',base+['-dM','-E',helper])
        objects=[]
        for index,unit in enumerate([*[upstream/p for p in units],helper]):
            obj=output/('c-'+mode)/f'unit-{index}.o';args=base+(['-Wall','-Wextra','-Werror']if unit==helper else[])
            add(mode+f'-compile-{index}',args+['-c',unit,'-o',obj]);objects.append(obj)
        executable=output/('c-'+mode)/'ultra-frequency'
        add(mode+'-link',base+[*objects,'-o',executable])
        for name,(expected,diagnostic)in outcomes().items():add(mode+'-run-'+name,[executable,output/'requests'/(name+'.bin')],expected,diagnostic)
    m.need(len({r['label']for r in rows})==len(rows),'unique labels');return rows

def validate_plan(rows,upstream,output,compiler,units):
    expected=plan(upstream,output,compiler,units)
    m.need(json.dumps(rows,sort_keys=True)==json.dumps(expected,sort_keys=True),'exact reviewed command inventory');return len(expected)
