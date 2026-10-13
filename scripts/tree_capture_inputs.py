"""Pinned inputs shared by the tree capture controller and offline auditor."""
from dataclasses import dataclass
from pathlib import Path
import hashlib
import struct
from types import SimpleNamespace

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = 'tests/fixtures/reference-tree-tiny-recipes/recipes.json'
PINS = {'scripts/check_tree_recipes.py': '5f8106d6ddcadf91e898fe03978d3cc7cb4eb90edfdf4da6b56acd85767827f4', 'scripts/check_tree_validator.py': 'adfa9ffab892555909746007b9d431e7604a0ad64be8b97497db939d9c310763', 'scripts/oracles/tree_direct.py': '200c9491b82ae70598f7d5f0d375070ce858494f2c96304ba186329cd4f6d6f1', 'scripts/oracles/tree_validation.py': '59c16db4486f1a2f81eb995245c0c95cb8f07738edbe5792d5b54e00d7afcc9b', 'scripts/oracles/tree_direct_v1.c': 'f4351bd13926bffb5ebb7c8d976543dffb5fa04b7c9ab757d770b0a1de5b01e9', 'tests/fixtures/reference-tree-tiny-recipes/recipes.json': '6d23c53ce5305ae0e9e9d43c5bd97bc6f3006f0207e75bebbf7b1843462fdff4'}


def require(value, message):
    if not value:
        raise ValueError(message)


@dataclass(frozen=True)
class Case:
    name: str
    request: object

    def packet(self):
        return self.request.packet()


def inputs():
    for name, digest in PINS.items():
        path = ROOT / name
        require(path.is_file() and not path.is_symlink() and
                hashlib.sha256(path.read_bytes()).hexdigest() == digest, 'reviewed tree input: ' + name)
    import check_tree_recipes as checker
    from oracles import tree_direct as wire, tree_validation as replay
    checker.check()
    cases = [Case(name, request) for name, request in wire.recipes()]
    oracle = SimpleNamespace(REVISION=wire.REVISION, MAX_RECORDS=replay.MAX_OUTPUT,
                             MAX_PACKET=wire.MAX_PACKET, decode_json=replay.parse,
                             integer=wire.integer, exact=lambda a, b: replay.canonical(a) == replay.canonical(b),
                             validate=lambda case, data: replay.validate(case.request, data))
    manifest = dict(recipes=len(cases), packet_bytes=sum(len(c.packet()) for c in cases))
    return oracle, cases, manifest


def malformed_packets(_oracle, _cases):
    from oracles import tree_direct as wire
    original = wire.Request(b'a' * 32, (0, 1, 24)).packet()
    results = {}
    for name, offset, value in [('magic',0,0),('version',8,2),('header',12,84),
                              ('source-small',16,7),('source-large',16,131073),
                              ('count-zero',20,0),('count-large',20,257),
                              ('window',24,9),('hash',28,13),('chain',32,5),
                              ('search',36,9),('minimum',40,8),('target',44,1),
                              ('strategy',48,5),('frontier',52,1),('window-floor',56,1),
                              ('prefix-floor',60,1),('reserved',76,1),
                              ('operation',80,0),('tail',84,25),('duplicate-query',92,0)]:
        data = bytearray(original)
        struct.pack_into('<I',data,offset,value)
        results[name] = bytes(data)
    results.update(short=original[:79],truncated=original[:-1],extra=original+b'\0',
                   oversize=b'x'*(wire.MAX_PACKET+1))
    for name, packet in results.items():
        try:wire.decode_packet(packet)
        except (ValueError,TypeError,struct.error):pass
        else:raise ValueError('accepted negative recipe: '+name)
    require(len(results)==25,'negative packet inventory')
    return results
