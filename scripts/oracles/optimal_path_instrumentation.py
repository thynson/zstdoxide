"""Unique-anchor observation additions to a pinned external zstd_opt.c copy."""
import hashlib
from .optimal_paths import require

# Each replacement preserves the original statement(s), except the deliberately
# substituted match-provider pointer. No path/price/repeat statement is replaced.
PATCHES = [
 ('scripted-provider', 'ZSTD_getAllMatchesFn getAllMatches = ZSTD_selectBtGetAllMatches(ms, dictMode);',
  'ZSTD_getAllMatchesFn getAllMatches = observer_matches;\n    (void)ZSTD_selectBtGetAllMatches;\n    (void)dictMode;'),
 ('begin', 'ip += (ip==prefixStart);',
  'ip += (ip==prefixStart);\n    PATH_OBSERVE("begin",NONE,NONE,NULL,NONE,NONE,NONE);'),
 ('initial-query', 'U32 nbMatches = getAllMatches(matches, ms, &nextToUpdate3, ip, iend, rep, ll0, minMatch);',
  'U32 nbMatches = getAllMatches(matches, ms, &nextToUpdate3, ip, iend, rep, ll0, minMatch);\n            PATH_QUERY(0,0,ip,rep,ll0,nbMatches);'),
 ('origin', 'ZSTD_memcpy(&opt[0].rep, rep, sizeof(opt[0].rep));',
  'ZSTD_memcpy(&opt[0].rep, rep, sizeof(opt[0].rep));\n            PATH_OBSERVE("origin",0,0,NULL,NONE,NONE,NONE);'),
 ('seed', 'last_pos = pos-1;\n                opt[pos].price = ZSTD_MAX_PRICE;',
  'last_pos = pos-1;\n                opt[pos].price = ZSTD_MAX_PRICE;\n                PATH_OBSERVE("seed",0,last_pos,NULL,NONE,NONE,NONE);'),
 ('literal', '/* last match must start at a minimum distance of 8 from oend */',
  'PATH_OBSERVE("literal",cur,last_pos,NULL,NONE,NONE,NONE);\n            /* last match must start at a minimum distance of 8 from oend */'),
 ('later-query', 'U32 nbMatches = getAllMatches(matches, ms, &nextToUpdate3, inr, iend, opt[cur].rep, ll0, minMatch);',
  'U32 nbMatches = getAllMatches(matches, ms, &nextToUpdate3, inr, iend, opt[cur].rep, ll0, minMatch);\n                PATH_QUERY(cur,last_pos,inr,opt[cur].rep,ll0,nbMatches);'),
 ('relax', 'opt[last_pos+1].price = ZSTD_MAX_PRICE;',
  'opt[last_pos+1].price = ZSTD_MAX_PRICE;\n            PATH_OBSERVE("relax",cur,last_pos,NULL,NONE,NONE,NONE);'),
 ('select', '_shortestPath:   /* cur, last_pos, best_mlen, best_off have to be set */',
  '_shortestPath:   /* cur, last_pos, best_mlen, best_off have to be set */\n        PATH_OBSERVE("select",cur,last_pos,&lastStretch,NONE,NONE,NONE);'),
 ('reverse-start', 'storeStart = storeEnd;\n            }\n            while (1) {',
  'storeStart = storeEnd;\n            }\n            PATH_OBSERVE("reverse_start",cur,last_pos,&lastStretch,storeStart,storeEnd,stretchPos);\n            while (1) {'),
 ('reverse', 'opt[storeStart].litlen = nextStretch.litlen;',
  'opt[storeStart].litlen = nextStretch.litlen;\n                PATH_OBSERVE("reverse",cur,last_pos,&lastStretch,storeStart,storeEnd,stretchPos);'),
 ('sequence-record', 'ZSTD_storeSeq(seqStore, llen, anchor, iend, offBase, mlen);',
  'ZSTD_storeSeq(seqStore, llen, anchor, iend, offBase, mlen);\n                    observer_sequence(llen,anchor,offBase,mlen);'),
 ('sequence-event', 'anchor += advance;\n                    ip = anchor;',
  'anchor += advance;\n                    ip = anchor;\n                    PATH_OBSERVE("sequence",cur,last_pos,&lastStretch,storeStart,storeEnd,storePos);'),
 ('refresh', 'ZSTD_setBasePrices(optStatePtr, optLevel);',
  'ZSTD_setBasePrices(optStatePtr, optLevel);\n            PATH_OBSERVE("refresh",cur,last_pos,&lastStretch,NONE,NONE,NONE);'),
 ('complete', 'return (size_t)(iend - anchor);',
  'observer_event("complete",optStatePtr,ip,anchor,NONE,NONE,NULL,NONE,NONE,NONE,NULL,NULL,0,(U32)(iend-anchor));\n    return (size_t)(iend - anchor);'),
]


def instrument(original):
    require(type(original) is bytes, 'owned original source')
    text = original.decode('utf-8')
    manifest = []
    for name, before, after in PATCHES:
        require(text.count(before) == 1, 'unique reviewed anchor: ' + name)
        preserved = '\n'.join(line for line in after.splitlines() if not any(token in line for token in ('PATH_OBSERVE(', 'PATH_QUERY(', 'observer_sequence(', 'observer_event(')))
        require(preserved.strip() == before.strip() or name == 'scripted-provider', 'preserved original statement: ' + name)
        text = text.replace(before, after)
        manifest.append(dict(name=name, before=before, after=after, occurrences=1))
    result = text.encode()
    return result, dict(original_sha256=hashlib.sha256(original).hexdigest(),
                        generated_sha256=hashlib.sha256(result).hexdigest(), substitutions=manifest)
