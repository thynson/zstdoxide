/* Observation-only complete Ultra2 lifecycle CCtx helper, pinned zstd v1.5.7.
 * Original provider and parser statements remain unchanged. LICENSE.zstd.
 * This source requires independent preflight/capture review before execution.
 */
#define _DARWIN_C_SOURCE
#define _POSIX_C_SOURCE 200809L
#define ZSTD_DISABLE_DEPRECATE_WARNINGS
#define ZSTD_STATIC_LINKING_ONLY
#include "compress/zstd_compress_internal.h"
#include <inttypes.h>
#include <stdint.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <unistd.h>
#define NONE UINT32_MAX
static void observer_entry(const ZSTD_MatchState_t*,const SeqStore_t*,const U32*,const void*,size_t);
static void observer_seed_begin(const ZSTD_MatchState_t*,const SeqStore_t*,const U32*);
static void observer_pass_begin(const ZSTD_MatchState_t*,const SeqStore_t*,const U32*,const void*,size_t);
static void observer_reset(const ZSTD_MatchState_t*,const SeqStore_t*,const U32*,const void*,size_t);
static void observer_pass_end(const U32*);
static void observer_before(const ZSTD_MatchState_t*,U32);
static void observer_event(const char*,const optState_t*,const BYTE*,const BYTE*,U32,U32,const ZSTD_optimal_t*,U32,U32,U32,const U32*,const ZSTD_match_t*,U32,U32,const U32*,U32);
static void observer_query(const optState_t*,const BYTE*,const BYTE*,U32,U32,const BYTE*,const U32*,U32,U32,const ZSTD_match_t*,U32,const U32*,U32);
static void observer_sequence(U32,const BYTE*,U32,U32);
static void observer_alternative(const optState_t*,const BYTE*,const BYTE*,U32,U32,const ZSTD_optimal_t*,int,int,int,int,U32,const U32*,U32);
#define PATH_ALTERNATIVE(c,l,old,inc,one,more,next,prev) observer_alternative(optStatePtr,ip,anchor,c,l,old,inc,one,more,next,prev,rep,nextToUpdate3)
#define PATH_OBSERVE(stage,c,l,stretch,rs,re,sp) observer_event(stage,optStatePtr,ip,anchor,c,l,stretch,rs,re,sp,NULL,NULL,0,NONE,rep,nextToUpdate3)
#define PATH_QUERY(c,l,qip,qrep,ll0,count) observer_query(optStatePtr,ip,anchor,c,l,qip,qrep,ll0,minMatch,matches,count,rep,nextToUpdate3)
#define STARTUP_SEARCH_BEGIN(ms,next3) observer_before(ms,next3)
#include "observed_zstd_opt.c"
#define SOURCE_CAP 16384u
#define PATH_CELLS 4099u
#define SEARCH_CELLS 73730u
_Static_assert(PATH_CELLS==ZSTD_OPT_SIZE,"complete ordinary path owner");
_Static_assert(64 % _Alignof(max_align_t)==0,"allocator alignment");
#define EVENT_CAP 65536u
#define SEQUENCE_CAP 5462u
#define ALLOCATION_CAP 32u
#define BYTE_CAP (8u<<20)
static struct { BYTE before[64],values[2*SOURCE_CAP+128],after[64]; } source;


static struct { BYTE *allocation,*data;size_t size;int live; } allocations[ALLOCATION_CAP];
static size_t allocation_count,live_bytes,peak_bytes;
static U32 before_heads[4096],before_nodes[4096],before_short[65536],before_frontier,before_short_frontier;
static int before_live;
static ZSTD_optimal_t prior_paths[PATH_CELLS];
static U32 prior_search[SEARCH_CELLS],scratch_search[SEARCH_CELLS];
static U32 sequence_records[SEQUENCE_CAP][4];
static BYTE packet[44+SOURCE_CAP];
static U32 price_policy;
static int alternative_active;
static int64_t alternative_values[12];
static U32 block_index,source_offset=44,block_tail;
static U32 seed_mode,sequence_prefix,total_events,total_passes;
static int seeding;
static U32 saved_repeats[3],observed_repeats[3];
static const U32*caller_repeats;
static char output_buffer[65536];
static U32 source_size,window_log,hash_log,chain_log,search_log,minimum,target,literal_mode,event_count,pass,sequence_count;
static BYTE padding;
static int reverse_phase;
static ZSTD_CCtx*context;
#define state (context->blockState.matchState)
#define store (context->seqStore)
static const BYTE*logical_source;
static void need(int ok,const char*why){if(!ok){fprintf(stderr,"Ultra2 observer: %s\n",why);exit(1);}}
static U32 little32(const BYTE*p){return(U32)p[0]|((U32)p[1]<<8)|((U32)p[2]<<16)|((U32)p[3]<<24);}
static size_t checked(size_t result,const char*why){need(!ZSTD_isError(result),why);return result;}
static void allocation_guards(void){size_t i,j;for(i=0;i<allocation_count;i++)if(allocations[i].live)for(j=0;j<64;j++)need(allocations[i].allocation[j]==0xa5&&allocations[i].data[allocations[i].size+j]==0xa5,"allocation guard");}
static void*zero_allocate(void*opaque,size_t size){BYTE*memory;size_t i=allocation_count;need(opaque==allocations,"allocator identity");need(i<ALLOCATION_CAP&&size>0&&size<=BYTE_CAP&&live_bytes<=BYTE_CAP-size,"allocation bounds");memory=(BYTE*)malloc(size+128);need(memory!=NULL,"allocation failure");memset(memory,0xa5,64);memset(memory+64,0,size);memset(memory+64+size,0xa5,64);allocations[i].allocation=memory;allocations[i].data=memory+64;allocations[i].size=size;allocations[i].live=1;allocation_count++;live_bytes+=size;if(live_bytes>peak_bytes)peak_bytes=live_bytes;return memory+64;}
static void release(void*opaque,void*pointer){size_t i;need(opaque==allocations,"free identity");if(pointer==NULL)return;allocation_guards();for(i=0;i<allocation_count;i++)if(allocations[i].live&&allocations[i].data==pointer){live_bytes-=allocations[i].size;allocations[i].live=0;free(allocations[i].allocation);return;}need(0,"foreign/double free");}
static int owned(const void*pointer,size_t bytes){size_t i;uintptr_t address=(uintptr_t)pointer;for(i=0;i<allocation_count;i++)if(allocations[i].live){uintptr_t base=(uintptr_t)allocations[i].data;if(address>=base&&address-base<=allocations[i].size&&bytes<=allocations[i].size-(address-base))return 1;}return 0;}
static void check_owners(void){
 need(context!=NULL&&owned(context,sizeof(*context)),"live CCtx owner");
 need(state.cParams.windowLog==window_log&&state.cParams.hashLog==hash_log&&state.cParams.chainLog==chain_log&&state.cParams.searchLog==search_log&&state.cParams.minMatch==minimum&&state.cParams.targetLength==target&&state.cParams.strategy==ZSTD_btultra2,"exact applied parameters");
 need(owned(context->blockState.prevCBlock,sizeof(*context->blockState.prevCBlock))&&owned(context->blockState.nextCBlock,sizeof(*context->blockState.nextCBlock))&&context->blockState.prevCBlock!=context->blockState.nextCBlock,"compressed state owners");
 need(state.hashLog3==(minimum==3?window_log:0),"ordinary short-key geometry");
 need(owned(state.hashTable,((size_t)1<<hash_log)*sizeof(U32))&&owned(state.chainTable,((size_t)1<<chain_log)*sizeof(U32)),"search owners");
 if(minimum==3)need(owned(state.hashTable3,((size_t)1<<window_log)*sizeof(U32)),"short owner");
 need(owned(state.opt.priceTable,ZSTD_OPT_SIZE*sizeof(ZSTD_optimal_t))&&owned(state.opt.matchTable,ZSTD_OPT_SIZE*sizeof(ZSTD_match_t)),"path/candidate owners");
 need(owned(state.opt.litFreq,256*sizeof(U32))&&owned(state.opt.litLengthFreq,36*sizeof(U32))&&owned(state.opt.matchLengthFreq,53*sizeof(U32))&&owned(state.opt.offCodeFreq,32*sizeof(U32)),"frequency owners");
 need(store.maxNbSeq<=21846&&store.maxNbLit<=131072&&owned(store.sequencesStart,store.maxNbSeq*sizeof(SeqDef))&&owned(store.litStart,store.maxNbLit),"sequence/literal owners");
}
static void guards(void){U32 i;allocation_guards();check_owners();
 need(state.ldmSeqStore==NULL&&state.dictMatchState==NULL&&state.loadedDictEnd==0,"no LDM or dictionary state");
 need(state.opt.symbolCosts==&context->blockState.prevCBlock->entropy&&state.opt.symbolCosts->huf.repeatMode==HUF_repeat_none&&state.opt.literalCompressionMode==(literal_mode?ZSTD_ps_disable:ZSTD_ps_auto),"fresh frontend entropy/literal policy");
 for(i=0;i<64;i++)need(source.before[i]==padding&&source.after[i]==padding&&source.values[i]==padding,"byte guards");
 for(i=0;i<SOURCE_CAP+64;i++)need(source.values[i]==padding,"source prefix guard");
 for(i=SOURCE_CAP+64+source_size;i<2*SOURCE_CAP+128;i++)need(source.values[i]==padding,"source tail");
 need(memcmp(logical_source,packet+source_offset,source_size)==0,"immutable source");
 for(i=PATH_CELLS;i<ZSTD_OPT_SIZE;i++){const ZSTD_optimal_t*c=state.opt.priceTable+i;need(c->price==0&&c->off==0&&c->mlen==0&&c->litlen==0&&c->rep[0]==0&&c->rep[1]==0&&c->rep[2]==0,"initialized unused path tail");}
 need(sequence_prefix+sequence_count<=store.maxNbSeq&&store.sequences==store.sequencesStart+sequence_prefix+sequence_count,"sequence publication");
}
static void array(const U32*,U32);
static void prefix_guard(void){if(sequence_prefix){SeqDef p=store.sequencesStart[0];need(sequence_prefix==1&&p.litLength==0&&p.mlBase==0&&p.offBase==1,"unchanged opaque sequence prefix");}}
static void search_snapshot(const U32*h,const U32*n,const U32*s,U32 frontier,U32 short_frontier,int advance){
 U32 i,count=0,changed=0;const U32 hn=1u<<hash_log,nn=1u<<chain_log,sn=minimum==3?1u<<window_log:0;
 need(hn+nn+sn+2<=SEARCH_CELLS,"bounded complete search snapshot");
 for(i=0;i<hn;i++)scratch_search[count++]=h[i];for(i=0;i<nn;i++)scratch_search[count++]=n[i];for(i=0;i<sn;i++)scratch_search[count++]=s[i];scratch_search[count++]=frontier;scratch_search[count++]=short_frontier;
 printf("{\"length\":%u,\"changes\":[",count);
 for(i=0;i<count;i++)if(prior_search[i]!=scratch_search[i]){if(changed++)putchar(',');printf("[%u,%u]",i,scratch_search[i]);if(advance)prior_search[i]=scratch_search[i];}
 fputs("]}",stdout);
}

static void observer_before(const ZSTD_MatchState_t*ms,U32 next3){need(ms==&state&&!before_live,"pre-query identity");guards();memcpy(before_heads,ms->hashTable,((size_t)1<<hash_log)*sizeof(U32));memcpy(before_nodes,ms->chainTable,((size_t)1<<chain_log)*sizeof(U32));if(minimum==3)memcpy(before_short,ms->hashTable3,((size_t)1<<window_log)*sizeof(U32));before_frontier=ms->nextToUpdate;before_short_frontier=next3;before_live=1;}
static void array(const U32*v,U32 n){U32 i;putchar('[');for(i=0;i<n;i++){if(i)putchar(',');printf("%u",v[i]);}putchar(']');}
static void cell(const ZSTD_optimal_t*c){printf("[%d,%u,%u,%u,%u,%u,%u]",c->price,c->off,c->mlen,c->litlen,c->rep[0],c->rep[1],c->rep[2]);}
static int same_cell(const ZSTD_optimal_t*a,const ZSTD_optimal_t*b){return a->price==b->price&&a->off==b->off&&a->mlen==b->mlen&&a->litlen==b->litlen&&a->rep[0]==b->rep[0]&&a->rep[1]==b->rep[1]&&a->rep[2]==b->rep[2];}
static void path_snapshot(void){U32 i,changed=0;printf("{\"length\":%u,\"changes\":[",PATH_CELLS);for(i=0;i<PATH_CELLS;i++)if(!same_cell(prior_paths+i,state.opt.priceTable+i)){if(changed++)putchar(',');printf("[%u,",i);cell(state.opt.priceTable+i);putchar(']');prior_paths[i]=state.opt.priceTable[i];}fputs("]}",stdout);}
static void optional(U32 v){if(v==NONE)fputs("null",stdout);else printf("%u",v);}
static U32 position(const BYTE*p){uintptr_t address=(uintptr_t)p,base=(uintptr_t)logical_source;need(address>=base&&address-base<=source_size,"source position");return(U32)(address-base);}
static void observer_query(const optState_t*opt,const BYTE*ip,const BYTE*anchor,U32 cur,U32 last,const BYTE*qip,const U32*rep,U32 ll0,U32 beat,const ZSTD_match_t*matches,U32 count,const U32*global_rep,U32 next3){
 U32 q[6]={position(qip),rep[0],rep[1],rep[2],ll0,beat};observer_event("query",opt,ip,anchor,cur,last,NULL,NONE,NONE,NONE,q,matches,count,NONE,global_rep,next3);
}
static void observer_sequence(U32 llen,const BYTE*anchor,U32 off,U32 length){
 U32 index=sequence_count;SeqDef actual;size_t consumed=0;U32 i;
 check_owners();need(index<SEQUENCE_CAP&&index<store.maxNbSeq&&store.sequences==store.sequencesStart+sequence_prefix+index+1,"bounded stored sequence");actual=store.sequencesStart[sequence_prefix+index];
 need(actual.litLength==llen&&actual.mlBase+3u==length&&actual.offBase==off,"exact upstream stored sequence");
 for(i=0;i<index;i++)consumed+=sequence_records[i][1];
 need(position(anchor)<=source_size&&llen<=source_size-position(anchor)&&consumed+llen<=SOURCE_CAP&&consumed+llen<=store.maxNbLit&&memcmp(store.litStart+consumed,anchor,llen)==0,"exact copied literals");
 sequence_records[index][0]=position(anchor);sequence_records[index][1]=llen;sequence_records[index][2]=length;sequence_records[index][3]=off;sequence_count++;
}
static void observer_event(const char*stage,const optState_t*opt,const BYTE*ip,const BYTE*anchor,U32 cur,U32 last,const ZSTD_optimal_t*stretch,U32 rs,U32 re,U32 sp,const U32*q,const ZSTD_match_t*matches,U32 count,U32 tail,const U32*global_rep,U32 next3){
 U32 i;need(opt==&state.opt&&event_count<EVENT_CAP&&count<=ZSTD_OPT_SIZE,"event bounds/owner");guards();
 if(strcmp(stage,"reverse_start")==0)reverse_phase=1;
 printf("{\"record\":\"event\",\"stage\":\"%s\",\"phase\":\"%s\",\"padding_pass\":%u,\"block\":%u,\"index\":%u,\"ip\":%u,\"anchor\":%u,\"cur\":",stage,reverse_phase?"reverse":"forward",pass,block_index,event_count++,position(ip),position(anchor));optional(cur);
 printf(",\"pass_name\":\"%s\",\"price_policy\":%u,\"alternative\":",seeding?"seeding":"encoding",price_policy);if(alternative_active){putchar('[');for(i=0;i<12;i++){if(i)putchar(',');printf("%" PRId64,alternative_values[i]);}putchar(']');}else fputs("null",stdout);
 fputs(",\"last\":",stdout);optional(last);fputs(",\"stretch\":",stdout);if(stretch)cell(stretch);else fputs("null",stdout);
 fputs(",\"reverse\":",stdout);if(rs==NONE)fputs("null",stdout);else printf("[%u,%u,%u]",rs,re,sp);
 fputs(",\"query\":",stdout);if(q)array(q,6);else fputs("null",stdout);
 fputs(",\"matches\":",stdout);if(q){putchar('[');for(i=0;i<count;i++){if(i)putchar(',');printf("[%u,%u]",matches[i].off,matches[i].len);}putchar(']');}else fputs("null",stdout);
 fputs(",\"tail\":",stdout);optional(tail);fputs(",\"repeats\":",stdout);array(global_rep,3);
 fputs(",\"cells\":",stdout);path_snapshot();fputs(",\"frequencies\":{\"lit\":",stdout);array(opt->litFreq,256);fputs(",\"ll\":",stdout);array(opt->litLengthFreq,36);fputs(",\"ml\":",stdout);array(opt->matchLengthFreq,53);fputs(",\"of\":",stdout);array(opt->offCodeFreq,32);
 printf("},\"sums\":{\"lit\":%u,\"ll\":%u,\"ml\":%u,\"of\":%u},\"base_prices\":{\"lit\":%u,\"ll\":%u,\"ml\":%u,\"of\":%u},\"predefined\":%s,\"sequences\":[",opt->litSum,opt->litLengthSum,opt->matchLengthSum,opt->offCodeSum,opt->litSumBasePrice,opt->litLengthSumBasePrice,opt->matchLengthSumBasePrice,opt->offCodeSumBasePrice,opt->priceType==zop_predef?"true":"false");
 for(i=0;i<sequence_count;i++){if(i)putchar(',');array(sequence_records[i],4);}
 fputs("],\"search_before\":",stdout);if(q){need(before_live,"pre-query snapshot");search_snapshot(before_heads,before_nodes,before_short,before_frontier,before_short_frontier,0);before_live=0;}else fputs("null",stdout);
 fputs(",\"search\":",stdout);search_snapshot(state.hashTable,state.chainTable,state.hashTable3,state.nextToUpdate,next3,1);
 puts(",\"source_unchanged\":true,\"guards_unchanged\":true,\"path_tail_zero\":true}");
 if(strcmp(stage,"refresh")==0)reverse_phase=0;
 if(strcmp(stage,"complete")==0){block_tail=tail;observer_pass_end(global_rep);}
}

static void observer_alternative(const optState_t*opt,const BYTE*ip,const BYTE*anchor,U32 cur,U32 last,const ZSTD_optimal_t*old,int increment,int one,int more,int next,U32 predecessor,const U32*global_rep,U32 next3){
 need(price_policy==2&&old!=NULL&&cur>=old->mlen&&predecessor==cur-old->mlen&&increment<0,"alternative bounds");
 alternative_values[0]=old->price;alternative_values[1]=old->off;alternative_values[2]=old->mlen;alternative_values[3]=old->litlen;
 alternative_values[4]=old->rep[0];alternative_values[5]=old->rep[1];alternative_values[6]=old->rep[2];alternative_values[7]=increment;
 alternative_values[8]=one;alternative_values[9]=more;alternative_values[10]=next;alternative_values[11]=predecessor;
 alternative_active=1;observer_event("alternative",opt,ip,anchor,cur,last,NULL,NONE,NONE,NONE,NULL,NULL,0,NONE,global_rep,next3);alternative_active=0;
}

static void ledger(const char*path){FILE*f=fopen(path,"wbx");size_t i;need(f!=NULL,"fresh ledger path");need(live_bytes==0,"all allocations released");fprintf(f,"{\"allocations\":[");for(i=0;i<allocation_count;i++){need(!allocations[i].live,"released owner");if(i)fputc(',',f);fprintf(f,"%zu",allocations[i].size);}fprintf(f,"],\"peak_bytes\":%zu,\"released\":true}\n",peak_bytes);need(!ferror(f)&&fclose(f)==0,"complete ledger");}


static void frequency(void){const optState_t*o=&state.opt;
 fputs("{\"tables\":{\"lit\":",stdout);array(o->litFreq,256);fputs(",\"ll\":",stdout);array(o->litLengthFreq,36);fputs(",\"ml\":",stdout);array(o->matchLengthFreq,53);fputs(",\"of\":",stdout);array(o->offCodeFreq,32);
 printf("},\"sums\":{\"lit\":%u,\"ll\":%u,\"ml\":%u,\"of\":%u},\"cache\":{\"lit\":%u,\"ll\":%u,\"ml\":%u,\"of\":%u},\"predefined\":%s}",o->litSum,o->litLengthSum,o->matchLengthSum,o->offCodeSum,o->litSumBasePrice,o->litLengthSumBasePrice,o->matchLengthSumBasePrice,o->offCodeSumBasePrice,o->priceType==zop_predef?"true":"false");
}
static U32 origin(void){need(logical_source>=state.window.base,"nonnegative source origin");return(U32)(logical_source-state.window.base);}
static void observer_entry(const ZSTD_MatchState_t*ms,const SeqStore_t*ss,const U32*rep,const void*src,size_t size){
 int a,b,c,d,e;caller_repeats=rep;need(ms==&state&&ss==&store&&src==logical_source&&size==source_size,"entry identities");guards();prefix_guard();
 a=ms->opt.litLengthSum==0;b=ss->sequences==ss->sequencesStart;c=ms->window.dictLimit==ms->window.lowLimit;d=origin()==ms->window.dictLimit;e=size>ZSTD_PREDEF_THRESHOLD;
 printf("{\"record\":\"eligibility\",\"padding_pass\":%u,\"predicates\":[%s,%s,%s,%s,%s],\"seed\":%s,\"current\":%u,\"dictionary_limit\":%u,\"low_limit\":%u,\"next_to_update\":%u,\"source_bytes\":%u,\"original_repeats\":",pass,a?"true":"false",b?"true":"false",c?"true":"false",d?"true":"false",e?"true":"false",a&&b&&c&&d&&e?"true":"false",origin(),ms->window.dictLimit,ms->window.lowLimit,ms->nextToUpdate,source_size);array(rep,3);printf(",\"sequence_prefix\":%u}\n",sequence_prefix);
}
static void observer_seed_begin(const ZSTD_MatchState_t*ms,const SeqStore_t*ss,const U32*rep){need(ms==&state&&ss==&store&&!seeding&&sequence_prefix==0,"seed identities");need(memcmp(rep,saved_repeats,sizeof(saved_repeats))==0,"original copied repeats");seeding=1;}
static void observer_pass_begin(const ZSTD_MatchState_t*ms,const SeqStore_t*ss,const U32*rep,const void*src,size_t size){
 need(ms==&state&&ss==&store&&src==logical_source&&size==source_size,"pass identities");guards();prefix_guard();need(memcmp(rep,saved_repeats,sizeof(saved_repeats))==0,"each pass starts with original repeats");need(seeding?(rep!=caller_repeats):(rep==caller_repeats),"copied seed versus original encoding repeat owner");
 event_count=0;reverse_phase=before_live=alternative_active=0;need(sequence_count==0,"empty appended sequence store");memset(sequence_records,0,sizeof(sequence_records));
 printf("{\"record\":\"pass_begin\",\"padding_pass\":%u,\"phase\":\"%s\",\"origin\":%u,\"repeats\":",pass,seeding?"seeding":"encoding",origin());array(rep,3);fputs(",\"frequency\":",stdout);frequency();puts("}");
}
static void observer_pass_end(const U32*rep){U32 i;guards();prefix_guard();
 printf("{\"record\":\"pass_end\",\"padding_pass\":%u,\"phase\":\"%s\",\"origin\":%u,\"candidate_repeats\":",pass,seeding?"seeding":"encoding",origin());array(rep,3);fputs(",\"sequences\":[",stdout);for(i=0;i<sequence_count;i++){if(i)putchar(',');array(sequence_records[i],4);}printf("],\"tail\":%u,\"frequency\":",block_tail);frequency();puts("}");memcpy(observed_repeats,rep,sizeof(observed_repeats));total_events+=event_count;total_passes++;
}
static void observer_reset(const ZSTD_MatchState_t*ms,const SeqStore_t*ss,const U32*rep,const void*src,size_t size){
 U32 i,at=0,literals=0;need(ms==&state&&ss==&store&&src==logical_source&&size==source_size&&seeding,"reset identities");
 need(ss->sequences==ss->sequencesStart&&ss->lit==ss->litStart,"native logical sequence/literal reset");
 for(i=0;i<(1u<<hash_log);i++)need(prior_search[at++]==ms->hashTable[i],"preserved hash cells");for(i=0;i<(1u<<chain_log);i++)need(prior_search[at++]==ms->chainTable[i],"preserved tree cells");if(minimum==3)for(i=0;i<(1u<<window_log);i++)need(prior_search[at++]==ms->hashTable3[i],"preserved short cells");
 for(i=0;i<sequence_count;i++)literals+=sequence_records[i][1];
 printf("{\"record\":\"reset\",\"padding_pass\":%u,\"discarded_sequences\":%u,\"discarded_literals\":%u,\"discarded_tail\":%u,\"caller_repeats\":",pass,sequence_count,literals,block_tail);array(rep,3);
 fputs(",\"discarded_repeats\":",stdout);array(observed_repeats,3);
 printf(",\"origin_before\":%u,\"origin_after\":%u,\"low_limit\":%u,\"dictionary_limit\":%u,\"next_to_update\":%u,\"frequency\":",origin()-source_size,origin(),ms->window.lowLimit,ms->window.dictLimit,ms->nextToUpdate);frequency();puts(",\"tables_preserved\":true}");
 need(memcmp(rep,saved_repeats,sizeof(saved_repeats))==0,"discarded pass leaves caller repeats unchanged");sequence_count=0;seeding=0;guards();
}
int main(int argc,char**argv){struct stat info;FILE*input;int descriptor;size_t size;U32 i;U64 fingerprint=UINT64_C(0xcbf29ce484222325);
 need(argc==4,"packet and two ledgers required");descriptor=open(argv[1],O_RDONLY|O_NOFOLLOW|O_NONBLOCK);need(descriptor>=0,"packet open");need(fstat(descriptor,&info)==0&&S_ISREG(info.st_mode),"regular packet");need(info.st_size>=51&&info.st_size<=(off_t)sizeof(packet),"packet size");size=(size_t)info.st_size;
 input=fdopen(descriptor,"rb");need(input!=NULL,"packet stream");need(fread(packet,1,size,input)==size&&fgetc(input)==EOF&&!ferror(input),"complete packet");need(fclose(input)==0,"packet close");need(memcmp(packet,"ZU2PS01\0",8)==0,"packet magic");
 source_size=little32(packet+8);seed_mode=little32(packet+12);window_log=little32(packet+16);hash_log=little32(packet+20);chain_log=little32(packet+24);search_log=little32(packet+28);minimum=little32(packet+32);target=little32(packet+36);literal_mode=little32(packet+40);price_policy=2;
 need(seed_mode<=1,"seeding mode");need(source_size>=7&&source_size<=SOURCE_CAP&&window_log>=10&&window_log<=16&&hash_log>=6&&hash_log<=12&&chain_log>=6&&chain_log<=12&&search_log>=1&&search_log<=6&&minimum>=3&&minimum<=7&&target<=4095&&literal_mode<=1,"request admission");need(source_size<=1u<<window_log,"source fits window");need(size==44+source_size,"complete source extent");
 for(i=0;i<source_size;i++)fingerprint=(fingerprint^packet[44+i])*UINT64_C(0x100000001b3);
 need(setvbuf(stdout,output_buffer,_IOFBF,sizeof(output_buffer))==0,"output buffer");
 printf("{\"record\":\"metadata\",\"protocol\":\"ZU2PS01\",\"revision\":\"f8745da6ff1ad1e7bab384bd1f9d742439278e99\",\"source_bytes\":%u,\"source_hash\":\"%016" PRIx64 "\",\"bias\":2,\"parameters\":[%u,%u,%u,%u,%u,%u,%u],\"seed\":%s,\"path_cells\":4099,\"padding_passes\":2,\"state_encoding\":\"canonical-deltas-from-zero\",\"price_policy\":2}\n",source_size,(uint64_t)fingerprint,window_log,hash_log,chain_log,search_log,minimum,target,literal_mode,seed_mode?"true":"false");
 for(pass=0;pass<2;pass++){
  ZSTD_parameters p;ZSTD_CCtx_params cp;ZSTD_customMem memory={zero_allocate,release,allocations};U32 rep[3]={1,4,8};size_t tail;
  padding=pass?0xc3:0x6a;event_count=sequence_count=0;seeding=reverse_phase=before_live=alternative_active=0;sequence_prefix=seed_mode?0:1;allocation_count=live_bytes=peak_bytes=0;memset(allocations,0,sizeof(allocations));memset(prior_paths,0,sizeof(prior_paths));memset(prior_search,0,sizeof(prior_search));memset(sequence_records,0,sizeof(sequence_records));memset(&source,padding,sizeof(source));logical_source=source.values+SOURCE_CAP+64;memcpy(source.values+SOURCE_CAP+64,packet+44,source_size);memcpy(saved_repeats,rep,sizeof(rep));
  memset(&p,0,sizeof(p));p.cParams.windowLog=window_log;p.cParams.hashLog=hash_log;p.cParams.chainLog=chain_log;p.cParams.searchLog=search_log;p.cParams.minMatch=minimum;p.cParams.targetLength=target;p.cParams.strategy=ZSTD_btultra2;p.fParams.contentSizeFlag=1;checked(ZSTD_checkCParams(p.cParams),"valid complete parameters");checked(ZSTD_CCtxParams_init_advanced(&cp,p),"advanced parameters");
  checked(ZSTD_CCtxParams_setParameter(&cp,ZSTD_c_nbWorkers,0),"single worker");checked(ZSTD_CCtxParams_setParameter(&cp,ZSTD_c_enableLongDistanceMatching,ZSTD_ps_disable),"disabled LDM");checked(ZSTD_CCtxParams_setParameter(&cp,ZSTD_c_splitAfterSequences,ZSTD_ps_disable),"disabled post splitting");checked(ZSTD_CCtxParams_setParameter(&cp,ZSTD_c_literalCompressionMode,literal_mode?ZSTD_ps_disable:ZSTD_ps_auto),"literal mode");
  context=ZSTD_createCCtx_advanced(memory);need(context!=NULL,"fresh CCtx");checked(ZSTD_compressBegin_advanced_internal(context,NULL,0,ZSTD_dct_auto,ZSTD_dtlm_fast,NULL,&cp,source_size<64?64:source_size),"private Begin");check_owners();
  ZSTD_resetSeqStore(&store);state.opt.symbolCosts=&context->blockState.prevCBlock->entropy;state.opt.literalCompressionMode=context->appliedParams.literalCompressionMode;state.window.base=logical_source-2;state.window.dictBase=state.window.base;state.window.nextSrc=logical_source+source_size;state.window.dictLimit=state.window.lowLimit=2;state.nextToUpdate=2;
  if(sequence_prefix){store.sequencesStart[0].litLength=0;store.sequencesStart[0].mlBase=0;store.sequencesStart[0].offBase=1;store.sequences++;}
  guards();prefix_guard();tail=ZSTD_compressBlock_btultra2(&state,&store,rep,logical_source,source_size);need(tail==block_tail&&!before_live&&!seeding,"complete wrapper return");guards();prefix_guard();checked(ZSTD_freeCCtx(context),"CCtx release");context=NULL;ledger(argv[2+pass]);
 }
 printf("{\"record\":\"complete\",\"padding_passes\":2,\"algorithm_passes\":%u,\"events\":%u,\"source_unchanged\":true,\"guards_unchanged\":true}\n",total_passes,total_events);need(fflush(stdout)==0&&!ferror(stdout),"complete stdout");return 0;
}
