/* Bounded observations of unchanged facebook/zstd v1.5.7 Greedy/Lazy/Lazy2.
 * Reference f8745da6ff1ad1e7bab384bd1f9d742439278e99; setup and replay only.
 * Arena/protocol contract: docs/reference-lazy-oracle.md.
 */
#define _POSIX_C_SOURCE 200809L
#define ZSTD_STATIC_LINKING_ONLY
#include "compress/zstd_lazy.h"
#include "zstd.h"
#include <inttypes.h>
#include <limits.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>

#define GUARD 64u
#define LEADING 64u
#define MAX_PREFIX (2u * 1024u * 1024u)
#define MAX_BLOCK (128u * 1024u)
#define SUFFIX 2048u
#define MAX_SEEDS 524288u
#define MAX_PACKET (152u + 4u * MAX_SEEDS + MAX_PREFIX + MAX_BLOCK)
#define MAX_ALLOCATION (64u * 1024u * 1024u)

typedef struct {
  unsigned char *allocation, *data;
  size_t bytes, cost, leading;
} Guarded;
typedef struct {
  char name[64];
  U32 branch, depth, prefix, block, window_log, hash_log, chain_log, search_log;
  U32 min_match, target_length, floor, next, repeats[3], seed_count, entropy;
  U64 salt;
  Guarded seeds, source;
} Recipe;
typedef struct {
  Guarded storage, before_storage;
  size_t count;
  unsigned log, element_bytes;
} Table;
typedef struct {
  size_t ll, ml;
  U32 off_base, distance;
} Observation;
static const char *active_case = "setup";
static size_t live_allocation_bytes;

static void require(int valid, const char *reason) {
  if (!valid) {
    fprintf(stderr, "lazy oracle [%s]: %s\n", active_case, reason);
    exit(1);
  }
}
static size_t add(size_t a, size_t b) {
  require(b <= SIZE_MAX - a, "size addition overflow");
  return a + b;
}
static size_t multiply(size_t a, size_t b) {
  require(a == 0 || b <= SIZE_MAX / a, "size multiplication overflow");
  return a * b;
}
static size_t guarded_cost(size_t bytes) {
  return add(bytes ? bytes : 1, 2 * GUARD + 63);
}
static Guarded guarded_new(size_t bytes) {
  Guarded result;
  size_t physical = bytes ? bytes : 1;
  size_t cost = guarded_cost(bytes), total = add(live_allocation_bytes, cost);
  uintptr_t address;
  size_t adjustment;
  require(total <= MAX_ALLOCATION, "aggregate allocation cap");
  result.allocation = (unsigned char *)malloc(cost);
  require(result.allocation != NULL, "allocation failed");
  address = (uintptr_t)(result.allocation + GUARD);
  adjustment = (64 - (address & 63)) & 63;
  result.leading = add(GUARD, adjustment);
  require(result.leading <= cost && physical <= cost - result.leading &&
              GUARD <= cost - result.leading - physical,
          "aligned payload extent");
  result.data = result.allocation + result.leading;
  result.bytes = bytes;
  result.cost = cost;
  require(((uintptr_t)result.data & 63) == 0, "payload alignment");
  memset(result.allocation, 0xd7, result.leading);
  memset(result.data, 0, physical);
  memset(result.data + physical, 0x9b, cost - result.leading - physical);
  live_allocation_bytes = total;
  return result;
}
static void guarded_check(const Guarded *g) {
  size_t i, physical = g->bytes ? g->bytes : 1;
  for (i = 0; i < g->leading; ++i)
    require(g->allocation[i] == 0xd7, "leading padding/guard changed");
  for (i = add(g->leading, physical); i < g->cost; ++i)
    require(g->allocation[i] == 0x9b, "trailing padding/guard changed");
}
static void guarded_drop(Guarded *g) {
  guarded_check(g);
  require(g->cost <= live_allocation_bytes, "allocation accounting underflow");
  free(g->allocation);
  live_allocation_bytes -= g->cost;
  memset(g, 0, sizeof(*g));
}
static uint64_t fingerprint(const unsigned char *bytes, size_t count) {
  uint64_t h = UINT64_C(0xcbf29ce484222325);
  size_t i;
  for (i = 0; i < count; ++i) {
    h ^= bytes[i];
    h *= UINT64_C(0x100000001b3);
  }
  return h;
}
static void print_hex(const unsigned char *bytes, size_t count) {
  size_t i;
  putchar('"');
  for (i = 0; i < count; ++i)
    printf("%02x", bytes[i]);
  putchar('"');
}
static void print_repeats(const U32 r[3]) {
  printf("[%" PRIu32 ",%" PRIu32 ",%" PRIu32 "]", r[0], r[1], r[2]);
}
static void print_seeds(const U32 *seeds, U32 count) {
  U32 i;
  putchar('[');
  for (i = 0; i < count; ++i) {
    if (i)
      putchar(',');
    printf("%" PRIu32, seeds[i]);
  }
  putchar(']');
}
/* Independent full wire-repeat update. Expanded LL selects the zero-literal
 * rule; neither the collector nor ZSTD_updateRep supplies expected state. */
static U32 resolve_repeat(U32 r[3], U32 off_base, size_t ll) {
  U32 a = r[0], b = r[1], c = r[2], distance;
  require(off_base != 0 && a != 0 && b != 0 && c != 0,
          "invalid wire repeat state");
  if (off_base > 3) {
    distance = off_base - 3;
    r[2] = b;
    r[1] = a;
    r[0] = distance;
  } else if (ll != 0 && off_base == 1) {
    distance = a;
  } else if ((ll != 0 && off_base == 2) || (ll == 0 && off_base == 1)) {
    distance = b;
    r[0] = b;
    r[1] = a;
  } else {
    if (ll == 0 && off_base == 3) {
      require(a > 1, "repeat-minus-one is zero");
      distance = a - 1;
    } else
      distance = c;
    r[2] = b;
    r[1] = a;
    r[0] = distance;
  }
  require(distance != 0, "zero match distance");
  return distance;
}
/* These are dispatch values, not an implementation of the search algorithm. */
_Static_assert(ZSTD_greedy == 3 && ZSTD_lazy == 4 && ZSTD_lazy2 == 5,
               "pinned strategy values");
static ZSTD_strategy strategy(unsigned depth) {
  require(depth <= 2, "lookahead depth");
  return (ZSTD_strategy)(ZSTD_greedy + depth);
}
static const char *entry_point(unsigned depth, unsigned branch) {
  require(depth <= 2 && branch <= 1, "entry point selector");
  switch (depth) {
  case 0:
    return branch ? "ZSTD_compressBlock_greedy_row"
                  : "ZSTD_compressBlock_greedy";
  case 1:
    return branch ? "ZSTD_compressBlock_lazy_row" : "ZSTD_compressBlock_lazy";
  default:
    return branch ? "ZSTD_compressBlock_lazy2_row" : "ZSTD_compressBlock_lazy2";
  }
}
static size_t parse(const Recipe *r, ZSTD_MatchState_t *state,
                    SeqStore_t *store, U32 repeats[3],
                    const unsigned char *source) {
  require(r->depth <= 2 && r->branch <= 1, "parser selector");
  /* Each unchanged upstream entry receives the already reviewed selected
   * owners and admitted source block. No speculative pointer is formed here. */
  switch (r->depth) {
  case 0:
    return r->branch ? ZSTD_compressBlock_greedy_row(state, store, repeats,
                                                     source, r->block)
                     : ZSTD_compressBlock_greedy(state, store, repeats, source,
                                                 r->block);
  case 1:
    return r->branch ? ZSTD_compressBlock_lazy_row(state, store, repeats,
                                                   source, r->block)
                     : ZSTD_compressBlock_lazy(state, store, repeats, source,
                                               r->block);
  default:
    return r->branch ? ZSTD_compressBlock_lazy2_row(state, store, repeats,
                                                    source, r->block)
                     : ZSTD_compressBlock_lazy2(state, store, repeats, source,
                                                r->block);
  }
}
static ZSTD_compressionParameters parameters(const Recipe *r) {
  ZSTD_compressionParameters p;
  memset(&p, 0, sizeof(p));
  p.windowLog = r->window_log;
  p.chainLog = r->chain_log;
  p.hashLog = r->hash_log;
  p.searchLog = r->search_log;
  p.minMatch = r->min_match;
  p.targetLength = r->target_length;
  p.strategy = strategy(r->depth);
  return p;
}
static unsigned width(const Recipe *r) {
  return r->min_match < 4 ? 4 : (r->min_match > 6 ? 6 : r->min_match);
}
static unsigned row_log(const Recipe *r) {
  return r->search_log < 4 ? 4 : (r->search_log > 6 ? 6 : r->search_log);
}
static void validate_scalars(const Recipe *r) {
  size_t n = add(r->prefix, r->block);
  unsigned i;
  require(ZSTD_WINDOW_START_INDEX == 2 && HASH_READ_SIZE == 8 &&
              MINMATCH == 3 && WILDCOPY_OVERLENGTH == 32 &&
              ZSTD_ROW_HASH_CACHE_SIZE == 8 && ZSTD_ROW_HASH_TAG_BITS == 8 &&
              sizeof(U32) == 4,
          "pinned constants");
  require(r->depth <= 2 && r->branch <= 1 && r->prefix <= MAX_PREFIX &&
              r->block >= 7 && r->block <= MAX_BLOCK && n <= UINT32_MAX - 2,
          "branch/source bounds");
  require(r->window_log >= 10 && r->window_log <= 21 &&
              r->window_log < sizeof(size_t) * CHAR_BIT,
          "window log");
  require(r->block <= ((size_t)1 << r->window_log), "block exceeds window");
  require(r->hash_log >= 6 && r->hash_log <= 19 &&
              r->hash_log < sizeof(size_t) * CHAR_BIT && r->chain_log >= 6 &&
              r->chain_log <= 18 && r->chain_log < sizeof(size_t) * CHAR_BIT,
          "table logs");
  require(r->search_log >= 1 && r->search_log <= 29 && r->min_match >= 3 &&
              r->min_match <= 7 && r->target_length <= 131072,
          "search/match/target fields");
  if (r->branch == 0)
    require(r->search_log <= 8 && r->salt == 0 && r->entropy == 0,
            "bounded unsalted chain");
  else
    require(r->hash_log >= row_log(r) && r->hash_log - row_log(r) + 8 <= 32,
            "salted hash width");
  require(r->floor <= r->next && r->next <= r->prefix &&
              r->seed_count <= MAX_SEEDS,
          "floor/update/seed count");
  for (i = 0; i < 3; ++i)
    require(r->repeats[i] != 0, "initial repeat must be positive");
  require(!ZSTD_isError(ZSTD_checkCParams(parameters(r))), "invalid C tuple");
  require(64 % _Alignof(U32) == 0 && 64 % _Alignof(SeqDef) == 0 &&
              64 % _Alignof(Observation) == 0,
          "typed payload alignment");
}
static Table table_new(unsigned log, unsigned element_bytes) {
  Table t;
  t.log = log;
  t.element_bytes = element_bytes;
  t.count = (size_t)1 << log;
  t.storage = guarded_new(multiply(t.count, element_bytes));
  t.before_storage = guarded_new(t.storage.bytes);
  return t;
}
static U32 cell(const unsigned char *data, size_t i, unsigned bytes) {
  return bytes == 1 ? data[i] : ((const U32 *)data)[i];
}
static void seed_tables(Table *first, Table *second, const Recipe *r,
                        const unsigned char *source) {
  const U32 *seeds = (const U32 *)r->seeds.data;
  U32 *positions = (U32 *)first->storage.data;
  U32 i;
  for (i = 0; i < r->seed_count; ++i) {
    U32 p = seeds[i];
    require(p < r->next && p < r->prefix && width(r) <= r->prefix - p &&
                (i == 0 || seeds[i - 1] < p),
            "strictly increasing complete prefix seeds");
    if (r->branch == 0) {
      size_t h = ZSTD_hashPtr(source + p, r->hash_log, width(r));
      U32 *links = (U32 *)second->storage.data;
      require(h < first->count, "chain seed hash");
      links[(p + 2) & (second->count - 1)] = positions[h];
      positions[h] = p + 2;
    } else {
      U32 hash = (U32)ZSTD_hashPtrSalted(
          source + p, r->hash_log - row_log(r) + 8, width(r), r->salt);
      size_t row = ((size_t)(hash >> 8)) << row_log(r);
      U32 mask = ((U32)1 << row_log(r)) - 1;
      U32 next;
      BYTE *tags = second->storage.data;
      require(row < first->count && mask < first->count - row,
              "seed row extent");
      next = ((U32)tags[row] - 1) & mask;
      if (next == 0)
        next = mask;
      tags[row] = (BYTE)next;
      tags[row + next] = (BYTE)hash;
      positions[row + next] = p + 2;
    }
  }
  memcpy(first->before_storage.data, first->storage.data, first->storage.bytes);
  memcpy(second->before_storage.data, second->storage.data,
         second->storage.bytes);
}
static void table_check(const Table *first, const Table *second,
                        const Recipe *r, const unsigned char *source,
                        size_t n) {
  size_t i;
  U32 mask = ((U32)1 << row_log(r)) - 1;
  guarded_check(&first->storage);
  guarded_check(&first->before_storage);
  guarded_check(&second->storage);
  guarded_check(&second->before_storage);
  for (i = 0; i < first->count; ++i) {
    U32 p = cell(first->storage.data, i, 4);
    if (r->branch && (i & mask) == 0) {
      require(p == 0 && second->storage.data[i] <= mask, "reserved row head");
      continue;
    }
    if (p != 0) {
      size_t h;
      require(p >= 2 && width(r) <= n && p - 2 <= n - width(r),
              "final position range");
      h = r->branch
              ? ZSTD_hashPtrSalted(source + p - 2, r->hash_log - row_log(r) + 8,
                                   width(r), r->salt)
              : ZSTD_hashPtr(source + p - 2, r->hash_log, width(r));
      if (r->branch)
        require(((h >> 8) << row_log(r)) == (i & ~(size_t)mask) &&
                    (h & 255) == second->storage.data[i],
                "final row/tag identity");
      else
        require(h == i, "final head bucket identity");
    } else if (r->branch) {
      require(second->storage.data[i] == 0, "empty position has a tag");
    }
  }
  if (r->branch == 0)
    for (i = 0; i < second->count; ++i) {
      U32 p = cell(second->storage.data, i, 4);
      require(p == 0 || (p >= 2 && width(r) <= n && p - 2 <= n - width(r)),
              "final predecessor range");
    }
}
static void print_table_hash(const unsigned char *data, const Table *t) {
  uint64_t h = UINT64_C(0xcbf29ce484222325);
  size_t i;
  for (i = 0; i < t->count; ++i) {
    unsigned j;
    U32 value = cell(data, i, t->element_bytes);
    for (j = 0; j < t->element_bytes; ++j) {
      h ^= (value >> (8 * j)) & 255;
      h *= UINT64_C(0x100000001b3);
    }
  }
  printf("\"%016" PRIx64 "\"", h);
}
static void print_table(const Table *t) {
  size_t i;
  int comma = 0;
  printf("{\"log\":%u,\"element_bytes\":%u,\"entries\":%zu,\"before\":[",
         t->log, t->element_bytes, t->count);
  for (i = 0; i < t->count; ++i) {
    U32 value = cell(t->before_storage.data, i, t->element_bytes);
    if (value) {
      if (comma)
        putchar(',');
      comma = 1;
      printf("[%zu,%" PRIu32 "]", i, value);
    }
  }
  fputs("],\"changes\":[", stdout);
  comma = 0;
  for (i = 0; i < t->count; ++i) {
    U32 before = cell(t->before_storage.data, i, t->element_bytes);
    U32 after = cell(t->storage.data, i, t->element_bytes);
    if (before != after) {
      if (comma)
        putchar(',');
      comma = 1;
      printf("[%zu,%" PRIu32 ",%" PRIu32 "]", i, before, after);
    }
  }
  fputs("],\"before_fnv1a64\":", stdout);
  print_table_hash(t->before_storage.data, t);
  fputs(",\"after_fnv1a64\":", stdout);
  print_table_hash(t->storage.data, t);
  putchar('}');
}
static void table_drop(Table *t) {
  guarded_drop(&t->before_storage);
  guarded_drop(&t->storage);
}
static void execute(const Recipe *r, unsigned padding) {
  size_t n, arena_bytes, q, total, trailing, count, literal_bytes, cursor,
      literal_cursor, i;
  size_t window, current, repeat_floor;
  Guarded arena, arena_copy, sequences, literals, codes, replay, observed;
  Table first, second;
  unsigned char *source, *base;
  Observation *observations;
  SeqStore_t store;
  ZSTD_MatchState_t state;
  U32 parser_repeats[3], wire_repeats[3];
  active_case = r->name;
  validate_scalars(r);
  n = add(r->prefix, r->block);
  window = (size_t)1 << r->window_log;
  q = add(r->block / MINMATCH, 1);
  arena_bytes = add(add(LEADING, n), SUFFIX);
  /* Preflight all simultaneously live heap owners, including input/guards. */
  total = add(guarded_cost(n), guarded_cost(multiply(r->seed_count, 4)));
  total = add(total, multiply(2, guarded_cost(arena_bytes)));
  total = add(total,
              multiply(2, guarded_cost(multiply((size_t)1 << r->hash_log, 4))));
  total =
      add(total,
          multiply(2, guarded_cost(multiply(
                          (size_t)1 << (r->branch ? r->hash_log : r->chain_log),
                          r->branch ? 1 : 4))));
  total = add(total, guarded_cost(multiply(q, sizeof(SeqDef))));
  total = add(total, guarded_cost(add(r->block, WILDCOPY_OVERLENGTH)));
  total = add(total, guarded_cost(multiply(q, 3)));
  total = add(total, guarded_cost(n));
  total = add(total, guarded_cost(multiply(q, sizeof(Observation))));
  require(total <= MAX_ALLOCATION, "preflight allocation sum");
  /* Reviewed in reference-lazy-oracle.md. Leading64 contains source-2,
   * iend-8/-16 and iend-32 even for B=7. The failed-probe step is <=513;
   * suffix2048 covers formed pointers as well as initialized copy slack.
   * Lookahead may reach (but not exceed) the limit: chain reads end at N,
   * and the row cache's current+8 eight-byte hash also ends at N.
   * No selected-key or four-byte repeat read requires later block bytes.
   */
  arena = guarded_new(arena_bytes);
  arena_copy = guarded_new(arena_bytes);
  memset(arena.data, (int)padding, arena.bytes);
  source = arena.data + LEADING;
  base = source - 2;
  memcpy(source, r->source.data, n);
  memcpy(arena_copy.data, arena.data, arena.bytes);
  first = table_new(r->hash_log, 4);
  second = table_new(r->branch ? r->hash_log : r->chain_log, r->branch ? 1 : 4);
  seed_tables(&first, &second, r, source);
  sequences = guarded_new(multiply(q, sizeof(SeqDef)));
  literals = guarded_new(add(r->block, WILDCOPY_OVERLENGTH));
  codes = guarded_new(multiply(q, 3));
  replay = guarded_new(n);
  observed = guarded_new(multiply(q, sizeof(Observation)));
  observations = (Observation *)observed.data;
  require(live_allocation_bytes == total,
          "owner accounting differs from preflight");
  memset(&state, 0, sizeof(state));
  memset(&store, 0, sizeof(store));
  state.cParams = parameters(r);
  state.window.base = base;
  state.window.dictBase = base;
  state.window.nextSrc = source + n;
  state.window.dictLimit = r->floor + 2;
  state.window.lowLimit = r->floor + 2;
  state.hashTable = (U32 *)first.storage.data;
  if (r->branch) {
    state.tagTable = second.storage.data;
    state.rowHashLog = r->hash_log - row_log(r);
    state.hashSalt = r->salt;
    state.hashSaltEntropy = r->entropy;
  } else {
    state.chainTable = (U32 *)second.storage.data;
  }
  state.nextToUpdate = r->next + 2;
  /* Optional/dictionary fields remain zero. Q=floor(B/3)+1 exceeds the
   * maximum number of >=4-byte matches, including immediate repeats. */
  store.sequencesStart = (SeqDef *)sequences.data;
  store.litStart = literals.data;
  store.llCode = codes.data;
  store.mlCode = codes.data + q;
  store.ofCode = codes.data + 2 * q;
  store.maxNbSeq = q;
  store.maxNbLit = r->block;
  ZSTD_resetSeqStore(&store);
  memcpy(parser_repeats, r->repeats, sizeof(parser_repeats));
  memcpy(wire_repeats, r->repeats, sizeof(wire_repeats));
  trailing = parse(r, &state, &store, parser_repeats, source + r->prefix);
  guarded_check(&arena);
  guarded_check(&arena_copy);
  guarded_check(&sequences);
  guarded_check(&literals);
  guarded_check(&codes);
  require(memcmp(arena.data, arena_copy.data, arena.bytes) == 0,
          "source or padding changed");
  /* Proven capacities keep returned pointers in their original arrays. */
  count = (size_t)(store.sequences - store.sequencesStart);
  literal_bytes = (size_t)(store.lit - store.litStart);
  require(count <= q && literal_bytes <= r->block && trailing <= r->block,
          "output cardinality");
  require(store.longLengthType >= ZSTD_llt_none &&
              store.longLengthType <= ZSTD_llt_matchLength,
          "long length kind");
  require(store.longLengthType == ZSTD_llt_none || store.longLengthPos < count,
          "long length position");
  memcpy(replay.data, source, r->prefix);
  cursor = r->prefix;
  literal_cursor = 0;
  for (i = 0; i < count; ++i) {
    const SeqDef *s = store.sequencesStart + i;
    size_t ll = s->litLength, ml = add(s->mlBase, MINMATCH), j;
    U32 distance;
    if (store.longLengthType != ZSTD_llt_none && store.longLengthPos == i) {
      if (store.longLengthType == ZSTD_llt_literalLength)
        ll = add(ll, 65536);
      else
        ml = add(ml, 65536);
    }
    require(literal_cursor <= literal_bytes &&
                ll <= literal_bytes - literal_cursor,
            "stored literal range");
    require(cursor <= n && ll <= n - cursor, "literal replay range");
    memcpy(replay.data + cursor, store.litStart + literal_cursor, ll);
    cursor += ll;
    literal_cursor += ll;
    distance = resolve_repeat(wire_repeats, s->offBase, ll);
    require(distance <= cursor && distance <= window &&
                cursor - distance >= r->floor,
            "match replay history");
    require(ml >= 4 && ml <= n - cursor, "match replay length");
    for (j = 0; j < ml; ++j) {
      replay.data[cursor] = replay.data[cursor - distance];
      ++cursor;
    }
    observations[i].ll = ll;
    observations[i].ml = ml;
    observations[i].off_base = s->offBase;
    observations[i].distance = distance;
  }
  require(literal_cursor == literal_bytes && trailing == n - cursor,
          "complete literal consumption");
  memcpy(replay.data + cursor, source + cursor, trailing);
  require(memcmp(replay.data, source, n) == 0, "independent replay differs");
  require(parser_repeats[0] == wire_repeats[0] &&
              parser_repeats[1] == wire_repeats[1],
          "parser-used repeats differ");
  require(parser_repeats[2] == r->repeats[2], "unused raw repeat changed");
  table_check(&first, &second, r, source, n);
  guarded_check(&replay);
  guarded_check(&observed);
  guarded_check(&r->source);
  guarded_check(&r->seeds);
  require(state.nextToUpdate >= r->next + 2 && state.nextToUpdate <= n + 2 &&
              (state.lazySkipping == 0 || state.lazySkipping == 1) &&
              state.hashSalt == r->salt,
          "final search scalar bounds");
  current = r->prefix + (r->prefix == r->floor);
  repeat_floor = current > window ? current - window : 0;
  if (repeat_floor < r->floor)
    repeat_floor = r->floor;
  printf("{\"record\":\"case\",\"id\":\"%s\",\"padding\":%u,", r->name,
         padding);
  printf("\"prefix_bytes\":%u,\"block_bytes\":%u,\"source_bytes\":%zu,\"source_"
         "fnv1a64\":\"%016" PRIx64 "\",",
         r->prefix, r->block, n, fingerprint(source, n));
  printf("\"depth\":%u,\"strategy\":%u,\"branch\":%u,\"window_log\":%u,\"hash_"
         "log\":%u,\"chain_log\":%u,"
         "\"search_log\":%u,\"min_match\":%u,\"target_length\":%u,"
         "\"prefix_floor\":%u,\"next_to_update\":%u,\"key_width\":%u,\"row_"
         "log\":%u,",
         r->depth, (unsigned)strategy(r->depth), r->branch, r->window_log,
         r->hash_log, r->chain_log, r->search_log, r->min_match,
         r->target_length, r->floor, r->next, width(r),
         r->branch ? row_log(r) : 0);
  printf("\"initial_dict_limit\":%u,\"initial_low_limit\":%u,"
         "\"initial_next_to_update\":%u,\"final_next_to_update\":%u,"
         "\"final_lazy_skipping\":%d,\"initial_salt\":\"%016" PRIx64 "\","
         "\"final_salt\":\"%016" PRIx64
         "\",\"initial_entropy\":%u,\"final_entropy\":%u,"
         "\"loaded_dict_end\":0,\"dictionary_state_null\":true,",
         r->floor + 2, r->floor + 2, r->next + 2, state.nextToUpdate,
         state.lazySkipping, (uint64_t)r->salt, (uint64_t)state.hashSalt,
         r->entropy, state.hashSaltEntropy);
  printf("\"initial_current\":%zu,\"repeat_floor\":%zu,\"physical_leading_"
         "bytes\":%u,"
         "\"physical_suffix_bytes\":%u,\"sequence_capacity\":%zu,\"literal_"
         "slack_bytes\":%u,"
         "\"requested_allocation_bytes\":%zu,\"hash_cache\":[",
         current, repeat_floor, LEADING, SUFFIX, q, WILDCOPY_OVERLENGTH, total);
  for (i = 0; i < ZSTD_ROW_HASH_CACHE_SIZE; ++i) {
    if (i)
      putchar(',');
    printf("%u", state.hashCache[i]);
  }
  fputs("],\"seeds\":", stdout);
  print_seeds((const U32 *)r->seeds.data, r->seed_count);
  fputs(",\"initial_repeats\":", stdout);
  print_repeats(r->repeats);
  fputs(",\"parser_repeats\":", stdout);
  print_repeats(parser_repeats);
  fputs(",\"wire_repeats\":", stdout);
  print_repeats(wire_repeats);
  printf(",\"sequence_count\":%zu,\"stored_literal_bytes\":%zu,\"trailing_"
         "literals\":%zu,\"long_length_type\":%u,\"long_length_position\":%u,"
         "\"stored_literals_hex\":",
         count, literal_bytes, trailing, (unsigned)store.longLengthType,
         (unsigned)store.longLengthPos);
  print_hex(store.litStart, literal_bytes);
  fputs(",\"sequences\":[", stdout);
  for (i = 0; i < count; ++i) {
    const Observation *o = observations + i;
    const SeqDef *s = store.sequencesStart + i;
    if (i)
      putchar(',');
    printf("{\"raw_ll\":%u,\"raw_ml_base\":%u,\"off_base\":%" PRIu32
           ",\"ll\":%zu,\"ml\":%zu,\"distance\":%" PRIu32 ",\"intent\":\"%s\"}",
           (unsigned)s->litLength, (unsigned)s->mlBase, o->off_base, o->ll,
           o->ml, o->distance, o->off_base <= 3 ? "repeat" : "offset");
  }
  fputs("],\"first_table\":", stdout);
  print_table(&first);
  fputs(",\"second_table\":", stdout);
  print_table(&second);
  fputs(",\"source_and_slack_unchanged\":true,\"replay_equal\":true,\"guards_"
        "intact\":true}\n",
        stdout);
  guarded_drop(&observed);
  guarded_drop(&replay);
  guarded_drop(&codes);
  guarded_drop(&literals);
  guarded_drop(&sequences);
  table_drop(&second);
  table_drop(&first);
  guarded_drop(&arena_copy);
  guarded_drop(&arena);
  require(live_allocation_bytes ==
              add(guarded_cost(n), guarded_cost(multiply(r->seed_count, 4))),
          "execution owner leak");
}
static U32 read_u32(FILE *f) {
  unsigned char b[4];
  require(fread(b, 1, 4, f) == 4, "truncated scalar");
  return (U32)b[0] | (U32)b[1] << 8 | (U32)b[2] << 16 | (U32)b[3] << 24;
}
static Recipe read_recipe(const char *path) {
  Recipe r;
  struct stat status;
  FILE *f;
  unsigned char magic[8];
  size_t i, terminator = 64, n;
  U32 *seeds, salt_low, salt_high;
  require(stat(path, &status) == 0 && S_ISREG(status.st_mode),
          "input must be regular");
  require(status.st_size >= 152 && (uintmax_t)status.st_size <= MAX_PACKET,
          "packet byte cap");
  f = fopen(path, "rb");
  require(f != NULL, "open packet");
  require(fstat(fileno(f), &status) == 0 && S_ISREG(status.st_mode) &&
              status.st_size >= 152 && (uintmax_t)status.st_size <= MAX_PACKET,
          "opened input must be bounded regular file");
  memset(&r, 0, sizeof(r));
  require(fread(magic, 1, 8, f) == 8 && memcmp(magic, "ZLAZY01\0", 8) == 0,
          "packet magic");
  require(fread(r.name, 1, 64, f) == 64, "truncated id");
  for (i = 0; i < 64; ++i) {
    unsigned char c = (unsigned char)r.name[i];
    if (c == 0 && terminator == 64)
      terminator = i;
    require((terminator < 64 && c == 0) ||
                (terminator == 64 &&
                 ((c >= 'a' && c <= 'z') || (c >= '0' && c <= '9') ||
                  c == '-' || c == '_')),
            "bounded ASCII id");
  }
  require(terminator > 0 && terminator < 64, "id termination");
  r.branch = read_u32(f);
  r.prefix = read_u32(f);
  r.block = read_u32(f);
  r.window_log = read_u32(f);
  r.hash_log = read_u32(f);
  r.chain_log = read_u32(f);
  r.search_log = read_u32(f);
  r.min_match = read_u32(f);
  r.target_length = read_u32(f);
  r.floor = read_u32(f);
  r.next = read_u32(f);
  for (i = 0; i < 3; ++i)
    r.repeats[i] = read_u32(f);
  salt_low = read_u32(f);
  salt_high = read_u32(f);
  r.salt = (U64)salt_low | (U64)salt_high << 32;
  r.entropy = read_u32(f);
  r.seed_count = read_u32(f);
  r.depth = read_u32(f);
  require(read_u32(f) == 0, "reserved second scalar");
  validate_scalars(&r);
  n = add(r.prefix, r.block);
  require(add(add(152, multiply(r.seed_count, 4)), n) ==
              (uintmax_t)status.st_size,
          "exact packet extent");
  r.seeds = guarded_new(multiply(r.seed_count, 4));
  seeds = (U32 *)r.seeds.data;
  for (i = 0; i < r.seed_count; ++i) {
    seeds[i] = read_u32(f);
    require(seeds[i] < r.next && seeds[i] < r.prefix &&
                width(&r) <= r.prefix - seeds[i] &&
                (i == 0 || seeds[i - 1] < seeds[i]),
            "ordered prefix seed");
  }
  r.source = guarded_new(n);
  require(fread(r.source.data, 1, n, f) == n, "truncated source");
  require(fgetc(f) == EOF && !ferror(f), "extra source bytes or read error");
  require(fclose(f) == 0, "close packet");
  return r;
}

/* Fresh CCtx owners use a separate bounded ledger. The callbacks return the
 * guarded payload unchanged and recover the exact malloc owner on free. */
typedef struct {
  Guarded owners[64];
  size_t calls, frees, peak_bytes;
  unsigned fill;
} ContextOwners;
static void *context_allocate(void *opaque, size_t bytes) {
  ContextOwners *owners = (ContextOwners *)opaque;
  size_t i, cost = guarded_cost(bytes);
  if (cost > 8u * 1024u * 1024u ||
      live_allocation_bytes > 8u * 1024u * 1024u - cost)
    return NULL;
  for (i = 0; i < 64; ++i)
    if (owners->owners[i].allocation == NULL) {
      owners->owners[i] = guarded_new(bytes);
      /* Distinct nonzero fills prove fresh zero observations come from C
       * initialization, not the helper's zeroed direct-table setup. */
      memset(owners->owners[i].data, (int)owners->fill, bytes ? bytes : 1);
      ++owners->calls;
      if (live_allocation_bytes > owners->peak_bytes)
        owners->peak_bytes = live_allocation_bytes;
      return owners->owners[i].data;
    }
  return NULL;
}
static void context_free(void *opaque, void *pointer) {
  ContextOwners *owners = (ContextOwners *)opaque;
  size_t i;
  if (pointer == NULL)
    return;
  for (i = 0; i < 64; ++i)
    if (owners->owners[i].data == pointer) {
      guarded_drop(&owners->owners[i]);
      ++owners->frees;
      return;
    }
  require(0, "CCtx free has no exact owner");
}
static void checked(size_t result, const char *reason) {
  require(!ZSTD_isError(result), reason);
}
static void fresh_salt(unsigned depth) {
  unsigned run;
  for (run = 0; run < 2; ++run) {
    ContextOwners owners;
    ZSTD_customMem memory;
    ZSTD_CCtx *context;
    ZSTD_CCtx_params params;
    ZSTD_parameters p;
    U64 initial_salt, final_salt;
    U32 initial_entropy, final_entropy;
    size_t i;
    memset(&owners, 0, sizeof(owners));
    owners.fill = run == 0 ? 0x3b : 0xd1;
    memset(&params, 0, sizeof(params));
    memset(&p, 0, sizeof(p));
    require(live_allocation_bytes == 0, "fresh salt owner separation");
    memory.customAlloc = context_allocate;
    memory.customFree = context_free;
    memory.opaque = &owners;
    context = ZSTD_createCCtx_advanced(memory);
    require(context != NULL, "fresh CCtx allocation");
    initial_salt = context->blockState.matchState.hashSalt;
    initial_entropy = context->blockState.matchState.hashSaltEntropy;
    require(initial_salt == 0 && initial_entropy == 0,
            "fresh CCtx initial salt");
    p.cParams.windowLog = 15;
    p.cParams.chainLog = 6;
    p.cParams.hashLog = 6;
    p.cParams.searchLog = 1;
    p.cParams.minMatch = 4;
    p.cParams.strategy = strategy(depth);
    p.fParams.contentSizeFlag = 1;
    checked(ZSTD_checkCParams(p.cParams), "fresh salt cParams");
    checked(ZSTD_CCtxParams_init_advanced(&params, p), "fresh salt parameters");
    checked(ZSTD_CCtxParams_setParameter(&params, ZSTD_c_useRowMatchFinder,
                                         ZSTD_ps_enable),
            "enable row");
    checked(ZSTD_CCtxParams_setParameter(&params, ZSTD_c_nbWorkers, 0),
            "disable workers");
    checked(ZSTD_CCtxParams_setParameter(
                &params, ZSTD_c_enableLongDistanceMatching, ZSTD_ps_disable),
            "disable LDM");
    checked(ZSTD_compressBegin_advanced_internal(context, NULL, 0,
                                                 ZSTD_dct_auto, ZSTD_dtlm_fast,
                                                 NULL, &params, 32768),
            "fresh context begin");
    {
      const ZSTD_compressionParameters *a =
          &context->blockState.matchState.cParams;
      const ZSTD_compressionParameters *b = &context->appliedParams.cParams;
      require(
          a->windowLog == 15 && a->chainLog == 6 && a->hashLog == 6 &&
              a->searchLog == 1 && a->minMatch == 4 && a->targetLength == 0 &&
              a->strategy == strategy(depth) && b->windowLog == a->windowLog &&
              b->chainLog == a->chainLog && b->hashLog == a->hashLog &&
              b->searchLog == a->searchLog && b->minMatch == a->minMatch &&
              b->targetLength == a->targetLength && b->strategy == a->strategy,
          "fresh salt applied tuple");
    }
    require(context->appliedParams.useRowMatchFinder == ZSTD_ps_enable &&
                context->appliedParams.nbWorkers == 0 &&
                context->appliedParams.ldmParams.enableLdm == ZSTD_ps_disable &&
                !context->appliedParams.fParams.checksumFlag &&
                context->blockState.matchState.rowHashLog == 2 &&
                context->blockState.matchState.loadedDictEnd == 0 &&
                context->blockState.matchState.dictMatchState == NULL &&
                context->blockState.matchState.hashTable != NULL &&
                context->blockState.matchState.tagTable != NULL,
            "fresh salt applied mode");
    for (i = 0; i < 64; ++i)
      require(context->blockState.matchState.hashTable[i] == 0 &&
                  context->blockState.matchState.tagTable[i] == 0,
              "fresh zero tables");
    final_salt = context->blockState.matchState.hashSalt;
    final_entropy = context->blockState.matchState.hashSaltEntropy;
    checked(ZSTD_freeCCtx(context), "release fresh CCtx");
    require(live_allocation_bytes == 0 && owners.calls == owners.frees,
            "fresh CCtx owner leak");
    printf(
        "{\"record\":\"salt\",\"depth\":%u,\"run\":%u,\"allocation_fill\":%u,"
        "\"resolved\":[15,6,6,1,4,0,%u],"
        "\"initial_salt\":\"%016" PRIx64 "\",\"final_salt\":\"%016" PRIx64 "\","
        "\"initial_entropy\":\"%08" PRIx32 "\",\"final_entropy\":\"%08" PRIx32
        "\","
        "\"row_hash_log\":2,\"table_entries\":64,\"zero_tables\":true,"
        "\"allocate_calls\":%zu,\"free_calls\":%zu,\"peak_guarded_bytes\":%"
        "zu,\"final_live_bytes\":0}\n",
        depth, run, owners.fill, (unsigned)strategy(depth),
        (uint64_t)initial_salt, (uint64_t)final_salt, initial_entropy,
        final_entropy, owners.calls, owners.frees, owners.peak_bytes);
  }
  printf("{\"record\":\"complete\",\"mode\":\"fresh-salt\",\"depth\":%u,"
         "\"observations\":2,"
         "\"success\":true}\n",
         depth);
}
static void metadata(const char *mode, unsigned depth, unsigned branch) {
  const uint16_t endian = 1;
  printf(
      "{\"record\":\"metadata\",\"schema_version\":1,\"revision\":"
      "\"f8745da6ff1ad1e7bab384bd1f9d742439278e99\",\"protocol\":\"ZLAZY01\","
      "\"mode\":\"%s\",\"depth\":%u,\"strategy\":%u,\"branch\":%u,\"ordinary_"
      "frames\":false,\"collector\":"
      "false,"
      "\"entry_point\":\"%s\",\"observation_kind\":\"sequences-and-final-"
      "tables-not-chronological-trace\","
      "\"pointer_bits\":%zu,\"little_endian\":%s,\"guard_bytes\":64,"
      "\"alignment\":64,"
      "\"alignment_slack\":63,\"leading_bytes\":64,\"suffix_bytes\":2048,"
      "\"max_prefix_bytes\":%u,\"max_block_bytes\":%u,\"max_seeds\":%u,"
      "\"max_packet_bytes\":%u,\"max_allocation_bytes\":%u,\"max_context_"
      "bytes\":8388608,"
      "\"u32_bytes\":%zu,\"u32_alignment\":%zu,\"sequence_bytes\":%zu,"
      "\"sequence_alignment\":%zu,"
      "\"observation_bytes\":%zu,\"observation_alignment\":%zu,\"state_bytes\":"
      "%zu,\"state_alignment\":%zu,"
      "\"hash_read_bytes\":%u,\"window_start_index\":%u,\"minimum_match\":%u,"
      "\"wildcopy_overlength\":%u,\"hash_cache_size\":%u,\"row_tag_bits\":%u,"
      "\"upstream_sanitizer_annotations_retained\":true}\n",
      mode, depth, (unsigned)strategy(depth), branch,
      !strcmp(mode, "fresh-salt") ? "ZSTD_compressBegin_advanced_internal"
                                  : entry_point(depth, branch),
      sizeof(void *) * CHAR_BIT,
      *(const unsigned char *)&endian ? "true" : "false", MAX_PREFIX, MAX_BLOCK,
      MAX_SEEDS, MAX_PACKET, MAX_ALLOCATION, sizeof(U32), _Alignof(U32),
      sizeof(SeqDef), _Alignof(SeqDef), sizeof(Observation),
      _Alignof(Observation), sizeof(ZSTD_MatchState_t),
      _Alignof(ZSTD_MatchState_t), HASH_READ_SIZE, ZSTD_WINDOW_START_INDEX,
      MINMATCH, WILDCOPY_OVERLENGTH, ZSTD_ROW_HASH_CACHE_SIZE,
      ZSTD_ROW_HASH_TAG_BITS);
}
int main(int argc, char **argv) {
  Recipe recipe;
  unsigned depth;
  require(argc == 2, "usage: lazy-direct packet | --fresh-salt-0 | "
                     "--fresh-salt-1 | --fresh-salt-2");
  require(ZSTD_versionNumber() == 10507 &&
              sizeof(uintptr_t) >= sizeof(void *) &&
              64 % _Alignof(max_align_t) == 0,
          "pinned reference/host alignment");
  if (!strcmp(argv[1], "--fresh-salt-0") ||
      !strcmp(argv[1], "--fresh-salt-1") ||
      !strcmp(argv[1], "--fresh-salt-2")) {
    /* The exact comparisons prove a 14-byte option and digit 0..2 at 13. */
    depth = (unsigned)(argv[1][13] - '0');
    metadata("fresh-salt", depth, 1);
    fresh_salt(depth);
  } else {
    recipe = read_recipe(argv[1]);
    metadata("direct", recipe.depth, recipe.branch);
    execute(&recipe, 0x6a);
    execute(&recipe, 0xc3);
    guarded_drop(&recipe.source);
    guarded_drop(&recipe.seeds);
    require(live_allocation_bytes == 0, "final owner leak");
    printf("{\"record\":\"complete\",\"mode\":\"direct\",\"id\":\"%s\","
           "\"depth\":%u,\"observations\":2,\"success\":true}\n",
           recipe.name, recipe.depth);
  }
  require(!ferror(stdout) && fflush(stdout) == 0, "output write error");
  return 0;
}
