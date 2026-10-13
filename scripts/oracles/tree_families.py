"""Broader deterministic tree inputs and explicitly conditional score hypotheses."""
from dataclasses import dataclass

from . import tree_direct as wire
from .tree_validation import key_bucket


@dataclass(frozen=True)
class FamilyCase:
    name: str
    request: wire.Request
    family: str
    hypothesis: tuple[tuple[str, int], ...] = ()
    control_of: str | None = None

    def metadata(self):
        return dict(name=self.name, family=self.family, control_of=self.control_of,
                    hypothesis=dict(self.hypothesis), **self.request.metadata(),
                    source_hex=self.request.source.hex())


def candidate_source(query, candidates, minimum, hash_log, seed):
    """Build exact input extents and a known last pre-query hash position.

    This bounds a source-only hash/construction search, not a tree algorithm.
    Candidates are (distance, exact_length), with nonoverlapping extents.
    """
    wire.require(candidates and all(0 < distance <= query and 6 <= length < 64
                                  for distance, length in candidates), 'candidate input geometry')
    extents = [(query - distance, query - distance + length + 1) for distance, length in candidates]
    for index, (start, end) in enumerate(extents):
        wire.require(end <= query, 'candidate precedes target')
        wire.require(all(end <= other_start or other_end <= start for other_start, other_end in extents[:index]),
                     'independent candidate extents')
    nearest = query - min(distance for distance, _ in candidates)
    for attempt in range(256):
        current_seed = seed + attempt * 0x10001
        source = bytearray(wire.noise(query + 128, current_seed))
        target = wire.noise(128, current_seed ^ 0x71ee5a17)
        source[query:] = target
        for distance, length in candidates:
            position = query - distance
            source[position:position + length] = target[:length]
            source[position + length] = target[length] ^ 0x80
        source = bytes(source)
        bucket = key_bucket(source, query, hash_log, minimum)
        if key_bucket(source, nearest, hash_log, minimum) != bucket:
            continue
        if any(key_bucket(source, position, hash_log, minimum) == bucket for position in range(nearest + 1, query)):
            continue
        for distance, length in candidates:
            position = query - distance
            wire.require(source[position:position + length] == source[query:query + length]
                         and source[position + length] != source[query + length], 'exact constructed match extent')
        return source, attempt
    raise ValueError('bounded source construction exhausted')


def tail_source(minimum, seed):
    for attempt in range(256):
        source = bytearray(wire.noise(72, seed + attempt))
        target = wire.noise(8, seed + attempt + 0x7a11)
        source[16:24] = target
        source[64:72] = target
        source = bytes(source)
        bucket = key_bucket(source, 64, 6, minimum)
        if key_bucket(source, 16, 6, minimum) == bucket and all(
                key_bucket(source, position, 6, minimum) != bucket for position in range(17, 64)):
            return source, attempt
    raise ValueError('bounded tail construction exhausted')


def recipes():
    cases = [FamilyCase('tiny-' + name, request, 'tiny-control', control_of=name)
             for name, request in wire.recipes()]
    seed = 0x5eedb712f001
    for minimum in range(3, 8):
        for near in (28, 29, 61):
            for far in (255, 511, 1023):
                query = 4096
                source, retry = candidate_source(query, ((near, 8), (far, 9)), minimum, 10, seed)
                seed += 0x1000000
                difference = (far + 1).bit_length() - (near + 3).bit_length()
                hypothesis = dict(query=query, near_distance=near, near_length=8,
                                  far_distance=far, far_length=9, weighted_length_gain=4,
                                  offset_bit_difference=difference, gain_minus_cost=4 - difference,
                                  construction_retry=retry)
                request = wire.Request(source, (0, query, query+1, query+2, query+7, query+8,
                                                query+9, query+10, query+16, query+32, query+64, len(source)-8),
                                       window_log=13, hash_log=10, chain_log=12, search_log=8, min_match=minimum)
                cases.append(FamilyCase(f'cost-m{minimum}-near{near}-far{far}', request, 'conditional-offset-cost', tuple(hypothesis.items())))
    for chain in (6, 7, 8):
        node_count = 1 << (chain - 1)
        for distance in (node_count - 2, node_count - 1, node_count, 2 * node_count):
            query = 2048
            source, retry = candidate_source(query, ((16, 8), (distance, 12)), 4, 10, seed)
            seed += 0x1000000
            request = wire.Request(source, (0, query, query+1, query+8, query+12, query+32, len(source)-8),
                                   window_log=12, hash_log=10, chain_log=chain, search_log=8)
            cases.append(FamilyCase(f'ring-c{chain}-distance{distance}',request,'node-floor',
                                   tuple(dict(query=query,near_distance=16,near_length=8,far_distance=distance,
                                              far_length=12,node_count=node_count,construction_retry=retry).items())))
    for window in (10, 11):
        for delta in (-1, 0, 1):
            distance = (1 << window) + delta
            query = 2 * (1 << window) + 16
            source, retry = candidate_source(query, ((distance, 16),), 4, 10, seed)
            seed += 0x1000000
            request = wire.Request(source, (0, query, query+1, query+8, query+16, len(source)-8),
                                   window_log=window, hash_log=10, chain_log=12, search_log=8)
            cases.append(FamilyCase(f'window-w{window}-distance{distance}',request,'window-floor',
                                   tuple(dict(query=query,distance=distance,length=16,window_delta=delta,
                                              construction_retry=retry).items())))
    for minimum in range(3, 8):
        source, retry = tail_source(minimum, seed)
        seed += 0x1000000
        cases.append(FamilyCase(f'tail-match-m{minimum}',wire.Request(source,(0,64),min_match=minimum),
                               'inclusive-matched-tail',tuple(dict(query=64,previous=16,length=8,construction_retry=retry).items())))
        source = (b'treeKey' * 586)[:4096]
        queries = (*range(128), 2047, 2048, len(source)-8)
        cases.append(FamilyCase(f'periodic-frontier-m{minimum}',wire.Request(source,queries,min_match=minimum),
                               'long-periodic-frontier'))
    wire.require(len(cases) == 103 and len({case.name for case in cases}) == 103,'family inventory')
    for case in cases:
        case.request.packet()
    return cases


def manifest():
    cases = recipes()
    return dict(schema_version=1,protocol='ZTREE01',upstream_revision=wire.REVISION,
                qualification='Inputs only. Candidate geometry and conditional score hypotheses are not observed C decisions, maps or branch traces.',
                recipes=[case.metadata() for case in cases])
