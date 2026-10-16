"""Source-only retained publication hypotheses; never supplied to C."""
from dataclasses import dataclass
from .optimal_retained import Request,require
from .optimal_startup_recipes import recipes as fresh_recipes

@dataclass(frozen=True)
class Recipe:
    request: Request
    kinds: tuple


def recipes():
    fresh=fresh_recipes();noise=fresh['startup_noise'].source;periodic=fresh['startup_periodic'].source
    raw=bytearray(noise[:256]);raw[128:132]=periodic[32:36]
    result={
        'retained_uniform':Recipe(Request(b'a'*128,(64,64)),(2,1)),
        'retained_periodic':Recipe(Request(periodic*2,(512,512)),(2,2)),
        'retained_noise_copy':Recipe(Request(noise*2,(512,512),hash_log=9,chain_log=9,search_log=4,target=4095),(0,2)),
        'retained_raw_rollback':Recipe(Request(periodic[:256]+bytes(raw)+periodic[:256],(256,256,256),target=4095),(2,0,2)),
        'retained_short':Recipe(Request((b'abcXabcYabcZabcd'*26)[:384],(128,128,128),min_match=3,target=4095),(2,2,2)),
        'retained_disabled_literals':Recipe(Request(periodic*2,(256,256,256,256),target=4095,literal_mode=1),(2,2,2,2)),
        'retained_larger_geometry':Recipe(Request(periodic*2,(512,512),window_log=12,hash_log=9,chain_log=9,search_log=4,min_match=3,target=16),(2,2)),
    }
    for minimum in range(3,8):
        result['retained_minimum_'+str(minimum)]=Recipe(Request(periodic*2,(512,512),min_match=minimum,target=4095),(2,2))
    require(len(result)==12,'twelve retained hypotheses')
    for recipe in result.values():
        recipe.request.validate();require(len(recipe.kinds)==len(recipe.request.blocks),'publication hypothesis count')
    return result
