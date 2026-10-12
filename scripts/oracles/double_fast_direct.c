/* Bounded observations of unchanged facebook/zstd v1.5.7 DoubleFast.
 * Reference f8745da6ff1ad1e7bab384bd1f9d742439278e99; setup and replay only.
 * Arena/protocol contract: docs/reference-double-fast-oracle.md.
 */
#define _POSIX_C_SOURCE 200809L
#define ZSTD_STATIC_LINKING_ONLY
#include "compress/zstd_double_fast.h"
#include "zstd.h"
#include <inttypes.h>
#include <limits.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>

#define GUARD 64u
#define LEADING 64u
#define MAX_PREFIX (2u * 1024u * 1024u)
#define MAX_BLOCK (128u * 1024u)
#define SUFFIX (MAX_BLOCK + 512u)
#define MAX_SEEDS 1024u
#define MAX_PACKET (132u + 8u * MAX_SEEDS + MAX_PREFIX + MAX_BLOCK)
#define MAX_ALLOCATION (64u * 1024u * 1024u)

typedef struct {
  unsigned char *allocation, *data;
  size_t bytes;
} Guarded;
typedef struct {
  char name[64];
  U32 prefix, block, window_log, long_log, short_log, search_log, min_match;
  U32 target_length, floor, repeats[3], long_count, short_count;
  U32 long_seeds[MAX_SEEDS], short_seeds[MAX_SEEDS];
  Guarded source;
} Recipe;
typedef struct {
  Guarded storage, before_storage;
  U32 *cells, *before;
  size_t count;
  unsigned log, width;
} Table;
typedef struct {
  size_t ll, ml;
  U32 off_base, distance;
} Observation;
static const char *active_case = "setup";
static size_t live_allocation_bytes;

static void require(int valid, const char *reason) {
  if (!valid) {
    fprintf(stderr, "double-fast oracle [%s]: %s\n", active_case, reason);
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
  return add(add(GUARD, bytes), GUARD);
}
static Guarded guarded_new(size_t bytes) {
  Guarded result;
  size_t cost = guarded_cost(bytes), total = add(live_allocation_bytes, cost);
  require(total <= MAX_ALLOCATION, "aggregate allocation cap");
  result.allocation = (unsigned char *)malloc(cost);
  require(result.allocation != NULL, "allocation failed");
  result.data = result.allocation + GUARD;
  result.bytes = bytes;
  memset(result.allocation, 0xd7, GUARD);
  memset(result.data, 0, bytes);
  memset(result.data + bytes, 0x9b, GUARD);
  live_allocation_bytes = total;
  return result;
}
static void guarded_check(const Guarded *g) {
  size_t i;
  for (i = 0; i < GUARD; ++i) {
    require(g->allocation[i] == 0xd7, "leading guard changed");
    require(g->data[g->bytes + i] == 0x9b, "trailing guard changed");
  }
}
static void guarded_drop(Guarded *g) {
  size_t cost = guarded_cost(g->bytes);
  guarded_check(g);
  require(cost <= live_allocation_bytes, "allocation accounting underflow");
  free(g->allocation);
  live_allocation_bytes -= cost;
  g->allocation = NULL;
  g->data = NULL;
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
static ZSTD_compressionParameters parameters(const Recipe *r) {
  ZSTD_compressionParameters p;
  memset(&p, 0, sizeof(p));
  p.windowLog = r->window_log;
  p.chainLog = r->short_log;
  p.hashLog = r->long_log;
  p.searchLog = r->search_log;
  p.minMatch = r->min_match;
  p.targetLength = r->target_length;
  p.strategy = ZSTD_dfast;
  return p;
}
static void validate_scalars(const Recipe *r) {
  size_t n = add(r->prefix, r->block);
  unsigned i;
  require(ZSTD_WINDOW_START_INDEX == 2 && HASH_READ_SIZE == 8 && MINMATCH == 3,
          "pinned constants");
  require(r->prefix <= MAX_PREFIX && r->block >= 7 && r->block <= MAX_BLOCK,
          "source bounds");
  require(n <= UINT32_MAX - 2 && n <= MAX_PREFIX + MAX_BLOCK,
          "normalized index range");
  require(r->window_log >= 10 && r->window_log <= 21 &&
              r->window_log < sizeof(size_t) * CHAR_BIT,
          "window log");
  require(r->block <= ((size_t)1 << r->window_log), "block exceeds window");
  require(r->long_log >= 6 && r->long_log <= 18 &&
              r->long_log < sizeof(size_t) * CHAR_BIT,
          "long log");
  require(r->short_log >= 6 && r->short_log <= 18 &&
              r->short_log < sizeof(size_t) * CHAR_BIT,
          "short log");
  require(r->search_log >= 1 && r->search_log <= 29, "search log");
  require(r->min_match >= 3 && r->min_match <= 7 && r->target_length <= 131072,
          "match/target fields");
  require(r->floor <= r->prefix && r->long_count <= MAX_SEEDS &&
              r->short_count <= MAX_SEEDS,
          "floor/seed counts");
  for (i = 0; i < 3; ++i)
    require(r->repeats[i] != 0, "initial repeat must be positive");
  require(!ZSTD_isError(ZSTD_checkCParams(parameters(r))),
          "invalid C parameter tuple");
  require(GUARD % _Alignof(U32) == 0 && GUARD % _Alignof(SeqDef) == 0 &&
              GUARD % _Alignof(Observation) == 0,
          "guard alignment");
}
static Table table_new(unsigned log, unsigned width, const U32 *seeds,
                       U32 seed_count, const Recipe *r,
                       const unsigned char *source) {
  Table t;
  U32 i;
  t.log = log;
  t.width = width;
  t.count = (size_t)1 << log;
  t.storage = guarded_new(multiply(t.count, sizeof(U32)));
  t.before_storage = guarded_new(t.storage.bytes);
  t.cells = (U32 *)t.storage.data;
  t.before = (U32 *)t.before_storage.data;
  for (i = 0; i < seed_count; ++i) {
    size_t position = seeds[i], bucket;
    require(position < r->prefix && width <= r->prefix - position,
            "seed selected key crosses prefix");
    /* For widths5..7 the physical eight-byte C read can enter the current
     * block. B>=7 guarantees those bytes are initialized in this arena. */
    bucket = ZSTD_hashPtr(source + position, log, width);
    require(bucket < t.count, "seed bucket range");
    t.cells[bucket] = (U32)(position + 2);
  }
  memcpy(t.before, t.cells, t.storage.bytes);
  return t;
}
static void table_check(const Table *t, const unsigned char *base, size_t n) {
  size_t i;
  guarded_check(&t->storage);
  guarded_check(&t->before_storage);
  for (i = 0; i < t->count; ++i) {
    U32 p = t->cells[i];
    if (p != 0) {
      require(p >= 2 && n >= t->width && p - 2 <= n - t->width,
              "final table key range");
      require(ZSTD_hashPtr(base + p, t->log, t->width) == i,
              "final table key/bucket mismatch");
    }
  }
}
static void print_table_hash(const U32 *cells, size_t count) {
  uint64_t h = UINT64_C(0xcbf29ce484222325);
  size_t i;
  for (i = 0; i < count; ++i) {
    unsigned byte;
    for (byte = 0; byte < 4; ++byte) {
      h ^= (cells[i] >> (8 * byte)) & 255;
      h *= UINT64_C(0x100000001b3);
    }
  }
  printf("\"%016" PRIx64 "\"", h);
}
static void print_table(const Table *t) {
  size_t i;
  int comma = 0;
  printf("{\"log\":%u,\"width\":%u,\"entries\":%zu,\"before\":[", t->log,
         t->width, t->count);
  for (i = 0; i < t->count; ++i)
    if (t->before[i]) {
      if (comma)
        putchar(',');
      comma = 1;
      printf("[%zu,%" PRIu32 "]", i, t->before[i]);
    }
  fputs("],\"changes\":[", stdout);
  comma = 0;
  for (i = 0; i < t->count; ++i)
    if (t->before[i] != t->cells[i]) {
      if (comma)
        putchar(',');
      comma = 1;
      printf("[%zu,%" PRIu32 ",%" PRIu32 "]", i, t->before[i], t->cells[i]);
    }
  fputs("],\"before_le32_fnv1a64\":", stdout);
  print_table_hash(t->before, t->count);
  fputs(",\"after_le32_fnv1a64\":", stdout);
  print_table_hash(t->cells, t->count);
  putchar('}');
}
static void table_drop(Table *t) {
  guarded_drop(&t->before_storage);
  guarded_drop(&t->storage);
}

static void execute(const Recipe *r, unsigned padding) {
  const unsigned short_width = r->min_match < 4 ? 4 : r->min_match;
  size_t n, arena_bytes, q, total, trailing, count, literal_bytes, cursor,
      literal_cursor, i;
  size_t window, ordinary_floor, current, repeat_floor;
  Guarded arena, arena_copy, sequences, literals, codes, replay, observed;
  Table long_table, short_table;
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
  total = guarded_cost(n);
  total = add(total, multiply(2, guarded_cost(arena_bytes)));
  total = add(total, multiply(2, guarded_cost(multiply((size_t)1 << r->long_log,
                                                       sizeof(U32)))));
  total = add(total, multiply(2, guarded_cost(multiply(
                                     (size_t)1 << r->short_log, sizeof(U32)))));
  total = add(total, guarded_cost(multiply(q, sizeof(SeqDef))));
  total = add(total, guarded_cost(add(r->block, WILDCOPY_OVERLENGTH)));
  total = add(total, guarded_cost(multiply(q, 3)));
  total = add(total, guarded_cost(n));
  total = add(total, guarded_cost(multiply(q, sizeof(Observation))));
  require(total <= MAX_ALLOCATION, "preflight allocation sum");
  /* All formed pointers stay in one initialized arena: source-2, iend-8
   * and iend-32 fit the leading64 bytes even for B=7. Outer ip+256 and
   * prefetches fit the suffix. An unsuccessful probe advances >=1 byte,
   * giving step<=B+1, hence the final ip1+=step <=iend+MAX_BLOCK+1.
   * nextStep's guarded +256 is <=iend+248. Suffix MAX_BLOCK+512 covers
   * these pointer expressions, not just actual loads. Logical bytes end at n.
   */
  arena = guarded_new(arena_bytes);
  arena_copy = guarded_new(arena_bytes);
  memset(arena.data, (int)padding, arena.bytes);
  source = arena.data + LEADING;
  base = source - 2;
  memcpy(source, r->source.data, n);
  memcpy(arena_copy.data, arena.data, arena.bytes);
  long_table =
      table_new(r->long_log, 8, r->long_seeds, r->long_count, r, source);
  short_table = table_new(r->short_log, short_width, r->short_seeds,
                          r->short_count, r, source);
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
  state.hashTable = long_table.cells;
  state.chainTable = short_table.cells;
  state.nextToUpdate = 2;
  /* All dictionary/optional state stays zero. Output Q is conservative:
   * each DoubleFast match consumes >=4 bytes, while Q=floor(B/3)+1.
   * Initialized literal write slack ends before its canary. */
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
  trailing = ZSTD_compressBlock_doubleFast(&state, &store, parser_repeats,
                                           source + r->prefix, r->block);
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
  table_check(&long_table, base, n);
  table_check(&short_table, base, n);
  guarded_check(&replay);
  guarded_check(&observed);
  guarded_check(&r->source);
  ordinary_floor = n > window ? n - window : 0;
  if (ordinary_floor < r->floor)
    ordinary_floor = r->floor;
  current = r->prefix + (r->prefix == ordinary_floor);
  repeat_floor = current > window ? current - window : 0;
  if (repeat_floor < r->floor)
    repeat_floor = r->floor;
  printf("{\"record\":\"case\",\"id\":\"%s\",\"padding\":%u,", r->name,
         padding);
  printf("\"prefix_bytes\":%u,\"block_bytes\":%u,\"source_bytes\":%zu,\"source_"
         "fnv1a64\":\"%016" PRIx64 "\",",
         r->prefix, r->block, n, fingerprint(source, n));
  printf(
      "\"window_log\":%u,\"long_hash_log\":%u,\"short_hash_log\":%u,\"search_"
      "log\":%u,\"min_match\":%u,\"target_length\":%u,\"prefix_floor\":%u,",
      r->window_log, r->long_log, r->short_log, r->search_log, r->min_match,
      r->target_length, r->floor);
  printf("\"initial_dict_limit\":%u,\"initial_low_limit\":%u,\"initial_next_to_"
         "update\":2,\"loaded_dict_end\":0,\"dictionary_state_null\":true,",
         r->floor + 2, r->floor + 2);
  printf("\"ordinary_floor\":%zu,\"initial_current\":%zu,\"repeat_floor\":%zu,"
         "\"physical_leading_bytes\":%u,\"physical_suffix_bytes\":%u,"
         "\"sequence_capacity\":%zu,\"literal_slack_bytes\":%u,\"requested_"
         "allocation_bytes\":%zu,",
         ordinary_floor, current, repeat_floor, LEADING, SUFFIX, q,
         WILDCOPY_OVERLENGTH, total);
  fputs("\"long_seeds\":", stdout);
  print_seeds(r->long_seeds, r->long_count);
  fputs(",\"short_seeds\":", stdout);
  print_seeds(r->short_seeds, r->short_count);
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
  fputs("],\"long_table\":", stdout);
  print_table(&long_table);
  fputs(",\"short_table\":", stdout);
  print_table(&short_table);
  fputs(",\"source_and_slack_unchanged\":true,\"replay_equal\":true,\"guards_"
        "intact\":true}\n",
        stdout);
  guarded_drop(&observed);
  guarded_drop(&replay);
  guarded_drop(&codes);
  guarded_drop(&literals);
  guarded_drop(&sequences);
  table_drop(&short_table);
  table_drop(&long_table);
  guarded_drop(&arena_copy);
  guarded_drop(&arena);
  require(live_allocation_bytes == guarded_cost(n), "execution owner leak");
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
  unsigned width;
  require(stat(path, &status) == 0 && S_ISREG(status.st_mode),
          "input must be a regular file");
  require(status.st_size >= 132 && (uintmax_t)status.st_size <= MAX_PACKET,
          "packet byte cap");
  f = fopen(path, "rb");
  require(f != NULL, "open packet");
  require(fstat(fileno(f), &status) == 0 && S_ISREG(status.st_mode),
          "opened input must be regular");
  memset(&r, 0, sizeof(r));
  require(fread(magic, 1, 8, f) == 8 && memcmp(magic, "ZDFST01\0", 8) == 0,
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
  r.prefix = read_u32(f);
  r.block = read_u32(f);
  r.window_log = read_u32(f);
  r.long_log = read_u32(f);
  r.short_log = read_u32(f);
  r.search_log = read_u32(f);
  r.min_match = read_u32(f);
  r.target_length = read_u32(f);
  r.floor = read_u32(f);
  for (i = 0; i < 3; ++i)
    r.repeats[i] = read_u32(f);
  r.long_count = read_u32(f);
  r.short_count = read_u32(f);
  require(read_u32(f) == 0, "reserved scalar");
  validate_scalars(&r);
  n = add(r.prefix, r.block);
  width = r.min_match < 4 ? 4 : r.min_match;
  require(add(add(132, multiply(add(r.long_count, r.short_count), 4)), n) ==
              (uintmax_t)status.st_size,
          "exact packet extent");
  for (i = 0; i < r.long_count; ++i) {
    r.long_seeds[i] = read_u32(f);
    require(r.long_seeds[i] < r.prefix && 8 <= r.prefix - r.long_seeds[i],
            "long seed before block");
  }
  for (i = 0; i < r.short_count; ++i) {
    r.short_seeds[i] = read_u32(f);
    require(r.short_seeds[i] < r.prefix && width <= r.prefix - r.short_seeds[i],
            "short seed before block");
  }
  r.source = guarded_new(n);
  require(fread(r.source.data, 1, n, f) == n, "truncated source");
  require(fgetc(f) == EOF && !ferror(f), "extra source bytes or read error");
  require(fclose(f) == 0, "close packet");
  return r;
}
int main(int argc, char **argv) {
  Recipe r;
  const uint16_t endian = 1;
  require(argc == 2, "usage: double-fast-direct packet");
  require(ZSTD_versionNumber() == 10507, "reference version");
  r = read_recipe(argv[1]);
  printf("{\"record\":\"metadata\",\"schema_version\":1,\"revision\":"
         "\"f8745da6ff1ad1e7bab384bd1f9d742439278e99\",\"entry_point\":\"ZSTD_"
         "compressBlock_doubleFast\",\"strategy\":2,\"protocol\":\"ZDFST01\","
         "\"ordinary_frames\":false,\"collector\":false,\"observation_kind\":"
         "\"sequences-and-final-tables-not-chronological-trace\",\"pointer_"
         "bits\":%zu,\"little_endian\":%s,\"guard_bytes\":%u,\"leading_bytes\":"
         "%u,\"suffix_bytes\":%u,\"max_prefix_bytes\":%u,\"max_block_bytes\":%"
         "u,\"max_packet_bytes\":%u,\"max_allocation_bytes\":%u,\"hash_read_"
         "bytes\":%u,\"window_start_index\":%u,\"minimum_match\":%u,\"wildcopy_"
         "overlength\":%u,\"u32_bytes\":%zu,\"u32_alignment\":%zu,\"sequence_"
         "bytes\":%zu,\"sequence_alignment\":%zu,\"observation_bytes\":%zu,"
         "\"observation_alignment\":%zu,\"state_bytes\":%zu,\"state_"
         "alignment\":%zu,\"store_bytes\":%zu,\"store_alignment\":%zu}\n",
         sizeof(void *) * CHAR_BIT,
         *(const unsigned char *)&endian ? "true" : "false", GUARD, LEADING,
         SUFFIX, MAX_PREFIX, MAX_BLOCK, MAX_PACKET, MAX_ALLOCATION,
         HASH_READ_SIZE, ZSTD_WINDOW_START_INDEX, MINMATCH, WILDCOPY_OVERLENGTH,
         sizeof(U32), _Alignof(U32), sizeof(SeqDef), _Alignof(SeqDef),
         sizeof(Observation), _Alignof(Observation), sizeof(ZSTD_MatchState_t),
         _Alignof(ZSTD_MatchState_t), sizeof(SeqStore_t), _Alignof(SeqStore_t));
  execute(&r, 106);
  execute(&r, 195);
  guarded_drop(&r.source);
  require(live_allocation_bytes == 0, "final owner leak");
  printf("{\"record\":\"complete\",\"id\":\"%s\",\"recipes\":1,"
         "\"observations\":2,\"success\":true}\n",
         r.name);
  require(fflush(stdout) == 0 && !ferror(stdout), "stdout failure");
  return 0;
}
