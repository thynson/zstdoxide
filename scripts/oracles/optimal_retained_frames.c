/* Observation-only larger sparse retained CCtx helper, pinned zstd v1.5.7.
 * Original provider and parser statements remain unchanged. LICENSE.zstd.
 * This source requires independent preflight/capture review before execution.
 */
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
#define NONE UINT32_MAX
static void observer_before(const ZSTD_MatchState_t*,U32);
static void observer_event(const char*,const optState_t*,const BYTE*,const BYTE*,U32,U32,const ZSTD_optimal_t*,U32,U32,U32,const U32*,const ZSTD_match_t*,U32,U32,const U32*,U32);
static void observer_query(const optState_t*,const BYTE*,const BYTE*,U32,U32,const BYTE*,const U32*,U32,U32,const ZSTD_match_t*,U32,const U32*,U32);
static void observer_sequence(U32,const BYTE*,U32,U32);
#define PATH_OBSERVE(stage,c,l,stretch,rs,re,sp) observer_event(stage,optStatePtr,ip,anchor,c,l,stretch,rs,re,sp,NULL,NULL,0,NONE,rep,nextToUpdate3)
#define PATH_QUERY(c,l,qip,qrep,ll0,count) observer_query(optStatePtr,ip,anchor,c,l,qip,qrep,ll0,minMatch,matches,count,rep,nextToUpdate3)
#define STARTUP_SEARCH_BEGIN(ms,next3) observer_before(ms,next3)
#include "observed_zstd_opt.c"
#define SOURCE_CAP 32768u
#define PATH_CELLS 4099u
#define SEARCH_CELLS 73730u
_Static_assert(PATH_CELLS==ZSTD_OPT_SIZE,"complete ordinary path owner");
_Static_assert(64 % _Alignof(max_align_t)==0,"allocator alignment");
#define EVENT_CAP 32768u
#define SEQUENCE_CAP 2731u
#define ALLOCATION_CAP 32u
#define BYTE_CAP (8u<<20)
static struct { BYTE before[64],values[SOURCE_CAP+128],after[64]; } source;
static struct { BYTE before[64],values[SOURCE_CAP+64],after[64]; } frame;
static struct { BYTE before[64],values[SOURCE_CAP],after[64]; } decoded;
static struct { BYTE *allocation,*data;size_t size;int live; } allocations[ALLOCATION_CAP];
static size_t allocation_count,live_bytes,peak_bytes;
static U32 before_heads[4096],before_nodes[4096],before_short[65536],before_frontier,before_short_frontier;
static int before_live;
static ZSTD_optimal_t prior_paths[PATH_CELLS];
static U32 prior_search[SEARCH_CELLS],scratch_search[SEARCH_CELLS];
static U32 sequence_records[SEQUENCE_CAP][4];
static BYTE packet[60+SOURCE_CAP];
static U32 block_count,block_lengths[4],block_index,source_offset,block_tail;
static char output_buffer[65536];
static U32 source_size,window_log,hash_log,chain_log,search_log,minimum,target,literal_mode,event_count,pass,sequence_count;
static BYTE padding;
static int reverse_phase;
static ZSTD_CCtx*context;
#define state (context->blockState.matchState)
#define store (context->seqStore)
static const BYTE*logical_source;
static void need(int ok,const char*why){if(!ok){fprintf(stderr,"Optimal retained observer: %s\n",why);exit(1);}}
static U32 little32(const BYTE*p){return(U32)p[0]|((U32)p[1]<<8)|((U32)p[2]<<16)|((U32)p[3]<<24);}
static size_t checked(size_t result,const char*why){need(!ZSTD_isError(result),why);return result;}
static void allocation_guards(void){size_t i,j;for(i=0;i<allocation_count;i++)if(allocations[i].live)for(j=0;j<64;j++)need(allocations[i].allocation[j]==0xa5&&allocations[i].data[allocations[i].size+j]==0xa5,"allocation guard");}
static void*zero_allocate(void*opaque,size_t size){BYTE*memory;size_t i=allocation_count;need(opaque==allocations,"allocator identity");need(i<ALLOCATION_CAP&&size>0&&size<=BYTE_CAP&&live_bytes<=BYTE_CAP-size,"allocation bounds");memory=(BYTE*)malloc(size+128);need(memory!=NULL,"allocation failure");memset(memory,0xa5,64);memset(memory+64,0,size);memset(memory+64+size,0xa5,64);allocations[i].allocation=memory;allocations[i].data=memory+64;allocations[i].size=size;allocations[i].live=1;allocation_count++;live_bytes+=size;if(live_bytes>peak_bytes)peak_bytes=live_bytes;return memory+64;}
static void release(void*opaque,void*pointer){size_t i;need(opaque==allocations,"free identity");if(pointer==NULL)return;allocation_guards();for(i=0;i<allocation_count;i++)if(allocations[i].live&&allocations[i].data==pointer){live_bytes-=allocations[i].size;allocations[i].live=0;free(allocations[i].allocation);return;}need(0,"foreign/double free");}
static int owned(const void*pointer,size_t bytes){size_t i;uintptr_t address=(uintptr_t)pointer;for(i=0;i<allocation_count;i++)if(allocations[i].live){uintptr_t base=(uintptr_t)allocations[i].data;if(address>=base&&address-base<=allocations[i].size&&bytes<=allocations[i].size-(address-base))return 1;}return 0;}
static void check_owners(void){
 need(context!=NULL&&owned(context,sizeof(*context)),"live CCtx owner");
 need(state.cParams.windowLog==window_log&&state.cParams.hashLog==hash_log&&state.cParams.chainLog==chain_log&&state.cParams.searchLog==search_log&&state.cParams.minMatch==minimum&&state.cParams.targetLength==target&&state.cParams.strategy==ZSTD_btopt,"exact applied parameters");
 need(owned(context->blockState.prevCBlock,sizeof(*context->blockState.prevCBlock))&&owned(context->blockState.nextCBlock,sizeof(*context->blockState.nextCBlock))&&context->blockState.prevCBlock!=context->blockState.nextCBlock,"compressed state owners");
 need(state.hashLog3==(minimum==3?window_log:0),"ordinary short-key geometry");
 need(owned(state.hashTable,((size_t)1<<hash_log)*sizeof(U32))&&owned(state.chainTable,((size_t)1<<chain_log)*sizeof(U32)),"search owners");
 if(minimum==3)need(owned(state.hashTable3,((size_t)1<<window_log)*sizeof(U32)),"short owner");
 need(owned(state.opt.priceTable,ZSTD_OPT_SIZE*sizeof(ZSTD_optimal_t))&&owned(state.opt.matchTable,ZSTD_OPT_SIZE*sizeof(ZSTD_match_t)),"path/candidate owners");
 need(owned(state.opt.litFreq,256*sizeof(U32))&&owned(state.opt.litLengthFreq,36*sizeof(U32))&&owned(state.opt.matchLengthFreq,53*sizeof(U32))&&owned(state.opt.offCodeFreq,32*sizeof(U32)),"frequency owners");
 need(store.maxNbSeq<=21846&&store.maxNbLit<=131072&&owned(store.sequencesStart,store.maxNbSeq*sizeof(SeqDef))&&owned(store.litStart,store.maxNbLit),"sequence/literal owners");
}
static void guards(void){U32 i;allocation_guards();check_owners();
 for(i=0;i<64;i++)need(source.before[i]==padding&&source.after[i]==padding&&source.values[i]==padding&&frame.before[i]==padding&&frame.after[i]==padding&&decoded.before[i]==padding&&decoded.after[i]==padding,"byte guards");
 for(i=64+source_size;i<SOURCE_CAP+128;i++)need(source.values[i]==padding,"source tail");
 need(memcmp(logical_source,packet+source_offset,source_size)==0,"immutable source");
 for(i=PATH_CELLS;i<ZSTD_OPT_SIZE;i++){const ZSTD_optimal_t*c=state.opt.priceTable+i;need(c->price==0&&c->off==0&&c->mlen==0&&c->litlen==0&&c->rep[0]==0&&c->rep[1]==0&&c->rep[2]==0,"initialized unused path tail");}
 need(sequence_count<=store.maxNbSeq&&store.sequences==store.sequencesStart+sequence_count,"sequence publication");
}
static void array(const U32*,U32);
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
 check_owners();need(index<SEQUENCE_CAP&&index<store.maxNbSeq&&store.sequences==store.sequencesStart+index+1,"bounded stored sequence");actual=store.sequencesStart[index];
 need(actual.litLength==llen&&actual.mlBase+3u==length&&actual.offBase==off,"exact upstream stored sequence");
 for(i=0;i<index;i++)consumed+=sequence_records[i][1];
 need(position(anchor)<=source_size&&llen<=source_size-position(anchor)&&consumed+llen<=SOURCE_CAP&&consumed+llen<=store.maxNbLit&&memcmp(store.litStart+consumed,anchor,llen)==0,"exact copied literals");
 sequence_records[index][0]=position(anchor);sequence_records[index][1]=llen;sequence_records[index][2]=length;sequence_records[index][3]=off;sequence_count++;
}
static void observer_event(const char*stage,const optState_t*opt,const BYTE*ip,const BYTE*anchor,U32 cur,U32 last,const ZSTD_optimal_t*stretch,U32 rs,U32 re,U32 sp,const U32*q,const ZSTD_match_t*matches,U32 count,U32 tail,const U32*global_rep,U32 next3){
 U32 i;need(opt==&state.opt&&event_count<EVENT_CAP&&count<=ZSTD_OPT_SIZE,"event bounds/owner");guards();
 if(strcmp(stage,"reverse_start")==0)reverse_phase=1;
 printf("{\"record\":\"event\",\"stage\":\"%s\",\"phase\":\"%s\",\"padding_pass\":%u,\"block\":%u,\"index\":%u,\"ip\":%u,\"anchor\":%u,\"cur\":",stage,reverse_phase?"reverse":"forward",pass,block_index,event_count++,position(ip),position(anchor));optional(cur);
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
 if(strcmp(stage,"complete")==0)block_tail=tail;
}

static void artifact(const char*path,const void*data,size_t size){FILE*f=fopen(path,"wbx");need(f!=NULL,"fresh artifact path");need(fwrite(data,1,size,f)==size&&!ferror(f),"complete artifact");need(fclose(f)==0,"artifact close");}
static void ledger(const char*path){FILE*f=fopen(path,"wbx");size_t i;need(f!=NULL,"fresh ledger path");need(live_bytes==0,"all allocations released");fprintf(f,"{\"allocations\":[");for(i=0;i<allocation_count;i++){need(!allocations[i].live,"released owner");if(i)fputc(',',f);fprintf(f,"%zu",allocations[i].size);}fprintf(f,"],\"peak_bytes\":%zu,\"released\":true}\n",peak_bytes);need(!ferror(f)&&fclose(f)==0,"complete ledger");}

static U32 owner_index(const ZSTD_compressedBlockState_t*p,ZSTD_compressedBlockState_t*const*owners){need(p==owners[0]||p==owners[1],"exact compressed state owner");return p==owners[1];}
static void owner_repeats(ZSTD_compressedBlockState_t*const*owners){putchar('[');array(owners[0]->rep,3);putchar(',');array(owners[1]->rep,3);putchar(']');}
int main(int argc,char**argv){struct stat info;FILE*input;size_t size;U32 i,total_events=0,sum=0;U64 fingerprint=UINT64_C(0xcbf29ce484222325);
 need(argc==6,"packet, two frames and two ledgers required");need(stat(argv[1],&info)==0&&S_ISREG(info.st_mode)&&info.st_size>=180&&info.st_size<=(off_t)sizeof(packet),"regular bounded packet");size=(size_t)info.st_size;input=fopen(argv[1],"rb");need(input!=NULL,"packet open");need(fread(packet,1,size,input)==size&&fgetc(input)==EOF&&!ferror(input),"complete packet");need(fclose(input)==0,"packet close");need(memcmp(packet,"ZOPRS03\0",8)==0,"packet magic");
 source_size=little32(packet+8);block_count=little32(packet+12);window_log=little32(packet+16);hash_log=little32(packet+20);chain_log=little32(packet+24);search_log=little32(packet+28);minimum=little32(packet+32);target=little32(packet+36);literal_mode=little32(packet+40);
 need(source_size>=128&&source_size<=SOURCE_CAP&&block_count>=2&&block_count<=4&&window_log>=10&&window_log<=16&&hash_log>=6&&hash_log<=12&&chain_log>=6&&chain_log<=12&&search_log>=1&&search_log<=6&&minimum>=3&&minimum<=7&&target<=4095&&literal_mode<=1,"request admission");
 need(source_size<=1u<<window_log,"source fits retained window");source_offset=44+4*block_count;need(size==source_offset+source_size,"complete retained source extent");
 for(i=0;i<block_count;i++){block_lengths[i]=little32(packet+44+4*i);need(block_lengths[i]>=64&&block_lengths[i]<=8192,"block length");sum+=block_lengths[i];}need(sum==source_size,"exact schedule total");
 for(i=0;i<source_size;i++)fingerprint=(fingerprint^packet[source_offset+i])*UINT64_C(0x100000001b3);
 need(setvbuf(stdout,output_buffer,_IOFBF,sizeof(output_buffer))==0,"output buffer");
 printf("{\"record\":\"metadata\",\"protocol\":\"ZOPRS03\",\"revision\":\"f8745da6ff1ad1e7bab384bd1f9d742439278e99\",\"source_bytes\":%u,\"source_hash\":\"%016" PRIx64 "\",\"bias\":2,\"parameters\":[%u,%u,%u,%u,%u,%u,%u],\"blocks\":",source_size,(uint64_t)fingerprint,window_log,hash_log,chain_log,search_log,minimum,target,literal_mode);array(block_lengths,block_count);puts(",\"path_cells\":4099,\"passes\":2,\"state_encoding\":\"canonical-deltas-from-zero\"}");
 for(pass=0;pass<2;pass++){
  ZSTD_parameters p;ZSTD_CCtx_params cp;ZSTD_customMem memory={zero_allocate,release,allocations};size_t written=0,consumed=0,result;ZSTD_compressedBlockState_t*owners[2];
  padding=pass?0xc3:0x6a;event_count=sequence_count=0;reverse_phase=before_live=0;allocation_count=live_bytes=peak_bytes=0;memset(allocations,0,sizeof(allocations));memset(prior_paths,0,sizeof(prior_paths));memset(prior_search,0,sizeof(prior_search));memset(sequence_records,0,sizeof(sequence_records));memset(&source,padding,sizeof(source));memset(&frame,padding,sizeof(frame));memset(&decoded,padding,sizeof(decoded));logical_source=source.values+64;memcpy(source.values+64,packet+source_offset,source_size);
  memset(&p,0,sizeof(p));p.cParams.windowLog=window_log;p.cParams.hashLog=hash_log;p.cParams.chainLog=chain_log;p.cParams.searchLog=search_log;p.cParams.minMatch=minimum;p.cParams.targetLength=target;p.cParams.strategy=ZSTD_btopt;p.fParams.contentSizeFlag=1;checked(ZSTD_checkCParams(p.cParams),"valid complete parameters");checked(ZSTD_CCtxParams_init_advanced(&cp,p),"advanced parameters");
  checked(ZSTD_CCtxParams_setParameter(&cp,ZSTD_c_nbWorkers,0),"single worker");checked(ZSTD_CCtxParams_setParameter(&cp,ZSTD_c_enableLongDistanceMatching,ZSTD_ps_disable),"disabled LDM");checked(ZSTD_CCtxParams_setParameter(&cp,ZSTD_c_splitAfterSequences,ZSTD_ps_disable),"disabled post splitting");checked(ZSTD_CCtxParams_setParameter(&cp,ZSTD_c_literalCompressionMode,literal_mode?ZSTD_ps_disable:ZSTD_ps_auto),"literal mode");
  context=ZSTD_createCCtx_advanced(memory);need(context!=NULL,"fresh CCtx");
  checked(ZSTD_compressBegin_advanced_internal(context,NULL,0,ZSTD_dct_auto,ZSTD_dtlm_fast,NULL,&cp,source_size),"ordinary Begin");check_owners();
  need(context->blockSizeMax==source_size&&context->isFirstBlock,"bounded known-size initial block limit");owners[0]=context->blockState.prevCBlock;owners[1]=context->blockState.nextCBlock;
  for(block_index=0;block_index<block_count;block_index++){
   U32 prior=owner_index(context->blockState.prevCBlock,owners),pending=1-prior,before_repeats[3],kind,word,field;size_t offset=0,payload;
   need(context->blockState.nextCBlock==owners[pending],"complementary pending owner");memcpy(before_repeats,owners[prior]->rep,sizeof(before_repeats));
   printf("{\"record\":\"block_begin\",\"padding_pass\":%u,\"block\":%u,\"start\":%zu,\"length\":%u,\"previous_owner\":%u,\"owners\":",pass,block_index,consumed,block_lengths[block_index],prior);owner_repeats(owners);printf(",\"frontier\":%u}\n",state.nextToUpdate);
   event_count=sequence_count=block_tail=0;reverse_phase=before_live=0;memset(sequence_records,0,sizeof(sequence_records));
   result=checked(ZSTD_compressContinue(context,frame.values+written,sizeof(frame.values)-written,logical_source+consumed,block_lengths[block_index]),"ordinary Continue");need(result<=sizeof(frame.values)-written&&event_count>0&&!before_live,"complete block parse");guards();
   if(block_index==0){ZSTD_frameHeader h;need(checked(ZSTD_getFrameHeader(&h,frame.values,result),"frame header")==0&&h.frameContentSize==source_size&&h.dictID==0&&h.checksumFlag==0,"exact complete frame header");offset=h.headerSize;}
   need(result>=offset+3,"complete block header");word=(U32)frame.values[written+offset]|((U32)frame.values[written+offset+1]<<8)|((U32)frame.values[written+offset+2]<<16);kind=(word>>1)&3;field=word>>3;need((word&1)==0&&kind<3&&field<=block_lengths[block_index],"ordinary nonfinal block");payload=kind==1?1:field;need(result==offset+3+payload,"exact block output extent");
   printf("{\"record\":\"block_publish\",\"padding_pass\":%u,\"block\":%u,\"kind\":%u,\"previous_owner_before\":%u,\"previous_owner_after\":%u,\"before_repeats\":",pass,block_index,kind,prior,owner_index(context->blockState.prevCBlock,owners));array(before_repeats,3);fputs(",\"candidate_repeats\":",stdout);array(owners[pending]->rep,3);fputs(",\"owners\":",stdout);owner_repeats(owners);printf(",\"sequences\":%u,\"tail\":%u}\n",sequence_count,block_tail);
   written+=result;consumed+=block_lengths[block_index];total_events+=event_count;
  }
  need(consumed==source_size,"complete consumed source");result=checked(ZSTD_compressEnd(context,frame.values+written,sizeof(frame.values)-written,logical_source+source_size,0),"ordinary End");need(result==3&&frame.values[written]==1&&frame.values[written+1]==0&&frame.values[written+2]==0,"empty final raw block");written+=result;guards();
  result=checked(ZSTD_decompress(decoded.values,sizeof(decoded.values),frame.values,written),"oracle frame decode");need(result==source_size&&memcmp(decoded.values,logical_source,source_size)==0,"oracle decoded source");guards();artifact(argv[2+pass],frame.values,written);checked(ZSTD_freeCCtx(context),"CCtx release");context=NULL;ledger(argv[4+pass]);
 }
 printf("{\"record\":\"complete\",\"events\":%u,\"passes\":2,\"blocks\":%u,\"guards_unchanged\":true,\"source_unchanged\":true}\n",total_events,block_count);need(fflush(stdout)==0&&!ferror(stdout),"complete stdout");return 0;
}
