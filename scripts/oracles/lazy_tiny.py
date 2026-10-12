"""Tiny source hypotheses frozen before any lazy-family oracle execution."""
from dataclasses import fields

from . import greedy_direct as predecessor
from .lazy_direct import Recipe, require


def predecessor_controls():
    return [{'id': 'd0-' + c.name, 'predecessor_id': c.name} for c in predecessor.recipes()]


def recipes():
    result = []
    for depth in range(3):
        for old in predecessor.recipes():
            values = {f.name: getattr(old, f.name) for f in fields(predecessor.Recipe)}
            values['name'] = f'd{depth}-' + old.name
            result.append(Recipe(**values, depth=depth))
        for branch, label, margin in ((0, 'chain', 8), (1, 'row', 16)):
            # Repeated bytes make matches available near the strict main-loop
            # boundary. These are input hypotheses, not generated C outputs.
            for kind, prefix, block in (
                ('strict-tail', 0, margin + 1),
                ('first-tail', 0, margin + 2),
                ('second-tail', 0, margin + 3),
                ('prefix-first-tail', 8, margin + 1),
                ('prefix-second-tail', 8, margin + 2),
            ):
                result.append(Recipe(f'd{depth}-{label}-{kind}', b'A' * (prefix + block),
                                     branch=branch, prefix=prefix, next=prefix,
                                     seeds=(0,) if prefix else (), depth=depth))
    require(len(result) == len({c.name for c in result}) == 138, 'tiny lazy inventory')
    for case in result:
        case.packet()
    return result


def geometry_hypotheses(cases):
    selected = [c for c in cases if c.name.endswith('tail')]
    require(len(selected) == 30, 'tail geometry inventory')
    return [{'id': c.name, 'depth': c.depth, 'branch': c.branch,
             'first_query': c.prefix + int(c.prefix == c.floor),
             'main_limit': len(c.source) - (16 if c.branch else 8),
             'purpose': ('strict-main-boundary' if c.name.endswith('strict-tail')
                         else 'first-lookahead-limit' if c.name.endswith('first-tail')
                         else 'second-lookahead-limit')}
            for c in selected]
