"""Source-only larger retained schedules, before any new native observations."""
from dataclasses import dataclass,asdict
from .optimal_retained_frames import Request,balanced_blocks,require
from .optimal_retained_wide_recipes import recipes as predecessors,noise

@dataclass(frozen=True)
class Recipe:
    request:Request
    kinds:tuple


def recipes():
    old=predecessors()
    cases={name:Recipe(Request(**asdict(recipe.request)),recipe.kinds)for name,recipe in old.items()}
    def add(name,source,*,minimum=4,target=4095,window=15,hash_log=12,chain=12,search=6,mode=0,kinds):
        cases[name]=Recipe(Request(source,balanced_blocks(len(source)),window,hash_log,chain,search,minimum,target,mode),kinds)
    for n in (16385,24576,24577,32768):
        add('wide_frames_uniform_'+str(n),b'a'*n,kinds=(2,)+(1,)*(len(balanced_blocks(n))-1))
    random=noise(8192)
    add('wide_frames_copy',random*4,kinds=(0,2,2,2))
    add('wide_frames_mixed',b'a'*8192+random+b'a'*8192+random,kinds=(2,0,1,2))
    short=(b'abcXabcYabcZabcd'*2185)[:32768]
    add('wide_frames_short',short,minimum=3,target=1,kinds=(2,2,2,2))
    for minimum in (5,6,7):
        add('wide_frames_minimum_'+str(minimum),(b'abcdefghIJKL'*1366)[:16385],minimum=minimum,kinds=(2,2,2))
    add('wide_frames_horizon',old['wide_horizon'].request.source*2,mode=1,kinds=(2,2,2,2))
    add('wide_frames_small_tables',(b'abcdEFGH'*4096),window=16,hash_log=6,chain=6,search=1,target=0,kinds=(2,2,2,2))
    require(len(cases)==19,'exact source-only retained inventory')
    for recipe in cases.values():
        recipe.request.validate()
        require(len(recipe.kinds)==len(recipe.request.blocks),'complete publication hypothesis')
    return cases
