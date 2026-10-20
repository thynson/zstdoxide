"""Source-only bounded Ultra2 public hypotheses over immutable Ultra inputs."""
from pathlib import Path
from dataclasses import replace
from functools import lru_cache
import collections,hashlib,json
ROOT=Path(__file__).resolve().parents[2]
REVISION='f8745da6ff1ad1e7bab384bd1f9d742439278e99'
MAX_SOURCE=4<<20
MAX_FRAME=5<<20
MAX_TOTAL_SOURCE=256<<20
PINS={'scripts/oracles/ultra_interop.py': 'c9d98ec9f8671d4e9a0596b4abdbda47282de93d1571718e4d74126ed6753108', 'scripts/oracles/double_fast_interop.py': '05eefcf492ce7dca0741d3348d165aca8a48e402aa63710d1c043775125705f3', 'scripts/oracles/ultra2_recipes.py': 'f897063592cc1c9dbf148d2dabcd7193d66a3e99c43b5ebb86a5b4052797a061', 'scripts/check_reference_admission_v8_v2.py': '3146694f9fcb7b0e1a2cfb7c2654df52f7049dd5c9aaf0e9fc0219e0317ffe29', 'scripts/check_lazy_interop_capture_v2.py': '0cb1ea0cef617b732c02537dfda09f5d4ac3080d2f3e2dccab6cb2c399d8410f', 'tests/fixtures/reference-admission-v8-v2/provenance.json': '4a1628e808a78cb8fd35256e68af5e285e3a5369e92dccb15ce5650807d2b269', 'docs/reference-ultra2-interop-protocol.md': '0701f4b0d3a51c9d5d67d2e447c7428b1744c3f7fa7ee8aba0e33c1692e58540', 'tests/fixtures/reference-ultra-interop-recipes/recipes.json': 'b498c6932e2fd85712608833dd7c745945cbea87d53bdfd009e2dae8a295fcd0', 'scripts/oracles/optimal_retained_wide_recipes.py': '1bcc4d2f5e3bc5b840e5c4639ecea939678f03d9315764967a75bcbc2fec37c6', 'tests/fixtures/reference-ultra2-recipes/recipes.json': '05025827219df1b3c4c1c4d752bea23f3a05e54814e0143a661ab17edc51e16a'}

def require(ok,why):
    if not ok:raise ValueError(why)
def sha(data):return hashlib.sha256(data).hexdigest()
def checked_inputs():
    for n,h in PINS.items():
        p=ROOT/n;require(p.is_file()and not p.is_symlink()and sha(p.read_bytes())==h,'pinned Ultra2 public input '+n)
@lru_cache(maxsize=1)
def prepared():
    checked_inputs()
    from oracles import ultra_interop as prior,double_fast_interop as common
    from oracles.ultra2_recipes import recipes as seeded
    from oracles.optimal_retained_wide_recipes import noise
    from check_reference_admission_v8_v2 import classify
    from check_lazy_interop_capture_v2 import presets,resolved_new_request
    table=presets();old=prior.recipes();manifest=json.loads((ROOT/'tests/fixtures/reference-ultra-interop-recipes/recipes.json').read_bytes());tuples=manifest['resolved_tuples']
    require([c.metadata()for c in old]==manifest['recipes'],'exact complete predecessor metadata')
    require(len(old)==len(tuples)==1345,'complete predecessor public inventory')
    result=[];resolved=[];origins=[];equal=[list(g)for g in manifest['expected_rust_frame_equivalence']]
    for c,v in zip(old,tuples,strict=True):
        if v[6]==9:
            independent=resolved_new_request(c,table);require(v==independent,'independent predecessor Ultra2 tuple');v=independent
        status=classify(v,len(c.source));require(c.expected!='success'or status=='success','preserve predecessor successes')
        result.append(replace(c,expected=status));resolved.append(v);origins.append('Preserved predecessor tuple; strategy9 independently reconstructed.')
    equal.append(['optimal-unsupported-9','ultra-unsupported-9'])
    def add(name,source,request,expected,why):
        c=common.Recipe(name,source,request,expected,why);v=resolved_new_request(c,table)
        require(classify(v,len(source))==expected,'independent new admission '+name)
        result.append(c);resolved.append(v);origins.append('Independent pinned-preset reconstruction.');return c,v
    seen=set()
    for name,recipe in seeded().items():
        r=recipe.request
        if not recipe.seed or len(r.source)<64:continue
        request=(16,r.window_log,r.chain_log,r.hash_log,r.search_log,r.min_match,r.target,9,0)
        key=(r.source,request)
        if key in seen:continue
        seen.add(key);add('ultra2-observed-'+name,r.source,request,'success','Complete seeded source; public automatic literals and independent known-size resolution.')
    require(len(seen)==26,'distinct observed seeded source/request pairs')
    aliases=[]
    for size in [64,512,1024,2048]:
        for level in [16,17]:
            for pattern,source in [('uniform',b'a'*size),('periodic',(b'abcXabcYabcZabcd'*((size+14)//15))[:size]),('noise',noise(size))]:
                c,v=add(f'ultra2-default-{level}-{size}-{pattern}',source,(level,0,0,0,0,0,0,0,0),'success','Default bounded Ultra2 level and source.')
                if pattern=='uniform':aliases.append((c,v))
    source=(b'abcXabcYabcZabcd'*1093)[:16384]
    for minimum in range(3,8):
        for target in [1,4095]:add(f'ultra2-minimum-{minimum}-target-{target}',source,(16,14,6,6,1,minimum,target,9,0),'success','Minimum geometry and target/horizon boundary.')
    for c,v in aliases:
        name=c.name+'-explicit';add(name,c.source,(1,*v,0),'success','Exact resolved tuple with a different requested level.');equal.append([c.name,name])
    base=(16,14,12,12,6,4,4095,9,0)
    for size in [0,1,7,8,9,63,16385,32768]:add(f'ultra2-resource-source-{size}',b'a'*size,base,'resource','Public complete-source boundary; private threshold controls remain private.')
    for name,index,value in [('window',1,10),('chain',2,13),('hash',3,13),('search',4,7),('target',6,4096)]:
        request=list(base);request[index]=value;add('ultra2-resource-'+name,b'a'*16384,tuple(request),'resource','Resolved geometry/target boundary.')
    for level,size in [(16,4096),(17,4096),*[(level,64)for level in range(18,23)]]:
        add(f'ultra2-resource-default-{level}-{size}',b'a'*size,(level,0,0,0,0,0,0,0,0),'resource','Default tuple exceeds the independently qualified envelope.')
    require(len(result)==1433 and len(equal)==25 and len({c.name for c in result})==len(result),'complete unique public recipe inventory')
    require(collections.Counter(c.expected for c in result)==dict(success=1292,resource=141),'fixed source-only classifications')
    require(sum(len(c.source)for c in result)<=MAX_TOTAL_SOURCE and all(len(c.source)<=MAX_SOURCE for c in result),'bounded regenerated source')
    return tuple(result),resolved,origins,equal

def recipes():return prepared()[0]
def manifest():
    cases,tuples,origins,equal=prepared()
    return dict(schema_version=8,upstream_revision=REVISION,status='Source-only pre-execution public Ultra2 hypotheses.',source_limit=MAX_SOURCE,frame_limit=MAX_FRAME,aggregate_source_limit=MAX_TOTAL_SOURCE,source_bytes=sum(len(c.source)for c in cases),recipe_count=len(cases),classifications=dict(collections.Counter(c.expected for c in cases)),input_sha256=PINS,request_fields=['level','window','chain','hash','search','minimum','target','strategy','hint'],recipes=[c.metadata()for c in cases],resolved_tuples=tuples,resolution_origins=origins,expected_rust_frame_equivalence=equal,families=dict(historical=1345,observed_seed_sources=26,defaults=24,minimum_target=10,equivalent_aliases=8,rejections=20),qualification='Every predecessor source/request byte retained. Every strategy9 and new tuple independently reconstructed. All prior successful Rust responses remain identity obligations. No public enabling, native execution, applied-C-scalar or cross-implementation entropy identity claimed.')
