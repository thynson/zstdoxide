"""Broader sorted-tree hypotheses containing only source and query inputs."""
from dataclasses import dataclass
import hashlib
import re
from .optimal_matches_broad import Request, REVISION, require


@dataclass(frozen=True)
class Recipe:
    name: str
    request: Request
    hypothesis: str

    def metadata(self):
        require(type(self.name) is str and re.fullmatch('om_[a-z0-9_]{1,60}', self.name), 'recipe identity')
        require(type(self.request) is Request and type(self.hypothesis) is str and 0 < len(self.hypothesis) <= 256, 'bounded recipe metadata')
        r = self.request
        packet = r.packet()
        return dict(id=self.name, source_bytes=len(r.source), source_sha256=hashlib.sha256(r.source).hexdigest(),
                    packet_bytes=len(packet), packet_sha256=hashlib.sha256(packet).hexdigest(),
                    parameters=[r.window_log,r.hash_log,r.chain_log,r.search_log,r.min_match,r.target,r.hash3_log],
                    queries=[list(q) for q in r.queries], hypothesis=self.hypothesis)


def recipes():
    result=[]
    def add(name,source,positions,hypothesis,minimum=3,window=19,target=4096,repeats=(0,0,0),ll0=0,search=4):
        request=Request(source,tuple((q,*repeats,ll0,3 if minimum==3 else 4) for q in positions),
                        window,12,12,search,minimum,target,min(window,17) if minimum==3 else 0)
        result.append(Recipe('om_broad_'+name,request,hypothesis))
    for window in (18,19):
        for distance in (262143,262144,262145):
            source=b'abcX'+b'z'*(distance-4)+b'abcY'+b'Q'*12
            add(f'distance_w{window}_d{distance}',source,(0,distance),
                'Independent inclusive window floor and strict short-key distance cutoff at 2^18.',window=window)
    for window in (10,14,17,18,19,21):
        add(f'ordinary_h3_w{window}',bytes(range(256))*8,(6,512,2040),
            'Ordinary CCtx short-key log min(window,17), with bounded direct primary/node geometry.',window=window)
    for minimum in (4,5,6,7):
        add(f'no_h3_m{minimum}',bytes(range(256))*8,(6,512,2040),
            'No short-key owner for effective minimum above three; primary width clamps to six.',minimum=minimum)
    repeat_source=b'!'*16+b'abcd'+b'z'*12+b'abcdefgh'+b'z'*8+b'abcdefghijkl'+b'z'*4+b'abcdefghijklmnop'+b'!!!!!!!!'
    for ll0,repeats in [(0,(48,32,16)),(1,(17,48,32))]:
        for target in (0,3,4,7,8,11,12,4095,4096):
            add(f'repeat_ll{ll0}_t{target}',repeat_source,(64,),
                'Repeat threshold equality/strictness and all intents before short-key publication.',
                window=17,target=target,repeats=repeats,ll0=ll0)
    for ll0 in (0,1):
        add(f'fresh_repeats_ll{ll0}',b'abcd'*32,(6,8,120),
            'Fresh repeat values 1/4/8 at pointer-safe query positions; ordinary parser startup remains separate.',
            window=17,repeats=(1,4,8),ll0=ll0)
    add('maximum',bytes(range(256))*2048,(*range(0,15*8,8),524280),
        'Maximum source/query extent and ordinary short-key log17; no large primary/node admission claim.',
        window=21,search=8)
    require(len(result)==len({r.name for r in result}),'unique recipes')
    for recipe in result:recipe.metadata()
    return result


def manifest():
    values=[r.metadata() for r in recipes()]
    return dict(schema_version=1,protocol='optimal-all-matches-broad-v2',revision=REVISION,
                status='source-only pre-execution hypotheses',recipes=values,recipe_count=len(values),
                source_bytes=sum(r['source_bytes'] for r in values),queries=sum(len(r['queries']) for r in values),
                qualification='Ordinary short-key logs and strict 2^18-distance hypotheses; bounded direct primary/node logs. No C execution, result maps, ordinary parser startup or public admission.')
