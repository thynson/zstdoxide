"""Tiny sorted-tree hypotheses containing only source and query inputs."""
from dataclasses import dataclass
import hashlib
import re
from .optimal_matches import Request, REVISION, require


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
    result = []

    def add(name, source, positions, hypothesis, minimum=4, target=0, **geometry):
        queries = tuple((q,0,0,0,0,3 if minimum == 3 else 4) for q in positions)
        result.append(Recipe('om_' + name, Request(source,queries,min_match=minimum,target=target,
                             hash3_log=6 if minimum == 3 else 0,**geometry),hypothesis))

    three = b'abcXabcYuvwxabcZmoremore'
    for target in (0,1,2,3,4,4095,4096,131072):
        add(f'three_target_{target}',three,(0,1,4,8,12),'Three-byte candidates, independent short-key frontier and strict sufficient-length threshold.',3,target)
    for target in (0,131072):
        add(f'four_target_{target}',three,(0,1,4,8,12),'Same source with the four-byte minimum and no short-key owner.',4,target)
    mixed = bytes((i * 37 + i // 7) % 256 for i in range(256)) * 2
    for minimum in (3,4,5,6,7):
        add(f'width_{minimum}',mixed,(0,1,2,8,16,31,32,33,64,65,128,256,504),
            'Effective hash-width clamp, paired-slot wrap and complete source-end query footprint.',minimum,16)
    for minimum in (3,4):
        for target in (0,4095,4096):
            add(f'long_m{minimum}_t{target}',b'a' * 8192 + b'BBBBBBBB',
                (0,1,2,383,384,385,4094,4095,4096,8182,8183,8192),
                'Long match/end handling and skipped frontiers around the distinct 4095/4096 thresholds.',minimum,target)
    repeat_source = b'!' * 16 + b'abcd' + b'z' * 12 + b'abcdefgh' + b'z' * 8 + b'abcdefghijkl' + b'z' * 4 + b'abcdefghijklmnop' + b'!!!!!!!!'
    for index,(rep0,rep1,rep2,ll0) in enumerate(((48,32,16,0),(17,48,32,1),(0,0,0,1),(1,1,1,0))):
        result.append(Recipe(f'om_repeats_{index}',Request(repeat_source,((64,rep0,rep1,rep2,ll0,4),),target=131072),
                             'All repeat intents, literal-zero order, zero exclusion and rep0-minus-one.'))
    window_source=b'abcdefgh'+b'z'*1016+b'abcdefghTAILTAIL'
    for window in (10,17):
        add(f'window_{window}',window_source,(0,1023,1024,1031),
            'Prefix-boundary eligibility at exact window distance with live larger node storage.',4,131072,
            window_log=window,hash_log=12,chain_log=12,search_log=8)
    add('tiny_ring',b'abcdefgh' * 32,(0,31,32,33,63,64,65,127,128,248),
        'Paired-node overwrite floor and stale child termination across repeated ring wraps.',4,131072)
    maximum=bytes(range(256))*64
    queries=tuple((q,0,0,0,0,3) for q in (*range(0,63*256,256),len(maximum)-8))
    result.append(Recipe('om_maximum',Request(maximum,queries,17,12,12,8,3,131072,12),
                         'Maximum tiny request geometry and operation inventory; no ordinary CCtx hash3-log claim.'))
    require(len(result) == len({r.name for r in result}), 'unique recipes')
    for r in result:
        r.metadata()
    return result


def manifest():
    values=[r.metadata() for r in recipes()]
    return dict(schema_version=1,protocol='optimal-all-matches-tiny-v1',revision=REVISION,
                status='source-only pre-execution hypotheses',recipes=values,recipe_count=len(values),
                source_bytes=sum(r['source_bytes'] for r in values),queries=sum(len(r['queries']) for r in values),
                qualification='No C execution or predicted matches/maps. Bounded direct geometry; larger hash3-distance and ordinary-state gates remain open.')
