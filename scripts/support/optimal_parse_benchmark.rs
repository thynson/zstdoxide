//! Diagnostic scripted path workload; exact C-derived event/final-state checksum.
#![feature(allocator_api)]
#![allow(dead_code)]
extern crate alloc;
#[path = "@ROOT@/src/error.rs"] mod error;
#[path = "@ROOT@/src/constants.rs"] mod constants;
#[path = "@ROOT@/src/greedy_key.rs"] mod greedy_key;
#[path = "@ROOT@/src/tree_layout.rs"] mod tree_layout;
#[path = "@ROOT@/src/optimal_search_layout.rs"] mod optimal_search_layout;
#[path = "@ROOT@/src/optimal_price.rs"] mod optimal_price;
#[path = "@ROOT@/src/optimal_frequencies.rs"] mod optimal_frequencies;
#[path = "@ROOT@/src/optimal_matches.rs"] mod optimal_matches;
#[path = "@ROOT@/src/optimal_path_layout.rs"] mod optimal_path_layout;
#[path = "@ROOT@/src/optimal_paths.rs"] mod optimal_paths;
#[path = "@ROOT@/src/optimal_parse.rs"] mod optimal_parse;
#[path = "@ROOT@/tests/support/optimal_path_fixture.rs"] mod fixture;
pub use error::{Error,Result};
use core::alloc::Allocator;
use optimal_parse::{PathEvent,PathObserver,PathProvider,OptimalParser};
use optimal_path_layout::{PathCell,PATH_CAPACITY};
use optimal_search_layout::{OptimalMatch,OptimalQuery};
use std::time::Instant;
use std::hint::black_box;
use zstdoxide_test_allocator::{AllocatorConfig,TrackingAllocator};
const WORKLOADS:[&str;5]=["path_all_literals","path_repeat_publication","path_script_seed_1","path_wide_later_4096","path_wide_literal_pricing"];
fn mix(hash:&mut u64,value:u64){for byte in value.to_le_bytes(){*hash=(*hash^u64::from(byte)).wrapping_mul(0x100000001b3);}}
fn event_hash(hash:&mut u64,event:PathEvent,matches:&[OptimalMatch]) {
    for value in [u64::from(event.stage),u64::from(event.reverse_phase),event.ip as u64,event.anchor as u64,event.cur.map_or(u64::MAX,|v|v as u64),event.last.map_or(u64::MAX,|v|v as u64),event.tail.map_or(u64::MAX,|v|v as u64),event.sequences as u64] {mix(hash,value);}
    for value in event.repeats{mix(hash,u64::from(value));}
    mix(hash,u64::from(event.stretch.is_some()));if let Some(cell)=event.stretch{for value in cell.words(){mix(hash,u64::from(value));}}
    mix(hash,u64::from(event.reverse.is_some()));if let Some(row)=event.reverse{for value in row{mix(hash,value as u64);}}
    mix(hash,u64::from(event.query.is_some()));if let Some(q)=event.query{for value in [q.position as u64,q.repeats[0]as u64,q.repeats[1]as u64,q.repeats[2]as u64,u64::from(q.zero_literals),q.length_to_beat as u64]{mix(hash,value);}}
    mix(hash,matches.len()as u64);for m in matches{mix(hash,u64::from(m.off_base()));mix(hash,m.length()as u64);}
    mix(hash,u64::from(event.appended.is_some()));if let Some(row)=event.appended{for value in row{mix(hash,u64::from(value));}}
}
fn final_hash(hash:&mut u64,cells:&[PathCell],frequencies:&[u32;386]){for cell in cells{for value in cell.words(){mix(hash,u64::from(value));}}for &value in frequencies{mix(hash,u64::from(value));}}
struct Script<'a>(&'a[fixture::Script]);
impl PathProvider for Script<'_>{fn query<A:Allocator>(&mut self,q:OptimalQuery,m:&mut optimal_matches::OptimalMatches<A>)->Result<()>{if let Some(s)=self.0.iter().find(|s|s.position as usize==q.position){for &[off,len]in &s.matches{m.push(OptimalMatch::new(off,len as usize)?)?;}}Ok(())}}
struct Observe{hash:u64,events:usize,cells:Vec<PathCell>}
impl PathObserver for Observe{
 fn event<P:Allocator,F:Allocator>(&mut self,e:PathEvent,p:&optimal_paths::OptimalPaths<P>,f:&optimal_frequencies::OptimalFrequencies<F>,m:&[OptimalMatch])->Result<()>{
  event_hash(&mut self.hash,e,m);self.events+=1;
  if e.stage==11{p.snapshot(&mut self.cells)?;let s=f.snapshot()?;let mut words=[0;386];words[..377].copy_from_slice(&s.counts);words[377..381].copy_from_slice(&s.sums);words[381..385].copy_from_slice(&s.base_prices);words[385]=u32::from(s.predefined);final_hash(&mut self.hash,&self.cells,&words);}Ok(())
 }
}
fn expected(case:&fixture::Case<'_>)->Result<u64>{
 let mut hash=0xcbf29ce484222325;let mut cells=vec![PathCell::ZERO;PATH_CAPACITY];let mut frequencies=[0;386];
 for e in &case.events{
  let event=PathEvent{stage:e.stage as u8,reverse_phase:e.reverse_phase,ip:e.ip as usize,anchor:e.anchor as usize,cur:e.cur.map(|v|v as usize),last:e.last.map(|v|v as usize),tail:e.tail.map(|v|v as usize),repeats:e.repeats,stretch:e.stretch.map(PathCell::from_words).transpose()?,reverse:e.reverse.map(|r|r.map(|v|v as usize)),query:e.query.map(|q|OptimalQuery{position:q[0]as usize,repeats:[q[1],q[2],q[3]],zero_literals:q[4]==1,length_to_beat:q[5]as usize}),appended:e.appended.first().copied(),sequences:e.sequence_count};
  let matches=e.matches.iter().map(|&[o,l]|OptimalMatch::new(o,l as usize)).collect::<Result<Vec<_>>>()?;event_hash(&mut hash,event,&matches);
  for &(i,c)in &e.path{cells[i]=PathCell::from_words(c)?;}for &(i,v)in &e.frequencies{frequencies[i]=v;}
 }
 final_hash(&mut hash,&cells,&frequencies);Ok(hash)
}
fn main()->Result<()>{
 let verify=std::env::args().nth(1).as_deref()==Some("--verify");let cases=fixture::read(fixture::PACK).unwrap();
 for name in WORKLOADS{
  let case=cases.iter().find(|c|c.name==name).unwrap();let golden=expected(case)?;
  let allocator=TrackingAllocator::new(AllocatorConfig::default()).unwrap().into_non_clone();let monitor=allocator.monitor();
  let frequencies=optimal_frequencies::OptimalFrequencies::new_in(case.literal_mode==1,&allocator)?;
  let paths=optimal_paths::OptimalPaths::new_in(&allocator)?;let matches=optimal_matches::OptimalMatches::new_in(&allocator)?;
  let mut parser=OptimalParser::from_parts(case.source,case.minimum as u8,case.target as usize,frequencies,paths,matches)?;
  let mut observer=Observe{hash:0,events:0,cells:vec![PathCell::ZERO;PATH_CAPACITY]};
  let mut run=||->Result<()>{parser.reset()?;observer.hash=0xcbf29ce484222325;observer.events=0;let result=parser.parse(&mut Script(&case.scripts),&mut observer)?;assert_eq!(black_box(observer.hash),golden);assert_eq!(observer.events,case.events.len());assert_eq!(result.sequences,case.events.last().unwrap().sequence_count);Ok(())};
  run()?;let mut samples=Vec::new();if !verify{for _ in 0..5{let start=Instant::now();for _ in 0..32{run()?;}samples.push(start.elapsed().as_nanos());}}
  let stats=monitor.snapshot();let bytes=377*core::mem::size_of::<u32>()+PATH_CAPACITY*core::mem::size_of::<PathCell>()+optimal_matches::MATCH_CAPACITY*core::mem::size_of::<OptimalMatch>();
  assert_eq!(stats.attempts,3);assert_eq!(stats.live_requested_bytes,bytes as u128);
  println!("{{\"case\":\"{name}\",\"checksum\":\"{golden:016x}\",\"events\":{},\"allocations\":{},\"owner_bytes\":{},\"rounds\":{},\"samples_ns\":{:?}}}",case.events.len(),stats.attempts,stats.live_requested_bytes,if verify{0}else{32},samples);
  drop(parser);monitor.assert_released();assert_eq!(monitor.snapshot().owners_dropped,0);drop(allocator);assert_eq!(monitor.snapshot().owners_dropped,1);
 }
 Ok(())
}
