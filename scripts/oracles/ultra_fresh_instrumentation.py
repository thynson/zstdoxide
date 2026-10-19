"""Read-only coupled-policy hooks retaining the original ordinary match provider."""
import hashlib
from .optimal_startup_instrumentation import instrument as previous
from .ultra_path_instrumentation import ANCHOR, ADDITION
from .ultra_fresh import require


def instrument(original):
    base, manifest = previous(original)
    text = base.decode()
    require(text.count(ANCHOR) == 1, 'unique ordinary saved-match anchor')
    text = text.replace(ANCHOR, ADDITION)
    manifest['substitutions'].append(dict(name='saved-match-alternative',
        before=ANCHOR, after=ADDITION, occurrences=1))
    reconstructed = text
    for patch in reversed(manifest['substitutions']):
        require(reconstructed.count(patch['after']) == 1, 'unique ordinary generated hook')
        reconstructed = reconstructed.replace(patch['after'], patch['before'])
    require(reconstructed.encode() == original, 'exact original ordinary provider and parser')
    result = text.encode()
    manifest['generated_sha256'] = hashlib.sha256(result).hexdigest()
    manifest['qualification'] = 'Fifteen read-only hooks; complete original provider and parser recovered by removal.'
    return result, manifest
