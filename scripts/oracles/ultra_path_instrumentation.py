"""Read-only alternative observation added to the frozen original path hooks."""
import hashlib
from .optimal_path_instrumentation import instrument as previous
from .ultra_scalar import need
ANCHOR='int withMoreLiterals = price + LIT_PRICE(ip+cur) + LL_INCPRICE(litlen+1);'
ADDITION=ANCHOR+'\n                        PATH_ALTERNATIVE(cur,last_pos,&prevMatch,LL_INCPRICE(1),with1literal,withMoreLiterals,opt[cur+1].price,cur-prevMatch.mlen);'

def instrument(original):
    base,manifest=previous(original);text=base.decode()
    need(text.count(ANCHOR)==1,'unique saved-match alternative anchor')
    text=text.replace(ANCHOR,ADDITION)
    need(ADDITION.split('\n')[0]==ANCHOR,'original alternative arithmetic preserved')
    result=text.encode();manifest['substitutions'].append(dict(name='saved-match-alternative',before=ANCHOR,after=ADDITION,occurrences=1));manifest['generated_sha256']=hashlib.sha256(result).hexdigest()
    return result,manifest
