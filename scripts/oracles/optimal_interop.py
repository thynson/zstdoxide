"""Source-only bounded Optimal public hypotheses over unchanged predecessors."""
from pathlib import Path
from dataclasses import replace
from functools import lru_cache
import collections,hashlib,json
ROOT=Path(__file__).resolve().parents[2]
REVISION='f8745da6ff1ad1e7bab384bd1f9d742439278e99'
MAX_SOURCE=4<<20
MAX_FRAME=5<<20
MAX_TOTAL_SOURCE=256<<20
PINS={'scripts/oracles/tree_interop.py': '0b5a07b558aefaa705628dc328f85228db257a48b71bcc564c7a7c334b324a40', 'scripts/oracles/double_fast_interop.py': '05eefcf492ce7dca0741d3348d165aca8a48e402aa63710d1c043775125705f3', 'scripts/oracles/optimal_fresh_wide_recipes.py': '49f93c8c570f857b636f4dd621094c9c96d71f7f953b1e6020bc84f9c5e6756d', 'scripts/check_reference_admission_v6.py': 'b73e2d9190ea5450c86d23b7e303826da8693a2fcb7c22f6d5a48ac186335391', 'scripts/check_lazy_interop_capture_v2.py': '0cb1ea0cef617b732c02537dfda09f5d4ac3080d2f3e2dccab6cb2c399d8410f', 'tests/fixtures/reference-admission-v6/provenance.json': '72c57c9f1bf51a6cfb1b18fbb7534e662d3edc881e0fdd4db775c199b76c2e3d', 'docs/reference-optimal-interop-protocol.md': '16fa1c6c09dbf0e3bf15d88fadb7b8358dc09054af683a72770d51f63c211b51', 'tests/fixtures/reference-tree-interop-recipes/recipes.json': 'd6200594584431fe14c7b481ace94d080f6651a9a518d371995182794d9be930', 'scripts/oracles/optimal_fresh_wide.py': 'a12a6745a47306f2a90b1fd0a88a91f2c1b6463830de845410a1f8b24136ed8c', 'scripts/oracles/optimal_retained_wide.py': '6a257560e27caabf42838978361d59f7f202d3ea9e2f2062d56a45f43af1c9de', 'scripts/oracles/optimal_retained_wide_recipes.py': '1bcc4d2f5e3bc5b840e5c4639ecea939678f03d9315764967a75bcbc2fec37c6', 'scripts/oracles/optimal_startup.py': '72baad2e13289bbef050a7692c181242da81c9adbb32a0ea32c4dd1cf126b588', 'scripts/oracles/optimal_startup_recipes.py': '6f75966526cf2a8fbf3e2f40856b381a24073c30b994b153d588fbc528c2957b', 'scripts/oracles/optimal_matches_broad.py': '01f57cdcfe92ed304fa36d8055b5a893880bcd178371432cc47d80ec60ecbf5f'}
def require(ok,why):
    if not ok:raise ValueError(why)
def sha(data):return hashlib.sha256(data).hexdigest()
def checked_inputs():
    for n,h in PINS.items():
        p=ROOT/n;require(p.is_file()and not p.is_symlink()and sha(p.read_bytes())==h,'pinned Optimal public input '+n)

@lru_cache(maxsize=1)
def prepared():
    checked_inputs()
    from oracles import tree_interop as prior,double_fast_interop as common
    from oracles.optimal_fresh_wide_recipes import recipes as fresh
    from oracles.optimal_retained_wide_recipes import noise
    from check_reference_admission_v6 import classify
    from check_lazy_interop_capture_v2 import presets,resolved_new_request
    table=presets();old=prior.recipes();prior_manifest=json.loads((ROOT/'tests/fixtures/reference-tree-interop-recipes/recipes.json').read_bytes());tuples=prior_manifest['resolved_tuples']
    require([c.metadata()for c in old]==prior_manifest['recipes'],'exact complete predecessor source/request metadata')
    require(len(old)==len(tuples)==1197,'complete predecessor public inventory')
    result=[];resolved=[];origins=[];equal=[]
    for c,v in zip(old,tuples,strict=True):
        if v[6]==7:
            independent=resolved_new_request(c,table);require(v==independent,'independent old Optimal tuple');v=independent
        status=classify(v,len(c.source));require(c.expected!='success'or status=='success','preserve previous successes')
        result.append(replace(c,expected=status));resolved.append(v);origins.append('preserved predecessor tuple; Optimal independently reconstructed')
    def add(name,source,request,expected,why):
        c=common.Recipe(name,source,request,expected,why);v=resolved_new_request(c,table)
        require(classify(v,len(source))==expected,'independent new admission '+name)
        result.append(c);resolved.append(v);origins.append('independent pinned-preset reconstruction');return c,v
    seen=set()
    for name,recipe in fresh().items():
        r=recipe.request;request=(11,r.window_log,r.chain_log,r.hash_log,r.search_log,r.min_match,r.target,7,0)
        key=(r.source,request)
        if key in seen:continue
        seen.add(key);add('optimal-observed-'+name,r.source,request,'success','Complete observed source; public automatic literal policy and known-size tuple resolution.')
    require(len(seen)==18,'distinct observed public source/request pairs')
    aliases=[]
    for size in [64,512,1024,2048]:
        for level in [11,12]:
            for pattern,source in [('uniform',b'a'*size),('periodic',(b'abcXabcYabcZabcd'*((size+14)//15))[:size]),('noise',noise(size))]:
                name=f'optimal-default-{level}-{size}-{pattern}'
                c,v=add(name,source,(level,0,0,0,0,0,0,0,0),'success','Default positive-level bounded Optimal request.')
                if pattern=='uniform':aliases.append((c,v))
    source=(b'abcXabcYabcZabcd'*1093)[:16384]
    for minimum in range(3,8):
        for target in [1,4095]:add(f'optimal-minimum-{minimum}-target-{target}',source,(11,14,6,6,1,minimum,target,7,0),'success','Minimum table geometry and sufficient/horizon target boundary.')
    for c,v in aliases:
        name=c.name+'-explicit';add(name,c.source,(1,*v,0),'success','Equivalent fully explicit resolved tuple with a different requested level.')
        equal.append([c.name,name])
    base=(11,14,12,12,6,4,4095,7,0)
    for size in [0,1,63,16385,32768]:add(f'optimal-resource-source-{size}',b'a'*size,base,'resource','Explicit complete-source resource boundary.')
    for name,index,value in [('window',1,10),('chain',2,13),('hash',3,13),('search',4,7),('target',6,4096)]:
        request=list(base);request[index]=value;add('optimal-resource-'+name,b'a'*16384,tuple(request),'resource','Resolved geometry/target resource boundary.')
    for strategy in [8,9]:
        request=list(base);request[7]=strategy;add(f'optimal-unsupported-{strategy}',b'a'*64,tuple(request),'unsupported','Unimplemented successor strategy remains distinct from resource limits.')
    require(len(result)==1269 and len(equal)==8 and len({c.name for c in result})==len(result),'complete unique recipe inventory')
    require(sum(len(c.source)for c in result)<=MAX_TOTAL_SOURCE,'aggregate source bound')
    require(all(len(c.source)<=MAX_SOURCE for c in result),'source cap')
    return tuple(result),resolved,origins,equal

def recipes():return prepared()[0]
def manifest():
    cases,tuples,origins,equal=prepared()
    return dict(schema_version=6,upstream_revision=REVISION,status='Source-only pre-execution public Optimal hypotheses.',source_limit=MAX_SOURCE,frame_limit=MAX_FRAME,aggregate_source_limit=MAX_TOTAL_SOURCE,source_bytes=sum(len(c.source)for c in cases),recipe_count=len(cases),classifications=dict(collections.Counter(c.expected for c in cases)),input_sha256=PINS,request_fields=['level','window','chain','hash','search','minimum','target','strategy','hint'],recipes=[c.metadata()for c in cases],resolved_tuples=tuples,resolution_origins=origins,expected_rust_frame_equivalence=equal,families=dict(historical=1197,observed_sources=18,defaults=24,minimum_target=10,equivalent_aliases=8,rejections=12),qualification='All original predecessor source/request bytes retained; every Optimal and every new tuple independently reconstructed. No native execution, public enabling, applied-C-scalar or C/Rust entropy identity claim.')
