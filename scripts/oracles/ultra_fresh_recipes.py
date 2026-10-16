"""Immutable ordinary Ultra recipes selected from source-only replay, never C output."""
from dataclasses import dataclass
from .ultra_fresh import Request,require
from .optimal_fresh_wide import Request as Original
from .optimal_fresh_wide_recipes import recipes as originals

@dataclass(frozen=True)
class Recipe:
    request:Request
    kinds:tuple

def alternative_source(seed):
    require(type(seed)is int and seed in(0,2),'frozen source-only seeds')
    state=0x144f5e+seed;data=bytearray();alphabet=[2,3,4,6][seed%4]
    for _ in range(192):
        state^=(state<<13)&0xffffffff;state^=state>>17;state^=(state<<5)&0xffffffff;state&=0xffffffff
        data.append(97+state%alphabet)
    prefix=b''.join(b'abbaabac'+bytes([128+i])for i in range(48))
    return prefix+bytes(data)

def recipes():
    result={}
    for policy in(0,2):
        for name,recipe in originals().items():
            result[f'uf_p{policy}_{name}']=Recipe(Request(policy,recipe.request),recipe.kinds)
        for seed in(0,2):
            source=alternative_source(seed)
            request=Original(source,(len(source),),window_log=10,hash_log=8,chain_log=8,search_log=6,min_match=3,target=128)
            result[f'uf_p{policy}_alternative_seed_{seed}']=Recipe(Request(policy,request),(2,))
    require(len(result)==44,'forty-four source-only ordinary recipes')
    for recipe in result.values():recipe.request.validate()
    return result
