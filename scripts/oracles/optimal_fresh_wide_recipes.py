"""Source-only full fresh-block hypotheses; no provider scripting or native calls."""
from dataclasses import dataclass,asdict
from .optimal_fresh_wide import Request,require
from .optimal_startup_recipes import recipes as small
from .optimal_retained_wide_recipes import recipes as retained,noise

@dataclass(frozen=True)
class Recipe:
    request:Request
    kinds:tuple


def recipes():
    result={}
    for name,r in small().items():
        result['fresh_'+name]=Recipe(Request(r.source,(len(r.source),),r.window_log,r.hash_log,r.chain_log,
            r.search_log,r.min_match,r.target,r.literal_mode),(0 if name=='startup_noise' else 2,))
    for name,recipe in retained().items():
        r=recipe.request
        result['fresh_'+name]=Recipe(Request(**dict(asdict(r),blocks=(len(r.source),))),(2,))
    result['fresh_noise16k']=Recipe(Request(noise(16384),(16384,),window_log=14,hash_log=12,chain_log=12,search_log=6,target=4095),(0,))
    require(len(result)==20,'twenty complete fresh hypotheses')
    for recipe in result.values():recipe.request.validate()
    return result
