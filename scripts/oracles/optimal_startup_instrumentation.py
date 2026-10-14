"""Add observation-only hooks while retaining the original Optimal provider."""
import hashlib
from .optimal_startup import require
from .optimal_path_instrumentation import PATCHES as SCRIPTED_PATCHES

ORIGINAL_SHA256='625936ee3fb02d789abb894c1fbc2918ee47582434094c963b630da13532c876'
PATCHES=[]
for name,before,after in SCRIPTED_PATCHES:
    if name=='scripted-provider':continue
    if name in ('initial-query','later-query'):
        indentation='            'if name=='initial-query'else'                '
        after='STARTUP_SEARCH_BEGIN(ms,nextToUpdate3);\n'+indentation+after
    if name=='complete':
        after=after.replace('(U32)(iend-anchor));','(U32)(iend-anchor),rep,nextToUpdate3);')
    PATCHES.append((name,before,after))


def instrument(original):
    require(type(original)is bytes,'owned pinned source')
    require(hashlib.sha256(original).hexdigest()==ORIGINAL_SHA256,'exact pinned Optimal source')
    text=original.decode();manifest=[]
    for name,before,after in PATCHES:
        require(text.count(before)==1,'unique observation anchor: '+name)
        preserved='\n'.join(line for line in after.splitlines()if not any(token in line for token in ('PATH_OBSERVE(','PATH_QUERY(','observer_sequence(','observer_event(','STARTUP_SEARCH_BEGIN(')))
        require(preserved.strip()==before.strip(),'unchanged algorithm statement: '+name)
        text=text.replace(before,after)
        manifest.append(dict(name=name,before=before,after=after,occurrences=1))
    selector='ZSTD_getAllMatchesFn getAllMatches = ZSTD_selectBtGetAllMatches(ms, dictMode);'
    require(text.count(selector)==1 and 'getAllMatches = observer_matches'not in text,'original provider retained')
    reconstructed=text
    for _,before,after in reversed(PATCHES):
        require(reconstructed.count(after)==1,'unique generated hook')
        reconstructed=reconstructed.replace(after,before)
    require(reconstructed.encode()==original,'exact original after observation removal')
    generated=text.encode()
    return generated,dict(original_sha256=ORIGINAL_SHA256,generated_sha256=hashlib.sha256(generated).hexdigest(),
        substitutions=manifest,provider_unchanged=True,qualification='Source-only observation additions; helper review and C preflight remain required.')
