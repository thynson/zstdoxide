"""Source-only ordinary tree frame recipes; no codec or subprocess execution."""
from dataclasses import dataclass, fields
from pathlib import Path
import hashlib
import re

ROOT = Path(__file__).resolve().parents[2]
# Dependencies supply only immutable source recipes and bounded metadata.
PINS = {'scripts/oracles/greedy_frames.py': '360bf5139b7e2a886e9d2359c1ea258317238bd1906cfe9d123be33df7a0ce44', 'scripts/oracles/greedy_frame_floor_recipes.py': 'e30cb2c60762b333d6e4b510e9c9a5384c43a20e93209e89d52716f099bae2c4', 'scripts/oracles/tree_families.py': '42292c72c63ed9572d44bfaf301922b8b8f29eb4f833721b7b866c3f1193b5f4', 'scripts/oracles/tree_direct.py': '200c9491b82ae70598f7d5f0d375070ce858494f2c96304ba186329cd4f6d6f1', 'scripts/oracles/tree_validation.py': '59c16db4486f1a2f81eb995245c0c95cb8f07738edbe5792d5b54e00d7afcc9b'}
for name, digest in PINS.items():
    path = ROOT / name
    if not path.is_file() or path.is_symlink() or hashlib.sha256(path.read_bytes()).hexdigest() != digest:
        raise ValueError('frozen tree frame recipe dependency: ' + name)
from . import greedy_frames as base, greedy_frame_floor_recipes as floors, tree_families as direct

REVISION = base.REVISION
PROTOCOL = 'tree-ordinary-v1'
HEADERS = base.HEADERS
require = base.require

@dataclass(frozen=True)
class Recipe:
    name: str
    source: bytes
    schedule: tuple
    hypothesis: str
    window_log: int = 15
    hash_log: int = 10
    chain_log: int = 10
    search_log: int = 4
    min_match: int = 4
    target: int = 0
    whole_compare: bool = False

    def metadata(self):
        require(type(self.name) is str and re.fullmatch('bt_[a-z0-9_]{1,59}', self.name), 'tree recipe name')
        require(type(self.source) is bytes and len(self.source) <= 4 << 20, 'source extent/type')
        require(type(self.hypothesis) is str and 0 < len(self.hypothesis) <= 256, 'hypothesis')
        require(type(self.schedule) is tuple and len(self.schedule) <= 64, 'schedule count/type')
        for value,lo,hi in [(self.window_log,10,21),(self.hash_log,6,19),(self.chain_log,6,19),(self.search_log,1,8),(self.min_match,3,7),(self.target,0,131072)]:
            require(type(value) is int and lo <= value <= hi, 'tree parameter domain/type')
        limit=min(131072,1 << self.window_log,max(1,len(self.source)))
        require(all(type(n) is int and 0 <= n <= limit for n in self.schedule) and sum(self.schedule)==len(self.source), 'complete bounded schedule')
        require(type(self.whole_compare) is bool, 'whole flag')
        positive=[n for n in self.schedule if n]
        if self.whole_compare:
            require(positive and all(n==limit for n in positive[:-1]) and 0<positive[-1]<=limit, 'whole schedule alignment')
        return dict(id=self.name,source_bytes=len(self.source),source_sha256=hashlib.sha256(self.source).hexdigest(),schedule=list(self.schedule),parameters=[self.window_log,self.hash_log,self.chain_log,self.search_log,self.min_match,self.target,6],selected_branch='tree',whole_compare=self.whole_compare,hypothesis=self.hypothesis)

def recipes():
    result=[]
    for old in base.recipes()+floors.recipes():
        args={f.name:getattr(old,f.name) for f in fields(Recipe)}
        args.update(name='bt_'+old.name,chain_log=min(19,old.chain_log),search_log=min(8,old.search_log))
        result.append(Recipe(**args))
    for old in direct.recipes():
        r=old.request
        source=r.source
        w,c,h,s,m,t,g=r.metadata()['parameters']
        limit=min(131072,1 << w);schedule=tuple(min(limit,len(source)-p) for p in range(0,len(source),limit))
        result.append(Recipe('bt_direct_'+old.name.replace('-','_'),source,schedule,'Direct-search source under ordinary prefix sampling; no seeded tables or promised parser branch.',w,h,c,s,m,t,True))
    for w,h,c,s,m,t in [(21,19,19,8,3,0),(21,19,19,8,7,131072),(10,6,6,1,4,1),(15,12,19,8,5,65536)]:
        source=base.repeated(0x5eed1234,97,1024)
        result.append(Recipe(f'bt_bounds_w{w}_h{h}_c{c}_s{s}_m{m}_t{t}',source,(1024,),'Private maximum owner/search and target/minimum equivalence hypotheses.',w,h,c,s,m,t,True))
    require(len(result)==len({r.name for r in result}), 'unique tree recipe identities')
    for r in result:r.metadata()
    return result

def manifest():
    rows=[r.metadata() for r in recipes()]
    return dict(schema_version=1,protocol=PROTOCOL,revision=REVISION,status='source-only pre-execution hypotheses',pins=PINS,recipes=rows,recipe_count=len(rows),source_bytes=sum(r['source_bytes'] for r in rows),scheduled_calls=sum(len(r['schedule']) for r in rows),positive_calls=sum(sum(n>0 for n in r['schedule']) for r in rows),whole_comparisons=sum(r['whole_compare'] for r in rows),header_only=[dict(pledge=p,window_log=w,strategy=6,completed_frame=False) for p,w in HEADERS])
