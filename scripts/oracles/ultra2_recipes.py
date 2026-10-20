"""Source-only two-pass recipes; no native output chooses these inputs."""
from dataclasses import asdict, dataclass

from .ultra2_validation import SourceRequest
from .ultra_fresh_recipes import recipes as ordinary
from .ultra_scalar import need


@dataclass(frozen=True)
class Recipe:
    request: SourceRequest
    seed: bool
    predecessor: str | None = None


def recipes():
    result = {}
    for name, recipe in ordinary().items():
        if recipe.request.policy != 2:
            continue
        source = SourceRequest(**asdict(recipe.request.original))
        for seed in (False, True):
            result[f'u2_{"seed" if seed else "skip"}_{name[6:]}'] = Recipe(source, seed, name)
    for size in (7, 8, 9):
        result[f'u2_threshold_{size}'] = Recipe(SourceRequest(b'a' * size, (size,), min_match=3), True)
    # Different residues modulo the 32-slot minimum tree test rebased physical
    # aliasing without treating a ring slot as permission to read old history.
    for size in (64, 65, 95, 96, 97, 129):
        pattern = b'abacabadabacabae'
        source = (pattern * ((size + len(pattern) - 1) // len(pattern)))[:size]
        result[f'u2_ring_{size}'] = Recipe(SourceRequest(source, (size,), chain_log=6,
                                                        min_match=3, search_log=6, target=128), True)
    need(len(result) == 53, 'fixed two-pass recipe inventory')
    for recipe in result.values():
        recipe.request.validate()
    return result
