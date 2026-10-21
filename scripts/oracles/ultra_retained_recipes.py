"""Source-only retained policy recipes, frozen before successor native execution."""
from dataclasses import dataclass

from .ultra_retained import Request, Original, require
from .optimal_retained_frames_recipes import recipes as predecessors
from .ultra_fresh_recipes import alternative_source


@dataclass(frozen=True)
class Recipe:
    request: Request
    kinds: tuple


def recipes():
    result = {}
    old = predecessors()
    for policy in (0, 2):
        for name, recipe in old.items():
            result[f'ur_p{policy}_{name}'] = Recipe(Request(policy, recipe.request), recipe.kinds)
        for seeds in ((0, 2, 0, 2), (2, 0, 2, 0)):
            blocks = tuple(alternative_source(seed) for seed in seeds)
            source = b''.join(blocks)
            original = Original(source, tuple(map(len, blocks)), window_log=12,
                                hash_log=8, chain_log=8, search_log=6,
                                min_match=3, target=128)
            name = f'ur_p{policy}_alternatives_'+''.join(map(str, seeds))
            result[name] = Recipe(Request(policy, original), (2, 2, 2, 2))
    require(len(result) == 42, 'exact retained source-only inventory')
    for recipe in result.values():
        recipe.request.validate()
    return result
