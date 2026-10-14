/* Bounded observation hooks around pinned zstd_opt.c path selection.
 * facebook/zstd v1.5.7 f8745da6ff1ad1e7bab384bd1f9d742439278e99; LICENSE.zstd.
 * Match discovery alone is scripted. Generated source preserves parser logic.
 */
#define _POSIX_C_SOURCE 200809L
#define ZSTD_STATIC_LINKING_ONLY
#include "compress/zstd_compress_internal.h"
#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#define NONE UINT32_MAX
static U32 observer_matches(ZSTD_match_t*,ZSTD_MatchState_t*,U32*,const BYTE*,const BYTE*,const U32*,U32,U32);
static void observer_event(const char*,const optState_t*,const BYTE*,const BYTE*,U32,U32,const ZSTD_optimal_t*,U32,U32,U32,const U32*,const ZSTD_match_t*,U32,U32);
static void observer_query(const optState_t*,const BYTE*,const BYTE*,U32,U32,const BYTE*,const U32*,U32,U32,const ZSTD_match_t*,U32);
static void observer_sequence(U32,const BYTE*,U32,U32);
#define PATH_OBSERVE(stage,c,l,stretch,rs,re,sp) observer_event(stage,optStatePtr,ip,anchor,c,l,stretch,rs,re,sp,NULL,NULL,0,NONE)
#define PATH_QUERY(c,l,qip,qrep,ll0,count) observer_query(optStatePtr,ip,anchor,c,l,qip,qrep,ll0,minMatch,matches,count)
#include "observed_zstd_opt.c"
#define SOURCE_CAP 512u
#define SCRIPT_CAP 64u
#define CANDIDATE_CAP 8u
#define PACKET_CAP 5148u
#define EVENT_CAP 1024u
#define SEQUENCE_CAP 171u
#define SENTINEL UINT32_C(0xa59bd763)
#define TABLE(name,type,count) static struct { U32 before[16]; type values[count]; U32 after[16]; } name
TABLE(literals,U32,256);
TABLE(lengths,U32,36);
TABLE(match_lengths,U32,53);
TABLE(offsets,U32,32);
TABLE(path,ZSTD_optimal_t,ZSTD_OPT_SIZE);
TABLE(candidates,ZSTD_match_t,ZSTD_OPT_SIZE);
TABLE(sequences,SeqDef,SEQUENCE_CAP);
static struct { unsigned char before[64], values[SOURCE_CAP+128], after[64]; } source;
static struct { unsigned char before[64], values[SOURCE_CAP+64], after[64]; } literal_output;
static struct { U32 position,count; ZSTD_match_t matches[CANDIDATE_CAP]; } scripts[SCRIPT_CAP];
static U32 sequence_records[SEQUENCE_CAP][4];
static unsigned char packet[PACKET_CAP];
static char output_buffer[65536];
static U32 source_size,script_count,minimum,target,literal_mode,event_count,pass,sequence_count,global_repeats[3];
static unsigned char padding;
static int reverse_phase;
static ZSTD_MatchState_t state;
static ZSTD_entropyCTables_t costs;
static SeqStore_t store;
static const BYTE *logical_source;
_Static_assert(sizeof(U32)==4,"32-bit values");
_Static_assert(sizeof(source)+sizeof(literal_output)+sizeof(scripts)+sizeof(sequence_records)+sizeof(packet)+sizeof(output_buffer)+sizeof(state)+sizeof(costs)+sizeof(store)+sizeof(literals)+sizeof(lengths)+sizeof(match_lengths)+sizeof(offsets)+sizeof(path)+sizeof(candidates)+sizeof(sequences)+65536u<(4u<<20),"bounded observer storage");
static void need(int ok,const char*why){if(!ok){fprintf(stderr,"optimal path oracle: %s\n",why);exit(1);}}
static U32 little32(const BYTE*p){return(U32)p[0]|((U32)p[1]<<8)|((U32)p[2]<<16)|((U32)p[3]<<24);}
static void guard(U32*b,U32*a,int init){U32 i;for(i=0;i<16;i++){if(init)b[i]=a[i]=SENTINEL;else need(b[i]==SENTINEL&&a[i]==SENTINEL,"owner guard");}}
#define GUARD(name,init) guard(name.before,name.after,init)
static void guards(int init){
 U32 i;GUARD(literals,init);GUARD(lengths,init);GUARD(match_lengths,init);GUARD(offsets,init);GUARD(path,init);GUARD(candidates,init);GUARD(sequences,init);
 if(init)return;
 for(i=0;i<64;i++)need(source.before[i]==padding&&source.after[i]==padding&&literal_output.before[i]==padding&&literal_output.after[i]==padding,"byte guards");
 for(i=0;i<64;i++)need(source.values[i]==padding,"prefix guard");
 for(i=64+source_size;i<SOURCE_CAP+128;i++)need(source.values[i]==padding,"source tail guard");
 need(memcmp(logical_source,packet+28,source_size)==0,"source unchanged");
 for(i=source_size+3;i<ZSTD_OPT_SIZE;i++){const ZSTD_optimal_t*c=path.values+i;need(c->price==0&&c->off==0&&c->mlen==0&&c->litlen==0&&c->rep[0]==0&&c->rep[1]==0&&c->rep[2]==0,"path tail zero");}
 need(store.sequences==sequences.values+sequence_count,"sequence count publication");
}
static void array(const U32*v,U32 n){U32 i;putchar('[');for(i=0;i<n;i++){if(i)putchar(',');printf("%u",v[i]);}putchar(']');}
static void cell(const ZSTD_optimal_t*c){printf("[%d,%u,%u,%u,%u,%u,%u]",c->price,c->off,c->mlen,c->litlen,c->rep[0],c->rep[1],c->rep[2]);}
static void optional(U32 v){if(v==NONE)fputs("null",stdout);else printf("%u",v);}
static U32 position(const BYTE*p){need(p>=logical_source&&p<=logical_source+source_size,"source position");return(U32)(p-logical_source);}
static U32 resolve(U32 off,const U32*rep,U32 ll0){
 need(off>0&&ll0<=1,"intent/ll0");if(off>3)return off-3;
 if(!ll0)return rep[off-1];if(off==1)return rep[1];if(off==2)return rep[2];return rep[0]-1;
}
static void valid_match(U32 pos,U32 off,U32 length,const U32*rep,U32 ll0){
 U32 d=resolve(off,rep,ll0);need(d>0&&d<=pos&&length<=source_size-pos,"available match");need(memcmp(logical_source+pos,logical_source+pos-d,length)==0,"match bytes");
}
static U32 observer_matches(ZSTD_match_t*out,ZSTD_MatchState_t*ms,U32*next3,const BYTE*ip,const BYTE*end,const U32*rep,U32 ll0,U32 beat){
 U32 p=position(ip),i,j;need(ms==&state&&next3!=NULL&&end==logical_source+source_size,"callback owners");
 need(p>=1&&p<=source_size-8&&ll0<=1&&beat==(minimum==3?3u:4u),"callback bounds");
 for(i=0;i<script_count;i++)if(scripts[i].position==p){
  for(j=0;j<scripts[i].count;j++){ZSTD_match_t m=scripts[i].matches[j];valid_match(p,m.off,m.len,rep,ll0);out[j]=m;}return scripts[i].count;
 }return 0;
}
static void observer_query(const optState_t*opt,const BYTE*ip,const BYTE*anchor,U32 cur,U32 last,const BYTE*qip,const U32*rep,U32 ll0,U32 beat,const ZSTD_match_t*matches,U32 count){
 U32 q[6]={position(qip),rep[0],rep[1],rep[2],ll0,beat};observer_event("query",opt,ip,anchor,cur,last,NULL,NONE,NONE,NONE,q,matches,count,NONE);
}
static void observer_sequence(U32 llen,const BYTE*anchor,U32 off,U32 length){
 U32 index=sequence_count;SeqDef actual;size_t consumed=0;U32 i;
 need(index<SEQUENCE_CAP&&store.sequences==sequences.values+index+1,"bounded stored sequence");actual=sequences.values[index];
 need(actual.litLength==llen&&actual.mlBase+3u==length&&actual.offBase==off,"exact upstream stored sequence");
 for(i=0;i<index;i++)consumed+=sequence_records[i][1];
 need(llen<=source_size&&consumed+llen<=SOURCE_CAP&&memcmp(literal_output.values+consumed,anchor,llen)==0,"exact copied literals");
 sequence_records[index][0]=position(anchor);sequence_records[index][1]=llen;sequence_records[index][2]=length;sequence_records[index][3]=off;sequence_count++;
}
static void observer_event(const char*stage,const optState_t*opt,const BYTE*ip,const BYTE*anchor,U32 cur,U32 last,const ZSTD_optimal_t*stretch,U32 rs,U32 re,U32 sp,const U32*q,const ZSTD_match_t*matches,U32 count,U32 tail){
 U32 i;need(opt==&state.opt&&event_count<EVENT_CAP&&count<=CANDIDATE_CAP,"event bounds/owner");guards(0);
 if(strcmp(stage,"reverse_start")==0)reverse_phase=1;
 printf("{\"record\":\"event\",\"stage\":\"%s\",\"phase\":\"%s\",\"padding_pass\":%u,\"index\":%u,\"ip\":%u,\"anchor\":%u,\"cur\":",stage,reverse_phase?"reverse":"forward",pass,event_count++,position(ip),position(anchor));optional(cur);
 fputs(",\"last\":",stdout);optional(last);fputs(",\"stretch\":",stdout);if(stretch)cell(stretch);else fputs("null",stdout);
 fputs(",\"reverse\":",stdout);if(rs==NONE)fputs("null",stdout);else printf("[%u,%u,%u]",rs,re,sp);
 fputs(",\"query\":",stdout);if(q)array(q,6);else fputs("null",stdout);
 fputs(",\"matches\":",stdout);if(q){putchar('[');for(i=0;i<count;i++){if(i)putchar(',');printf("[%u,%u]",matches[i].off,matches[i].len);}putchar(']');}else fputs("null",stdout);
 fputs(",\"tail\":",stdout);optional(tail);fputs(",\"repeats\":",stdout);array(global_repeats,3);
 fputs(",\"cells\":[",stdout);for(i=0;i<source_size+3;i++){if(i)putchar(',');cell(path.values+i);}fputs("],\"frequencies\":{\"lit\":",stdout);array(opt->litFreq,256);fputs(",\"ll\":",stdout);array(opt->litLengthFreq,36);fputs(",\"ml\":",stdout);array(opt->matchLengthFreq,53);fputs(",\"of\":",stdout);array(opt->offCodeFreq,32);
 printf("},\"sums\":{\"lit\":%u,\"ll\":%u,\"ml\":%u,\"of\":%u},\"base_prices\":{\"lit\":%u,\"ll\":%u,\"ml\":%u,\"of\":%u},\"predefined\":%s,\"sequences\":[",opt->litSum,opt->litLengthSum,opt->matchLengthSum,opt->offCodeSum,opt->litSumBasePrice,opt->litLengthSumBasePrice,opt->matchLengthSumBasePrice,opt->offCodeSumBasePrice,opt->priceType==zop_predef?"true":"false");
 for(i=0;i<sequence_count;i++){if(i)putchar(',');array(sequence_records[i],4);}
 puts("],\"source_unchanged\":true,\"guards_unchanged\":true,\"path_tail_zero\":true}");
 if(strcmp(stage,"refresh")==0)reverse_phase=0;
}
int main(int argc,char**argv){
 struct stat info;FILE*input;size_t size,offset;U32 i,j,previous=0,total_events=0;U64 fingerprint=UINT64_C(0xcbf29ce484222325);
 need(argc==2,"one packet path");need(stat(argv[1],&info)==0&&S_ISREG(info.st_mode),"regular packet");need(info.st_size>=37&&info.st_size<=PACKET_CAP,"packet size");size=(size_t)info.st_size;
 input=fopen(argv[1],"rb");need(input!=NULL,"packet open");need(fread(packet,1,size,input)==size&&fgetc(input)==EOF&&!ferror(input),"complete packet");need(fclose(input)==0,"packet close");need(memcmp(packet,"ZOPTH01\0",8)==0,"packet magic");
 source_size=little32(packet+8);script_count=little32(packet+12);minimum=little32(packet+16);target=little32(packet+20);literal_mode=little32(packet+24);
 need(source_size>=9&&source_size<=SOURCE_CAP&&script_count<=SCRIPT_CAP&&minimum>=3&&minimum<=7&&target<=4095&&literal_mode<=1,"request bounds");offset=28u+source_size;need(offset<=size,"source packet range");
 for(i=0;i<script_count;i++){
  U32 prior=(minimum==3?3u:4u)-1;need(offset+8<=size,"script header");scripts[i].position=little32(packet+offset);scripts[i].count=little32(packet+offset+4);offset+=8;
  need(scripts[i].position>previous&&scripts[i].position<=source_size-8&&scripts[i].count>=1&&scripts[i].count<=CANDIDATE_CAP,"script bounds/order");previous=scripts[i].position;need(offset+8u*scripts[i].count<=size,"candidate packet range");
  for(j=0;j<scripts[i].count;j++){
   U32 off=little32(packet+offset),len=little32(packet+offset+4);offset+=8;need(off>0&&off<=previous+3&&len>prior&&len<=source_size-previous,"candidate intent/extent");prior=len;scripts[i].matches[j].off=off;scripts[i].matches[j].len=len;
   if(off>3)need(memcmp(packet+28+previous,packet+28+previous-(off-3),len)==0,"direct source bytes");
  }
 }
 need(offset==size,"exact packet extent");for(i=0;i<source_size;i++)fingerprint=(fingerprint^packet[28+i])*UINT64_C(0x100000001b3);
 need(setvbuf(stdout,output_buffer,_IOFBF,sizeof(output_buffer))==0,"stdout buffer");
 printf("{\"record\":\"metadata\",\"protocol\":\"ZOPTH01\",\"revision\":\"f8745da6ff1ad1e7bab384bd1f9d742439278e99\",\"source_bytes\":%u,\"source_hash\":\"%016" PRIx64 "\",\"script_positions\":%u,\"parameters\":[%u,%u,%u],\"path_cells\":%u,\"passes\":2}\n",source_size,(uint64_t)fingerprint,script_count,minimum,target,literal_mode,source_size+3);
 for(pass=0;pass<2;pass++){
  size_t tail;padding=pass?0xc3:0x6a;event_count=sequence_count=0;reverse_phase=0;memset(&state,0,sizeof(state));memset(&store,0,sizeof(store));memset(&costs,0,sizeof(costs));memset(&source,padding,sizeof(source));memset(&literal_output,padding,sizeof(literal_output));logical_source=source.values+64;memcpy(source.values+64,packet+28,source_size);
  memset(literals.values,0,sizeof(literals.values));memset(lengths.values,0,sizeof(lengths.values));memset(match_lengths.values,0,sizeof(match_lengths.values));memset(offsets.values,0,sizeof(offsets.values));memset(path.values,0,sizeof(path.values));memset(candidates.values,0,sizeof(candidates.values));memset(sequences.values,0,sizeof(sequences.values));memset(sequence_records,0,sizeof(sequence_records));guards(1);
  state.window.base=source.values+62;state.window.dictBase=source.values+62;state.window.dictLimit=2;state.window.lowLimit=2;state.window.nextSrc=logical_source+source_size;state.nextToUpdate=2;
  state.cParams.windowLog=10;state.cParams.hashLog=6;state.cParams.chainLog=6;state.cParams.searchLog=1;state.cParams.minMatch=minimum;state.cParams.targetLength=target;state.cParams.strategy=ZSTD_btopt;
  state.opt.litFreq=literals.values;state.opt.litLengthFreq=lengths.values;state.opt.matchLengthFreq=match_lengths.values;state.opt.offCodeFreq=offsets.values;state.opt.priceTable=path.values;state.opt.matchTable=candidates.values;state.opt.symbolCosts=&costs;state.opt.literalCompressionMode=literal_mode?ZSTD_ps_disable:ZSTD_ps_auto;
  store.sequencesStart=store.sequences=sequences.values;store.litStart=store.lit=literal_output.values;store.maxNbSeq=SEQUENCE_CAP;store.maxNbLit=SOURCE_CAP;
  global_repeats[0]=1;global_repeats[1]=4;global_repeats[2]=8;guards(0);
  tail=ZSTD_compressBlock_opt_generic(&state,&store,global_repeats,logical_source,source_size,0,ZSTD_noDict);need(tail<=source_size,"trailing literals");guards(0);total_events+=event_count;
 }
 printf("{\"record\":\"complete\",\"events\":%u,\"passes\":2,\"sequences\":%u,\"guards_unchanged\":true,\"source_unchanged\":true}\n",total_events,sequence_count);need(fflush(stdout)==0&&!ferror(stdout),"complete output");return 0;
}
