"""Pinned broader inputs, retaining the original helper and tiny packet controls."""
from pathlib import Path
import hashlib

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = 'tests/fixtures/reference-tree-family-recipes/recipes.json'
PINS = {'docs/evidence/2026-10-13-tree-tiny.tar.gz': 'bd237f62c8b4b572d57042b6788520bff0bc167fead8823698f5d2dc3391cda1', 'scripts/check_tree_family_recipes.py': 'adde5d5d511a98e1716ec8671478cf6414192fe77a32a1936239123142395163', 'scripts/check_tree_recipes.py': '5f8106d6ddcadf91e898fe03978d3cc7cb4eb90edfdf4da6b56acd85767827f4', 'scripts/check_tree_validator.py': 'adfa9ffab892555909746007b9d431e7604a0ad64be8b97497db939d9c310763', 'scripts/oracles/tree_direct.py': '200c9491b82ae70598f7d5f0d375070ce858494f2c96304ba186329cd4f6d6f1', 'scripts/oracles/tree_direct_v1.c': 'f4351bd13926bffb5ebb7c8d976543dffb5fa04b7c9ab757d770b0a1de5b01e9', 'scripts/oracles/tree_families.py': '42292c72c63ed9572d44bfaf301922b8b8f29eb4f833721b7b866c3f1193b5f4', 'scripts/oracles/tree_validation.py': '59c16db4486f1a2f81eb995245c0c95cb8f07738edbe5792d5b54e00d7afcc9b', 'scripts/tree_capture_inputs.py': 'daed6ffef1f1f2c10893b7c9443e9cd16911de4089ef67dd6a758edd2f70ef28', 'tests/fixtures/reference-tree-family-recipes/recipes.json': 'b28cba0d6d827c82e579ce4131dfb6f5d8329094c47f409a41b47614656ce269', 'tests/fixtures/reference-tree-tiny-recipes/recipes.json': '6d23c53ce5305ae0e9e9d43c5bd97bc6f3006f0207e75bebbf7b1843462fdff4'}


def inputs():
    for name,digest in PINS.items():
        p=ROOT/name
        if not p.is_file() or p.is_symlink() or hashlib.sha256(p.read_bytes()).hexdigest()!=digest:
            raise ValueError('reviewed broader input: '+name)
    import tree_capture_inputs as tiny
    import check_tree_family_recipes as checker
    from oracles import tree_families as families
    checker.check()
    oracle,_,_=tiny.inputs()
    cases=[tiny.Case(case.name,case.request) for case in families.recipes()]
    return oracle,cases,dict(recipes=len(cases),packet_bytes=sum(len(c.packet()) for c in cases))


def malformed_packets(oracle,cases):
    import tree_capture_inputs as tiny
    return tiny.malformed_packets(oracle,cases)
