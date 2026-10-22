"""Correct two pre-native wire hypotheses from the pinned literal threshold.

ZSTD_minLiteralsToCompress returns eight for strategy 9 with no reusable table.
Uniform first blocks of eight/nine bytes can encode an RLE literal section and
zero sequences. Seven bytes still use raw literals. Inputs are unchanged.
"""
from dataclasses import replace
from .ultra2_retained_recipes import Recipe, recipes as previous


def recipes():
    result = previous()
    for size in (8, 9):
        name = f'u2r_first_threshold_{size}'
        result[name] = replace(result[name], kinds=(2, 1))
    return result
