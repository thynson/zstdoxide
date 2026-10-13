"""Source-only ordinary lazy-family recipes; never execute or predict a codec."""
from dataclasses import dataclass, fields
from pathlib import Path
import hashlib

ROOT = Path(__file__).resolve().parents[2]
PINS = {
    'scripts/oracles/greedy_frames.py': '360bf5139b7e2a886e9d2359c1ea258317238bd1906cfe9d123be33df7a0ce44',
    'scripts/oracles/greedy_frame_floor_recipes.py': 'e30cb2c60762b333d6e4b510e9c9a5384c43a20e93209e89d52716f099bae2c4',
    'scripts/oracles/greedy_direct.py': '616248428077b4b90b873aaef1282fdf4cea5bf39388e027d882c0aa1a1f9114',
    'scripts/oracles/greedy_families.py': 'b8f5be9467988ef60d1a8b9bbdfe7007c08512bc53753cc4ad2e18818b6a0e1f',
    'scripts/oracles/lazy_direct.py': '9a3e1cdb77e8e3d392cf2a3e6975ca53f69c73ae6e8d70a147e5a3950843314e',
    'scripts/oracles/lazy_families.py': 'f1c392d298ad51ff3b19894679c9dcee9a56f1fe22e6b15503a8bb14d3ca3ce3',
    'scripts/oracles/lazy_tiny.py': '6250504d4d87fd545e4b9dbd12b9e31e61dc132e8297da6587bfe468f205ca49',
}
for name, digest in PINS.items():
    path = ROOT / name
    if not path.is_file() or path.is_symlink() or path.stat().st_size > 128 << 10:
        raise ValueError('bounded regular dependency: ' + name)
    if hashlib.sha256(path.read_bytes()).hexdigest() != digest:
        raise ValueError('frozen recipe dependency: ' + name)

from . import greedy_frames as base, greedy_frame_floor_recipes as floors, lazy_families as direct

REVISION = base.REVISION
HEADERS = tuple((pledge, window, depth) for depth in range(3) for pledge, window in base.HEADERS)
require = base.require


@dataclass(frozen=True)
class Recipe(base.Recipe):
    depth: int = 0
    predecessor: str | None = None

    def metadata(self):
        require(type(self.depth) is int and 0 <= self.depth <= 2, 'depth domain/type')
        value = super().metadata()
        require(self.name.startswith(f'd{self.depth}_'), 'depth/name agreement')
        require(self.predecessor is None or (type(self.predecessor) is str
                and self.depth == 0 and self.name == 'd0_' + self.predecessor), 'predecessor identity')
        value['parameters'][-1] = 3 + self.depth
        return value | {'depth': self.depth, 'predecessor': self.predecessor}


def recipes():
    result = []
    historical = base.recipes() + floors.recipes()
    require(len(historical) == 138, 'predecessor count')
    for depth in range(3):
        for old in historical:
            values = {f.name: getattr(old, f.name) for f in fields(base.Recipe)}
            values['name'] = f'd{depth}_' + old.name
            result.append(Recipe(**values, depth=depth, predecessor=old.name if depth == 0 else None))
        for branch, window in [(0, 14), (1, 15)]:
            for source, _ in direct.threshold_hypotheses(branch):
                result.append(Recipe(
                    f'd{depth}_ordinary_' + source.name.replace('-', '_'), source.source,
                    (source.prefix, len(source.source) - source.prefix),
                    'Constructed byte copies under ordinary prefix sampling; no seeded maps, repeat overrides or promised score witness.',
                    window_log=window, hash_log=12, chain_log=12, search_log=4,
                    depth=depth,
                ))
    require(len(result) == len({r.name for r in result}) == 510, 'complete unique case inventory')
    for row in result:
        row.metadata()
    return result


def manifest():
    rows = [r.metadata() for r in recipes()]
    return {'schema_version': 1, 'protocol': 'lazy-family-ordinary-v1', 'revision': REVISION,
            'status': 'source-only pre-execution hypotheses', 'pins': PINS,
            'recipes': rows, 'recipe_count': len(rows),
            'source_bytes': sum(r['source_bytes'] for r in rows),
            'scheduled_calls': sum(len(r['schedule']) for r in rows),
            'positive_calls': sum(sum(n > 0 for n in r['schedule']) for r in rows),
            'whole_comparisons': sum(r['whole_compare'] for r in rows),
            'predecessor_count': sum(r['predecessor'] is not None for r in rows),
            'branches': {b: sum(r['selected_branch'] == b for r in rows) for b in ['chain', 'row']},
            'depth_counts': [sum(r['depth'] == d for r in rows) for d in range(3)],
            'header_only': [{'pledge': p, 'window_log': w, 'depth': d, 'completed_frame': False}
                            for p, w, d in HEADERS]}
