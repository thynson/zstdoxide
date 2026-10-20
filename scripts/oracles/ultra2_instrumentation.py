"""Add only lifecycle observations to the frozen ordinary Ultra hooks."""
import hashlib

from .ultra_fresh_instrumentation import instrument as ordinary
from .ultra_scalar import need

PATCHES = [
    ('two-pass-entry',
     '    DEBUGLOG(5, "ZSTD_compressBlock_btultra2 (srcSize=%zu)", srcSize);',
     '    DEBUGLOG(5, "ZSTD_compressBlock_btultra2 (srcSize=%zu)", srcSize);\n'
     '    observer_entry(ms, seqStore, rep, src, srcSize);'),
    ('seed-phase',
     '    ZSTD_compressBlock_opt2(ms, seqStore, tmpRep, src, srcSize, ZSTD_noDict);',
     '    observer_seed_begin(ms, seqStore, rep);\n'
     '    ZSTD_compressBlock_opt2(ms, seqStore, tmpRep, src, srcSize, ZSTD_noDict);'),
    ('pre-rescale',
     '    ZSTD_rescaleFreqs(optStatePtr, (const BYTE*)src, srcSize, optLevel);',
     '    observer_pass_begin(ms, seqStore, rep, src, srcSize);\n'
     '    ZSTD_rescaleFreqs(optStatePtr, (const BYTE*)src, srcSize, optLevel);'),
    ('rebased-history',
     '    ms->window.lowLimit = ms->window.dictLimit;\n'
     '    ms->nextToUpdate = ms->window.dictLimit;',
     '    ms->window.lowLimit = ms->window.dictLimit;\n'
     '    ms->nextToUpdate = ms->window.dictLimit;\n'
     '    observer_reset(ms, seqStore, rep, src, srcSize);'),
]


def instrument(original):
    transformed, manifest = ordinary(original)
    text = transformed.decode()
    for name, before, after in PATCHES:
        need(text.count(before) == 1, 'unique Ultra2 anchor: ' + name)
        preserved = '\n'.join(line for line in after.splitlines()
                              if '    observer_' not in line)
        need(preserved == before, 'unchanged lifecycle statements')
        text = text.replace(before, after)
        manifest['substitutions'].append(dict(name=name, before=before, after=after, occurrences=1))
    reconstructed = text
    for patch in reversed(manifest['substitutions']):
        need(reconstructed.count(patch['after']) == 1, 'unique generated lifecycle hook')
        reconstructed = reconstructed.replace(patch['after'], patch['before'])
    need(reconstructed.encode() == original, 'exact original wrapper/provider/parser')
    result = text.encode()
    manifest.update(generated_sha256=hashlib.sha256(result).hexdigest(),
                    qualification='Nineteen read-only hooks. Removing them recovers every original '
                    'eligibility, first-pass call, sequence reset, coordinate and parser statement.')
    return result, manifest
