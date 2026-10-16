"""Pure bounded Ultra scalar native command/request inventory; no execution."""
from pathlib import Path
import json
import struct
from oracles import ultra_scalar as m
from oracles.ultra_scalar_recipes import recipes, manifest

ROOT=Path(__file__).resolve().parents[1]
HELPER='scripts/oracles/ultra_scalar.c'
MANIFEST='tests/fixtures/reference-ultra-scalar-recipes/recipes.json'
INPUTS=['scripts/ultra_scalar_capture_plan.py','scripts/capture_ultra_scalars.py',
        'scripts/check_ultra_scalar_capture.py','scripts/check_ultra_scalar_capture_plan.py',
        'scripts/oracles/ultra_scalar.py','scripts/oracles/ultra_scalar_recipes.py',
        'scripts/check_ultra_scalar_model.py',HELPER,MANIFEST,
        'docs/reference-ultra-scalar-protocol.md','docs/reference-ultra-scalar-adapter-review.md',
        'tests/upstream/manifest.json']
sha=m.sha

def source_manifest():
    result={}
    for name in INPUTS:
        p=ROOT/name;m.need(p.is_file()and not p.is_symlink(),'regular input '+name);result[name]=sha(p.read_bytes())
    m.need((ROOT/MANIFEST).read_text()==json.dumps(manifest(),sort_keys=True,indent=2)+'\n','frozen source-only inputs')
    m.need(result['tests/upstream/manifest.json']=='3bb5ac1c2a0407b71c5e5dbaabac6b39074f5b7f851801a2056856a910f3ccaa','pinned source inventory')
    return result

def malformed_packets():
    first=(2,0,0,0,0,0,0,0,0,0)
    good=m.packet((first,));bad={'short':good[:11],'truncated':good[:-1],'trailing':good+b'!','magic':b'!'+good[1:]}
    for name,value in [('zero-count',0),('count-limit',4097)]:bad[name]=good[:8]+struct.pack('<I',value)+good[12:]
    rows=[('mode',(1,0,0,0,0,0,0,0,0,0)),('tag',(2,5,0,0,0,0,0,0,0,0)),
          ('fraction-upper',(2,0,8388607,0,0,0,0,0,0,0)),('whole-overflow',(0,0,0xffffffff,0,0,0,0,0,0,0)),
          ('weight-reserved',(2,0,0,1,0,0,0,0,0,0)),('ll-limit',(2,1,131073,0,0,1,0,0,0,0)),
          ('ll-frequency',(2,1,1,1,2,0,0,0,0,0)),('ll-mode',(2,1,1,0,0,2,0,0,0,0)),
          ('ll-unused',(2,1,1,1,0,1,0,0,0,0)),('ll-reserved',(2,1,1,0,0,1,1,0,0,0)),
          ('zero-offset',(2,2,0,3,0,0,0,0,1,0)),('short-match',(2,2,1,2,0,0,0,0,1,0)),
          ('long-match',(2,2,1,131073,0,0,0,0,1,0)),('offset-frequency',(2,2,1,3,1,2,0,0,0,0)),
          ('match-frequency',(2,2,1,3,0,0,1,2,0,0)),('match-unused',(2,2,1,3,1,0,0,0,1,0)),
          ('match-reserved',(2,2,1,3,0,0,0,0,1,1)),('literal-limit',(2,3,131073,0,0,0,1,0,0,0)),
          ('literal-sum',(2,3,1,1,1,1,0,0,0,0)),('literal-zero-base',(0,3,1,0,0,0,0,0,0,0)),
          ('literal-unused',(2,3,1,1,1,0,1,0,0,0)),('literal-mode',(2,3,1,0,0,0,2,0,0,0)),
          ('literal-reserved',(2,3,1,0,0,0,1,0,1,0)),('increment-zero',(2,4,0,0,0,0,1,0,0,0)),
          ('increment-limit',(2,4,131073,0,0,0,1,0,0,0)),('increment-same',(2,4,17,2,1,2,0,0,0,0)),
          ('increment-sum',(2,4,1,2,1,2,0,0,0,0)),('increment-unused',(2,4,1,1,0,0,1,0,0,0)),
          ('increment-reserved',(2,4,1,0,0,0,1,1,0,0))]
    for name,row in rows:
        # Valid first row proves complete-packet validation precedes any output.
        bad[name]=m.MAGIC+struct.pack('<I',2)+struct.pack('<10I',*first)+struct.pack('<10I',*row)
    for name,data in bad.items():
        try:m.parse(data)
        except ValueError:pass
        else:raise ValueError('invalid request accepted '+name)
    return bad

def requests():
    return {'scalar-boundaries':m.packet(recipes()),**{'negative-'+n:p for n,p in malformed_packets().items()}}

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
    def add(label,argv,expected=0):
        rows.append(dict(label=label,argv=[str(v)for v in argv],expected_exit=expected,timeout_seconds=120,stdout_limit=2<<20,stderr_limit=1<<20))
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
        executable=output/('c-'+mode)/'ultra-scalar'
        add(mode+'-link',base+[*objects,'-o',executable])
        for name in requests():add(mode+'-run-'+name,[executable,output/'requests'/(name+'.bin')],1 if name.startswith('negative-')else 0)
    m.need(len({r['label']for r in rows})==len(rows),'unique labels');return rows

def validate_plan(rows,upstream,output,compiler,units):
    expected=plan(upstream,output,compiler,units)
    m.need(json.dumps(rows,sort_keys=True)==json.dumps(expected,sort_keys=True),'exact reviewed command inventory');return len(expected)
