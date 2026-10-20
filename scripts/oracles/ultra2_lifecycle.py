"""Independent Ultra2 eligibility and integer coordinates; no native execution.

Derived from facebook/zstd v1.5.7 zstd_opt.c, ZSTD_compressBlock_btultra2
and ZSTD_initStats_ultra. No pointer before a source allocation is formed.
This model describes lifecycle arithmetic, not public strategy admission.
"""
from dataclasses import dataclass, replace

from .ultra_scalar import need

U32_MAX = (1 << 32) - 1
MAX_BLOCK = 128 << 10
PREDEFINED_THRESHOLD = 8


def integer(value, low, high, name):
    need(type(value) is int and low <= value <= high, name)
    return value


@dataclass(frozen=True)
class Entry:
    literal_length_sum: int
    sequence_count: int
    dictionary_limit: int
    low_limit: int
    current: int
    source_bytes: int
    next_to_update: int

    def validate(self):
        for name in ('literal_length_sum', 'sequence_count', 'dictionary_limit',
                     'low_limit', 'current', 'next_to_update'):
            integer(getattr(self, name), 0, U32_MAX, 'entry ' + name)
        integer(self.source_bytes, 0, MAX_BLOCK, 'native block extent')
        need(self.low_limit <= self.dictionary_limit <= self.current,
             'ordered history limits')
        need(self.current + self.source_bytes <= U32_MAX, 'source index overflow')
        return self

    def predicates(self):
        self.validate()
        return (self.literal_length_sum == 0, self.sequence_count == 0,
                self.dictionary_limit == self.low_limit,
                self.current == self.dictionary_limit,
                self.source_bytes > PREDEFINED_THRESHOLD)

    def eligible(self):
        return all(self.predicates())

    def seeding_contract(self):
        need(self.eligible(), 'seeding requires all five predicates')
        # The upstream helper separately asserts unsigned dictLimit-frontier <= 1.
        # Do not silently turn this extra helper precondition into a sixth gate.
        need((self.dictionary_limit - self.next_to_update) & U32_MAX <= 1,
             'seeding helper update frontier')
        need(self.current + 2 * self.source_bytes <= U32_MAX,
             'rebased end index overflow')
        return self

    def rebased(self):
        self.seeding_contract()
        origin = self.current + self.source_bytes
        return replace(self, sequence_count=0, dictionary_limit=origin,
                       low_limit=origin, current=origin, next_to_update=origin)


@dataclass(frozen=True)
class Coordinates:
    """Map absolute integer indices onto one borrowed source interval.

    Zero-length ranges may name the end; positive reads cannot. This object
    carries no table ownership and does not clear, translate or dereference
    stale table cells. Ring slots are identities, never validity proofs.
    """
    origin: int
    source_bytes: int

    def validate(self):
        integer(self.origin, 0, U32_MAX, 'coordinate origin')
        integer(self.source_bytes, 0, MAX_BLOCK, 'coordinate extent')
        need(self.origin + self.source_bytes <= U32_MAX, 'coordinate end overflow')
        return self

    def absolute(self, offset):
        self.validate()
        integer(offset, 0, self.source_bytes, 'source offset')
        return self.origin + offset

    def offset(self, index, width=1):
        self.validate()
        integer(index, 0, U32_MAX, 'absolute index')
        integer(width, 0, self.source_bytes, 'read width')
        need(self.origin <= index <= self.origin + self.source_bytes - width,
             'current history read interval')
        return index - self.origin

    def candidate(self, index, current, width=1):
        self.offset(current, width)
        self.offset(index, width)
        need(index < current, 'strictly earlier candidate')
        return index - self.origin

    def rebase(self):
        self.validate()
        return Coordinates(self.origin + self.source_bytes, self.source_bytes).validate()


def skipped_ordinary(request, kinds, entry):
    """Preserve the ordinary model under a sequence-store-only gate blocker.

    Existing sequence-store content is an opaque prefix outside parser state.
    Returned observations describe the appended ordinary block only; they do
    not claim a native Ultra2 wrapper observation or encode that prefix.
    Other blockers require their corresponding nonfresh parser state and are
    deliberately not simulated by this fresh-only reconciliation helper.
    """
    from .ultra_fresh_validation import records
    request.validate()
    entry.validate()
    need(request.policy == 2 and entry.source_bytes == len(request.original.source),
         'same Ultra source and policy')
    need(entry.predicates() == (True, False, True, True, True)
         and entry.current == entry.next_to_update == 2,
         'fresh parser with only an opaque sequence prefix')
    return records(request, kinds)
