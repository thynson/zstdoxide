"""Pre-execution retained Ultra2 sources with explicit publication hypotheses."""
from dataclasses import asdict, dataclass, replace

from .ultra2_retained import Request, SourceRequest, need
from .ultra_retained_recipes import recipes as retained


@dataclass(frozen=True)
class Recipe:
    request: Request
    kinds: tuple
    predecessor: str | None = None


def recipes():
    result = {}
    for name, old in retained().items():
        if old.request.policy == 2:
            r = SourceRequest(**asdict(old.request.original))
            result['u2r_' + name.removeprefix('ur_p2_')] = Recipe(Request(r), old.kinds, name)
    for length in (7, 8, 9):
        for first in (True, False):
            blocks = (length, 64) if first else (64, length)
            source = SourceRequest(b'a' * sum(blocks), blocks, min_match=3, target=128)
            kinds = (0, 1) if first else (2, 1)
            result[f'u2r_{"first" if first else "later"}_threshold_{length}'] = Recipe(Request(source), kinds)
    for first in (65, 95, 97):
        blocks = (first, 97)
        pattern = b'abacabadabacabae'
        source = (pattern * ((sum(blocks)+len(pattern)-1)//len(pattern)))[:sum(blocks)]
        r = SourceRequest(source, blocks, min_match=3, chain_log=6, search_log=6, target=128)
        result[f'u2r_ring_{first}'] = Recipe(Request(r), (2, 2))
    original = result['u2r_alternatives_0202']
    r = original.request.original
    for mask in (128, 255):
        source = r.source[:r.blocks[0]] + bytes(byte ^ mask for byte in r.source[r.blocks[0]:])
        changed = replace(r, source=source)
        result[f'u2r_future_{mask}'] = Recipe(Request(changed), original.kinds)
    need(len(result) == 32, 'complete source-only retained Ultra2 inventory')
    for recipe in result.values():
        recipe.request.validate()
        need(2 <= len(recipe.request.original.blocks) <= 4, 'native retained schedule')
    return result
