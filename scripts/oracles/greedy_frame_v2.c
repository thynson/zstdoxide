/* Version 2: tiny ordinary calls may clamp the frontier to the window floor.
 * Version 1 remains preserved, unexecuted, as a pre-execution review artifact.
 * External test oracle for unchanged facebook/zstd v1.5.7 normal Greedy
 * frames. Revision f8745da6ff1ad1e7bab384bd1f9d742439278e99. No copied
 * parser/codec. Setup, checked observations and independent sequence replay
 * only. Adapted from this project's frozen DoubleFast frame helper; no upstream
 * codec copied.
 */
#define ZSTD_STATIC_LINKING_ONLY
#include "compress/zstd_compress_internal.h"
#include "zstd.h"
#include <errno.h>
#include <inttypes.h>
#include <limits.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define SOURCE_CAP (4u * 1024u * 1024u)
#define BLOCK_CAP 131072u
#define TARGET_CAP 131072u
#define SUFFIX_BYTES (TARGET_CAP + BLOCK_CAP + 512u)
#define LEADING_BYTES 64u
#define GUARD_BYTES 64u
#define SCRATCH_BYTES (2u * 1024u * 1024u)
#define CALL_CAP 64u

typedef struct {
  BYTE *allocation;
  BYTE *data;
  size_t size;
} Buffer;
typedef struct {
  U32 rep[3];
  unsigned huf, ll, of, ml;
  unsigned huf_log, huf_max, widths[256];
} State;
typedef struct {
  unsigned last, kind;
  size_t field, payload_offset, payload_size;
} Block;
typedef struct {
  U32 next, skipping, entropy, row_hash_log, cache[8];
  U64 salt;
} Sampling;
static const char *case_name = "setup";
static void need(int ok, const char *why) {
  if (!ok) {
    fprintf(stderr, "frame oracle [%s]: %s\n", case_name, why);
    exit(1);
  }
}
static size_t checked(size_t result, const char *what) {
  if (ZSTD_isError(result)) {
    fprintf(stderr, "frame oracle [%s]: %s: %s\n", case_name, what,
            ZSTD_getErrorName(result));
    exit(1);
  }
  return result;
}
static size_t plus(size_t a, size_t b) {
  need(b <= SIZE_MAX - a, "size addition overflow");
  return a + b;
}
static size_t times(size_t a, size_t b) {
  need(!a || b <= SIZE_MAX / a, "size product overflow");
  return a * b;
}
static uint64_t number(const char *value) {
  char *end;
  unsigned long long n;
  need(value[0] >= '0' && value[0] <= '9', "noncanonical numeric argument");
  errno = 0;
  n = strtoull(value, &end, 10);
  need(!errno && !*end, "invalid numeric argument");
  return (uint64_t)n;
}
static Buffer buffer(size_t size) {
  Buffer b;
  b.size = size;
  b.allocation = (BYTE *)malloc(plus(size, 2 * GUARD_BYTES));
  need(b.allocation != NULL, "allocation failed");
  b.data = b.allocation + GUARD_BYTES;
  memset(b.allocation, 0xd7, GUARD_BYTES);
  memset(b.data, 0, size);
  memset(b.data + size, 0x9b, GUARD_BYTES);
  return b;
}
static void guards(const Buffer *b) {
  size_t i;
  for (i = 0; i < GUARD_BYTES; ++i) {
    need(b->allocation[i] == 0xd7, "leading guard changed");
    need(b->data[b->size + i] == 0x9b, "trailing guard changed");
  }
}
static void drop(Buffer *b) {
  guards(b);
  free(b->allocation);
  b->allocation = NULL;
  b->data = NULL;
}
static uint64_t hash_bytes(const BYTE *p, size_t n) {
  uint64_t h = UINT64_C(0xcbf29ce484222325);
  size_t i;
  for (i = 0; i < n; ++i) {
    h ^= p[i];
    h *= UINT64_C(0x100000001b3);
  }
  return h;
}
static uint64_t hash_table(const U32 *p, size_t n) {
  uint64_t h = UINT64_C(0xcbf29ce484222325);
  size_t i;
  unsigned j;
  for (i = 0; i < n; ++i)
    for (j = 0; j < 4; ++j) {
      h ^= (p[i] >> (j * 8)) & 255;
      h *= UINT64_C(0x100000001b3);
    }
  return h;
}
static void hex(const BYTE *p, size_t n) {
  size_t i;
  putchar('"');
  for (i = 0; i < n; ++i)
    printf("%02x", p[i]);
  putchar('"');
}
static void reps(const U32 r[3]) {
  printf("[%" PRIu32 ",%" PRIu32 ",%" PRIu32 "]", r[0], r[1], r[2]);
}
static void write_file(const char *path, const BYTE *p, size_t n) {
  FILE *f = fopen(path, "wbx");
  need(f != NULL, "cannot open output file");
  need(fwrite(p, 1, n, f) == n, "cannot write output file");
  need(!fclose(f), "cannot close output file");
}
static size_t read_file(const char *path, Buffer *arena) {
  FILE *f = fopen(path, "rb");
  long n;
  size_t count;
  need(f != NULL, "cannot open input file");
  need(!fseek(f, 0, SEEK_END), "input seek failed");
  n = ftell(f);
  need(n >= 0 && (uint64_t)n <= SOURCE_CAP, "input cap exceeded");
  need(!fseek(f, 0, SEEK_SET), "input rewind failed");
  *arena = buffer(plus(plus(LEADING_BYTES, (size_t)n), SUFFIX_BYTES));
  memset(arena->data, 0x6d, arena->size);
  count = fread(arena->data + LEADING_BYTES, 1, (size_t)n, f);
  need(count == (size_t)n && !ferror(f), "input read failed");
  need(fgetc(f) == EOF, "input grew while reading");
  need(!fclose(f), "input close failed");
  return count;
}
static unsigned owner(const ZSTD_compressedBlockState_t *p,
                      const ZSTD_compressedBlockState_t *a,
                      const ZSTD_compressedBlockState_t *b) {
  need(a != b && (p == a || p == b), "unknown block-state owner");
  return p == a ? 0u : 1u;
}
/* Only call on the initially reset previous owner, or an owner initialized by
 * a successful fresh candidate. Never inspect initial pending memory. */
static State snapshot(const ZSTD_compressedBlockState_t *p) {
  State s;
  unsigned i;
  memset(&s, 0, sizeof(s));
  memcpy(s.rep, p->rep, sizeof(s.rep));
  s.huf = (unsigned)p->entropy.huf.repeatMode;
  s.ll = (unsigned)p->entropy.fse.litlength_repeatMode;
  s.of = (unsigned)p->entropy.fse.offcode_repeatMode;
  s.ml = (unsigned)p->entropy.fse.matchlength_repeatMode;
  need(s.huf <= HUF_repeat_valid && s.ll <= FSE_repeat_valid &&
           s.of <= FSE_repeat_valid && s.ml <= FSE_repeat_valid,
       "invalid initialized repeat tag");
  need(s.rep[0] && s.rep[1] && s.rep[2], "zero saved repeat");
  if (s.huf != HUF_repeat_none) {
    HUF_CTableHeader h = HUF_readCTableHeader(p->entropy.huf.CTable);
    s.huf_log = h.tableLog;
    s.huf_max = h.maxSymbolValue;
    need(s.huf_log > 0 && s.huf_log <= HUF_TABLELOG_MAX,
         "invalid HUF table log");
    for (i = 0; i < 256; ++i) {
      s.widths[i] = HUF_getNbBitsFromCTable(p->entropy.huf.CTable, i);
      need(s.widths[i] <= s.huf_log, "invalid HUF width");
    }
  }
  return s;
}
static void print_state(const State *s) {
  unsigned i;
  fputs("{\"raw_repeats\":", stdout);
  reps(s->rep);
  printf(",\"tags\":{\"huf\":%u,\"ll\":%u,\"of\":%u,\"ml\":%u},\"huf_table\":",
         s->huf, s->ll, s->of, s->ml);
  if (s->huf == HUF_repeat_none)
    fputs("null", stdout);
  else {
    printf("{\"log\":%u,\"max_symbol\":%u,\"widths\":[", s->huf_log,
           s->huf_max);
    for (i = 0; i < 256; ++i) {
      if (i)
        putchar(',');
      printf("%u", s->widths[i]);
    }
    fputs("]}", stdout);
  }
  putchar('}');
}
/* Fresh CCtx initialization zeroes the complete match state. Only the row
 * branch reads row-specific fields; all copied padding is initialized zero. */
static Sampling sampling(const ZSTD_MatchState_t *ms, int row) {
  Sampling s;
  memset(&s, 0, sizeof(s));
  s.next = ms->nextToUpdate;
  s.skipping = ms->lazySkipping;
  need(s.skipping <= 1, "skipping domain");
  if (row) {
    s.salt = ms->hashSalt;
    s.entropy = ms->hashSaltEntropy;
    s.row_hash_log = ms->rowHashLog;
    memcpy(s.cache, ms->hashCache, sizeof(s.cache));
  }
  return s;
}
/* Compare named initialized fields, independent of struct padding or ABI. */
static int same_sampling(const Sampling *a, const Sampling *b) {
  return a->next == b->next && a->skipping == b->skipping &&
         a->entropy == b->entropy && a->row_hash_log == b->row_hash_log &&
         a->salt == b->salt && !memcmp(a->cache, b->cache, sizeof(a->cache));
}
static void print_sampling(const Sampling *s) {
  unsigned i;
  printf("{\"next_to_update\":%" PRIu32 ",\"skipping\":%" PRIu32
         ",\"row_hash_log\":%" PRIu32 ",\"salt\":\"%016" PRIx64
         "\",\"salt_entropy\":%" PRIu32 ",\"cache\":[",
         s->next, s->skipping, s->row_hash_log, (uint64_t)s->salt, s->entropy);
  for (i = 0; i < 8; ++i) {
    if (i)
      putchar(',');
    printf("%" PRIu32, s->cache[i]);
  }
  fputs("]}", stdout);
}
static void print_params(const ZSTD_compressionParameters *p) {
  printf("[%u,%u,%u,%u,%u,%u,%u]", p->windowLog, p->hashLog, p->chainLog,
         p->searchLog, p->minMatch, p->targetLength, (unsigned)p->strategy);
}
static int same_params(const ZSTD_compressionParameters *a,
                       const ZSTD_compressionParameters *b) {
  return a->windowLog == b->windowLog && a->chainLog == b->chainLog &&
         a->hashLog == b->hashLog && a->searchLog == b->searchLog &&
         a->minMatch == b->minMatch && a->targetLength == b->targetLength &&
         a->strategy == b->strategy;
}
static void set(ZSTD_CCtx_params *p, ZSTD_cParameter parameter, int value) {
  checked(ZSTD_CCtxParams_setParameter(p, parameter, value),
          "explicit control setter");
}
/* Check integer ranges before any observation dereferences a context table.
 * The pinned reset path establishes initialization within tableValidEnd and
 * initOnceStart; the context retains this workspace throughout the schedule. */
static uintptr_t table_extent(const ZSTD_cwksp *ws, const void *pointer,
                              size_t bytes, int init_once) {
  uintptr_t start = (uintptr_t)pointer;
  uintptr_t low = (uintptr_t)(init_once ? ws->initOnceStart : ws->objectEnd);
  uintptr_t high =
      (uintptr_t)(init_once ? ws->workspaceEnd : ws->tableValidEnd);
  need(pointer != NULL && start >= (uintptr_t)ws->workspace && low <= high &&
           high <= (uintptr_t)ws->workspaceEnd && start >= low &&
           start <= high && bytes <= high - start,
       "initialized workspace table extent");
  return start + bytes;
}
static ZSTD_CCtx_params parameters(unsigned w, unsigned h, unsigned chain,
                                   unsigned search, unsigned m, unsigned t) {
  ZSTD_parameters p;
  ZSTD_CCtx_params cp;
  need(w >= 10 && w <= 21 && h >= 6 && h <= 19 && chain >= 6 && chain <= 29 &&
           search >= 1 && search <= 29 &&
           (w > 14 || (chain <= 18 && search <= 8)) && m >= 3 && m <= 7 &&
           t <= TARGET_CAP,
       "parameter admission");
  memset(&p, 0, sizeof(p));
  p.cParams.windowLog = w;
  p.cParams.chainLog = chain;
  p.cParams.hashLog = h;
  p.cParams.searchLog = search;
  p.cParams.minMatch = m;
  p.cParams.targetLength = t;
  p.cParams.strategy = ZSTD_greedy;
  p.fParams.contentSizeFlag = 1;
  checked(ZSTD_checkCParams(p.cParams), "check cParams");
  checked(ZSTD_CCtxParams_init_advanced(&cp, p),
          "initialize complete parameters");
  set(&cp, ZSTD_c_nbWorkers, 0);
  set(&cp, ZSTD_c_enableLongDistanceMatching, ZSTD_ps_disable);
  set(&cp, ZSTD_c_targetCBlockSize, 0);
  set(&cp, ZSTD_c_useRowMatchFinder, ZSTD_ps_auto);
  set(&cp, ZSTD_c_blockSplitterLevel, 1);
  set(&cp, ZSTD_c_splitAfterSequences, ZSTD_ps_disable);
  set(&cp, ZSTD_c_maxBlockSize, BLOCK_CAP);
  return cp;
}
static void applied(ZSTD_CCtx *c, const ZSTD_CCtx_params *p, uint64_t pledge) {
  size_t expected = (size_t)1 << p->cParams.windowLog;
  need(same_params(&c->appliedParams.cParams, &p->cParams),
       "initializer changed explicit cParams");
  need(same_params(&c->blockState.matchState.cParams, &p->cParams),
       "match-state cParams mismatch");
  need(c->appliedParams.fParams.contentSizeFlag == 1 &&
           !c->appliedParams.fParams.checksumFlag &&
           !c->appliedParams.fParams.noDictIDFlag,
       "frame parameters mismatch");
  need(c->appliedParams.nbWorkers == 0 &&
           c->appliedParams.ldmParams.enableLdm == ZSTD_ps_disable &&
           c->appliedParams.targetCBlockSize == 0 &&
           c->appliedParams.useRowMatchFinder ==
               (p->cParams.windowLog > 14 ? ZSTD_ps_enable : ZSTD_ps_disable) &&
           c->appliedParams.preBlockSplitter_level == 1 &&
           c->appliedParams.postBlockSplitter == ZSTD_ps_disable &&
           c->appliedParams.maxBlockSize == BLOCK_CAP,
       "applied disabled-control mismatch");
  need(c->appliedParams.literalCompressionMode == ZSTD_ps_auto,
       "literal policy not auto");
  need(c->appliedParams.extSeqProdFunc == NULL &&
           c->seqCollector.collectSequences == 0,
       "unexpected sequence producer");
  need(c->pledgedSrcSizePlusOne == pledge + 1 && pledge != UINT64_MAX,
       "known pledge mismatch");
  need(c->dictID == 0 && c->dictContentSize == 0 && c->cdict == NULL &&
           c->blockState.matchState.dictMatchState == NULL &&
           c->blockState.matchState.loadedDictEnd == 0,
       "unexpected dictionary");
  {
    const ZSTD_MatchState_t *ms = &c->blockState.matchState;
    uintptr_t first_end =
        table_extent(&c->workspace, ms->hashTable,
                     times((size_t)1 << p->cParams.hashLog, sizeof(U32)), 0);
    need(ms->hashTable != NULL && (uintptr_t)ms->hashTable % _Alignof(U32) == 0,
         "initialized hash owner alignment");
    if (p->cParams.windowLog > 14) {
      uintptr_t second_end = table_extent(&c->workspace, ms->tagTable,
                                          (size_t)1 << p->cParams.hashLog, 1);
      unsigned row_log =
          p->cParams.searchLog < 4
              ? 4
              : (p->cParams.searchLog > 6 ? 6 : p->cParams.searchLog);
      need(ms->tagTable != NULL &&
               (uintptr_t)ms->tagTable != (uintptr_t)ms->hashTable &&
               (uintptr_t)ms->hashTable % 64 == 0 &&
               (uintptr_t)ms->tagTable % ((size_t)1 << row_log) == 0 &&
               ms->rowHashLog == p->cParams.hashLog - row_log,
           "row owner/alignment/geometry");
      need(first_end <= (uintptr_t)ms->tagTable ||
               second_end <= (uintptr_t)ms->hashTable,
           "overlapping position/tag owners");
      need(ms->hashSalt == UINT64_C(0x835892b4bb2cae74) &&
               ms->hashSaltEntropy == 0,
           "fresh row salt differs from prior independent capture");
      /* A zero-byte chain reservation may return a nonnull workspace cursor.
       * It is not an initialized owner and must never be dereferenced here. */
    } else {
      uintptr_t second_end =
          table_extent(&c->workspace, ms->chainTable,
                       times((size_t)1 << p->cParams.chainLog, sizeof(U32)), 0);
      need(ms->chainTable != NULL && ms->chainTable != ms->hashTable &&
               (uintptr_t)ms->chainTable % _Alignof(U32) == 0,
           "independent initialized chain owner");
      need(first_end <= (uintptr_t)ms->chainTable ||
               second_end <= (uintptr_t)ms->hashTable,
           "overlapping head/link owners");
    }
  }
  if (pledge < expected)
    expected = (size_t)pledge;
  if (expected == 0)
    expected = 1;
  if (expected > BLOCK_CAP)
    expected = BLOCK_CAP;
  need(c->blockSizeMax == expected && c->stage == ZSTDcs_init &&
           c->isFirstBlock,
       "begin lifecycle mismatch");
}
static ZSTD_CCtx *begin(const ZSTD_CCtx_params *p, uint64_t pledge) {
  ZSTD_CCtx *c = ZSTD_createCCtx();
  need(c != NULL, "CCtx allocation failed");
  checked(ZSTD_compressBegin_advanced_internal(c, NULL, 0, ZSTD_dct_auto,
                                               ZSTD_dtlm_fast, NULL, p, pledge),
          "begin normal frame");
  applied(c, p, pledge);
  return c;
}
static size_t header(const BYTE *p, size_t n, uint64_t pledge, unsigned w,
                     int emit) {
  ZSTD_frameHeader h;
  size_t result = checked(ZSTD_getFrameHeader(&h, p, n), "parse frame header");
  unsigned descriptor, single, fcs_code, fcs_bytes;
  size_t offset;
  uint64_t content = 0;
  unsigned i;
  need(result == 0 && n >= h.headerSize && h.frameType == ZSTD_frame,
       "incomplete/nonstandard header");
  need(h.frameContentSize == pledge && h.checksumFlag == 0 && h.dictID == 0,
       "observed header metadata mismatch");
  need(h.windowSize ==
           (pledge <= ((uint64_t)1 << w) ? pledge : ((uint64_t)1 << w)),
       "untruthful advertised window");
  need(n >= 5 && p[0] == 0x28 && p[1] == 0xb5 && p[2] == 0x2f && p[3] == 0xfd,
       "bad magic");
  descriptor = p[4];
  single = (descriptor >> 5) & 1;
  fcs_code = descriptor >> 6;
  need(single == (pledge <= ((uint64_t)1 << w)) && (descriptor & 31) == 0,
       "unexpected descriptor flags");
  offset = 5;
  if (!single) {
    need(offset < n, "missing window descriptor");
    need(p[offset] == (w - 10) * 8, "unexpected window descriptor");
    ++offset;
  }
  fcs_bytes =
      fcs_code == 0 ? single : (fcs_code == 1 ? 2 : (fcs_code == 2 ? 4 : 8));
  need(fcs_bytes <= n - offset, "truncated content-size field");
  for (i = 0; i < fcs_bytes; ++i)
    content |= (uint64_t)p[offset + i] << (8 * i);
  if (fcs_code == 1)
    content += 256;
  need(content == pledge && offset + fcs_bytes == h.headerSize,
       "independent content-size parse mismatch");
  if (emit) {
    printf("{\"type\":\"header\",\"case\":\"%s\",\"pledge\":%" PRIu64
           ",\"window_log\":%u,\"window\":%" PRIu64
           ",\"single_segment\":%u,\"fcs_bytes\":%u,\"bytes\":",
           case_name, pledge, w, (uint64_t)h.windowSize, single, fcs_bytes);
    hex(p, h.headerSize);
    fputs("}\n", stdout);
  }
  return h.headerSize;
}
static Block block(const BYTE *p, size_t n, size_t offset) {
  Block b;
  U32 word;
  need(offset <= n && n - offset >= 3, "missing block header");
  word = (U32)p[offset] | (U32)p[offset + 1] << 8 | (U32)p[offset + 2] << 16;
  b.last = word & 1;
  b.kind = (word >> 1) & 3;
  b.field = word >> 3;
  b.payload_offset = offset + 3;
  need(b.kind != 3 && b.field <= BLOCK_CAP, "invalid block kind/size");
  b.payload_size = b.kind == 1 ? 1 : b.field;
  need(b.payload_size <= n - b.payload_offset, "truncated block payload");
  return b;
}
/* Independent wire-repeat interpretation; no ZSTD_updateRep/collector call. */
static U32 resolve(U32 r[3], U32 off, size_t ll) {
  U32 a = r[0], b = r[1], c = r[2], d;
  need(off && a && b && c, "zero wire repeat input");
  if (off > 3) {
    d = off - 3;
    r[0] = d;
    r[1] = a;
    r[2] = b;
  } else if (ll && off == 1)
    d = a;
  else if ((ll && off == 2) || (!ll && off == 1)) {
    d = b;
    r[0] = b;
    r[1] = a;
  } else {
    if (!ll && off == 3) {
      need(a > 1, "invalid minus-one repeat");
      d = a - 1;
    } else
      d = c;
    r[0] = d;
    r[1] = a;
    r[2] = b;
  }
  need(d != 0, "zero decoded distance");
  return d;
}
static size_t sequence_count(const SeqStore_t *s) {
  uintptr_t start = (uintptr_t)s->sequencesStart, end = (uintptr_t)s->sequences;
  size_t span = times(s->maxNbSeq, sizeof(SeqDef));
  need(start <= UINTPTR_MAX - span && end >= start && end <= start + span &&
           (end - start) % sizeof(SeqDef) == 0,
       "sequence pointer extent");
  return (size_t)(end - start) / sizeof(SeqDef);
}
static size_t literal_count(const SeqStore_t *s) {
  uintptr_t start = (uintptr_t)s->litStart, end = (uintptr_t)s->lit;
  need(start <= UINTPTR_MAX - s->maxNbLit && end >= start &&
           end <= start + s->maxNbLit,
       "literal pointer extent");
  return (size_t)(end - start);
}
static void candidate(const SeqStore_t *s, const BYTE *source, BYTE *replay,
                      size_t start, size_t length, U32 wire[3],
                      unsigned window_log, U32 prefix_floor) {
  size_t count = sequence_count(s), literals = literal_count(s), i, j,
         cursor = start, lit_cursor = 0;
  need(prefix_floor >= 2, "invalid candidate prefix floor");
  need(count <= length / 3 + 1 && literals <= length, "candidate cardinality");
  printf("{\"fresh\":true,\"sequence_count\":%zu,\"literal_count\":%zu,"
         "\"sequences\":[",
         count, literals);
  for (i = 0; i < count; ++i) {
    ZSTD_SequenceLength lens = ZSTD_getSequenceLength(s, s->sequencesStart + i);
    size_t ll = lens.litLength, ml = lens.matchLength;
    U32 off = s->sequencesStart[i].offBase, d;
    need(ml >= 4 && (off == 1 || off > 3), "Greedy match/intent domain");
    need(ll <= start + length - cursor && ll <= literals - lit_cursor,
         "literal extent");
    memcpy(replay + cursor, s->litStart + lit_cursor, ll);
    cursor += ll;
    lit_cursor += ll;
    d = resolve(wire, off, ll);
    need(d <= cursor && d <= ((size_t)1 << window_log) &&
             cursor - d >= prefix_floor - 2 && ml <= start + length - cursor,
         "match extent/distance/window floor");
    for (j = 0; j < ml; ++j)
      replay[cursor + j] = replay[cursor + j - d];
    cursor += ml;
    if (i)
      putchar(',');
    printf("[%zu,%zu,%" PRIu32 ",%" PRIu32 ",%u,%u]", ll, ml, off, d,
           (unsigned)s->sequencesStart[i].litLength,
           (unsigned)s->sequencesStart[i].mlBase);
  }
  need(literals - lit_cursor == start + length - cursor,
       "trailing literals mismatch");
  memcpy(replay + cursor, s->litStart + lit_cursor, literals - lit_cursor);
  need(!memcmp(replay + start, source + start, length),
       "candidate reconstruction mismatch");
  need(s->longLengthType <= ZSTD_llt_matchLength &&
           (s->longLengthType == ZSTD_llt_none || s->longLengthPos < count),
       "long length marker bounds");
  printf("],\"long_length_type\":%u,\"long_length_position\":%u,"
         "\"trailing\":%zu,\"literal_bytes\":",
         (unsigned)s->longLengthType,
         s->longLengthType == ZSTD_llt_none ? 0u : s->longLengthPos,
         literals - lit_cursor);
  hex(s->litStart, literals);
  fputs(",\"semantic_repeats\":", stdout);
  reps(wire);
  putchar('}');
}
static void decode(const BYTE *frame, size_t size, const BYTE *source,
                   size_t n) {
  Buffer out = buffer(n ? n : 1);
  size_t result;
  need(checked(ZSTD_findFrameCompressedSize(frame, size), "frame extent") ==
           size,
       "multiple/trailing frames");
  need(ZSTD_getFrameContentSize(frame, size) == n,
       "complete frame content size");
  result = checked(ZSTD_decompress(out.data, n, frame, size),
                   "ordinary full decode");
  need(result == n && !memcmp(out.data, source, n),
       "full decoded bytes differ");
  drop(&out);
}
static void window(const ZSTD_MatchState_t *ms, const BYTE *source,
                   size_t consumed) {
  need(ms->window.base == source - 2 && ms->window.nextSrc == source + consumed,
       "noncontiguous source/window");
  need(ms->window.nbOverflowCorrections == 0 && ms->loadedDictEnd == 0 &&
           ms->dictMatchState == NULL &&
           ZSTD_matchState_dictMode(ms) == ZSTD_noDict,
       "unexpected dictionary/index correction");
  printf("{\"base_index_bias\":2,\"low_limit\":%" PRIu32
         ",\"dict_limit\":%" PRIu32 ",\"next_to_update\":%" PRIu32
         ",\"consumed\":%zu}",
         ms->window.lowLimit, ms->window.dictLimit, ms->nextToUpdate, consumed);
}
/* Width-specific memcpy loads avoid aliasing assumptions for byte copies.
 * Buffer/context extents and natural alignment are verified before use. */
static U32 cell(const BYTE *values, size_t i, unsigned bytes) {
  U32 value;
  if (bytes == 1)
    return values[i];
  memcpy(&value, values + i * 4, sizeof(value));
  return value;
}
static void table_changes(const BYTE *before, const BYTE *after, size_t n,
                          unsigned log, unsigned bytes) {
  size_t i;
  int first = 1;
  uint64_t before_hash, after_hash;
  need(log >= 6 && log <= 19 && (bytes == 1 || bytes == 4) &&
           n == ((size_t)1 << log),
       "table observation dimensions");
  before_hash =
      bytes == 1 ? hash_bytes(before, n) : hash_table((const U32 *)before, n);
  after_hash =
      bytes == 1 ? hash_bytes(after, n) : hash_table((const U32 *)after, n);
  printf("{\"log\":%u,\"element_bytes\":%u,\"entries\":%zu,"
         "\"before_fnv64\":\"%016" PRIx64 "\",\"after_fnv64\":\"%016" PRIx64
         "\",\"changed\":[",
         log, bytes, n, before_hash, after_hash);
  for (i = 0; i < n; ++i) {
    U32 old = cell(before, i, bytes), value = cell(after, i, bytes);
    if (old != value) {
      if (!first)
        putchar(',');
      first = 0;
      printf("[%zu,%" PRIu32 ",%" PRIu32 "]", i, old, value);
    }
  }
  fputs("]}", stdout);
}
static size_t source_position(U32 value, const ZSTD_MatchState_t *ms,
                              size_t consumed, unsigned width) {
  size_t position;
  need(value >= 2 && value < ms->nextToUpdate,
       "invalid/future raw source position");
  position = value - 2;
  need(position <= consumed && width <= consumed - position,
       "key outside consumed logical source");
  return position;
}
static void validate_tables(const ZSTD_MatchState_t *ms, const BYTE *source,
                            size_t consumed, int row) {
  size_t i, n = (size_t)1 << ms->cParams.hashLog;
  unsigned width = ms->cParams.minMatch < 4
                       ? 4
                       : (ms->cParams.minMatch > 6 ? 6 : ms->cParams.minMatch);
  if (row) {
    unsigned row_log =
        ms->cParams.searchLog < 4
            ? 4
            : (ms->cParams.searchLog > 6 ? 6 : ms->cParams.searchLog);
    size_t row_width = (size_t)1 << row_log;
    for (i = 0; i < n; i += row_width) {
      size_t j;
      unsigned head = ms->tagTable[i];
      int occupied = 0;
      need(ms->hashTable[i] == 0 && head < row_width,
           "reserved row/head bound");
      for (j = 1; j < row_width; ++j) {
        U32 value = ms->hashTable[i + j];
        if (value) {
          size_t position = source_position(value, ms, consumed, width);
          size_t hash = ZSTD_hashPtrSalted(
              source + position, ms->rowHashLog + 8, width, ms->hashSalt);
          need(((hash >> 8) << row_log) == i &&
                   (hash & 255) == ms->tagTable[i + j],
               "row/tag hash membership");
          occupied = 1;
        } else
          need(ms->tagTable[i + j] == 0, "unused row position has a tag");
      }
      need((head != 0) == occupied && (!head || ms->hashTable[i + head] != 0),
           "row occupancy/head mismatch");
    }
  } else {
    for (i = 0; i < n; ++i) {
      if (ms->hashTable[i]) {
        size_t position =
            source_position(ms->hashTable[i], ms, consumed, width);
        need(ZSTD_hashPtr(source + position, ms->cParams.hashLog, width) == i,
             "chain head hash membership");
      }
    }
    for (i = 0; i < ((size_t)1 << ms->cParams.chainLog); ++i)
      if (ms->chainTable[i])
        (void)source_position(ms->chainTable[i], ms, consumed, width);
  }
}
static size_t schedule(const char *text, size_t sizes[CALL_CAP]) {
  const char *p = text;
  size_t n = 0;
  if (!strcmp(text, "-"))
    return 0;
  while (*p) {
    char *end;
    unsigned long value;
    need(n < CALL_CAP && *p >= '0' && *p <= '9', "bad/oversize schedule");
    errno = 0;
    value = strtoul(p, &end, 10);
    need(!errno && (end[0] == ',' || end[0] == 0) && value <= BLOCK_CAP,
         "bad block size");
    sizes[n++] = (size_t)value;
    p = end;
    if (*p == ',') {
      ++p;
      need(*p != 0, "trailing schedule comma");
    }
  }
  return n;
}
static void whole_compare(const ZSTD_CCtx_params *p, const BYTE *source,
                          size_t n, const BYTE *scheduled,
                          size_t scheduled_size, const char *path,
                          size_t positive_calls) {
  size_t capacity = checked(ZSTD_compressBound(n), "whole frame bound"), a, b,
         blocks = 0, total;
  Buffer out = buffer(capacity);
  ZSTD_CCtx *c = ZSTD_createCCtx();
  need(c != NULL, "whole CCtx allocation");
  total = checked(ZSTD_compress_advanced_internal(c, out.data, out.size, source,
                                                  n, NULL, 0, p),
                  "whole normal compression");
  need(same_params(&c->appliedParams.cParams, &p->cParams),
       "whole applied scalar mismatch");
  decode(out.data, total, source, n);
  a = header(scheduled, scheduled_size, n, p->cParams.windowLog, 0);
  b = header(out.data, total, n, p->cParams.windowLog, 0);
  while (blocks < positive_calls) {
    Block x = block(scheduled, scheduled_size, a),
          y = block(out.data, total, b);
    need(!x.last && x.kind == y.kind && x.field == y.field &&
             x.payload_size == y.payload_size,
         "whole/scheduled block mismatch");
    need(!memcmp(scheduled + x.payload_offset, out.data + y.payload_offset,
                 x.payload_size),
         "whole/scheduled payload mismatch");
    need(y.last == (blocks + 1 == positive_calls),
         "whole final-block boundary");
    a = x.payload_offset + x.payload_size;
    b = y.payload_offset + y.payload_size;
    ++blocks;
  }
  {
    Block end = block(scheduled, scheduled_size, a);
    need(end.last && end.kind == 0 && end.field == 0 &&
             end.payload_offset == scheduled_size,
         "scheduled terminal empty block");
  }
  need(b == total, "whole output not exhausted");
  write_file(path, out.data, total);
  printf("{\"type\":\"whole_compare\",\"case\":\"%s\",\"blocks\":%zu,\"decoded_"
         "bytes\":%zu,\"frame_bytes\":%zu,\"frame_fnv64\":\"%016" PRIx64
         "\"}\n",
         case_name, blocks, n, total, hash_bytes(out.data, total));
  checked(ZSTD_freeCCtx(c), "free whole context");
  drop(&out);
}
static void run_frame(const char *input, const char *schedule_text,
                      const char *output, const char *whole_path, unsigned w,
                      unsigned h, unsigned chain, unsigned search, unsigned m,
                      unsigned t) {
  Buffer arena, copy, scratch, frame, replay, before_table, before_second;
  BYTE *source;
  size_t n, calls, sizes[CALL_CAP], i, sum = 0, positive = 0, consumed = 0,
                                       written = 0;
  size_t candidate_generation = 0, last_count = 0, last_literals = 0;
  uint64_t last_store_hash = 0;
  unsigned header_seen = 0, initialized[2] = {1, 0};
  U32 semantic[3] = {1, 4, 8};
  int row = w > 14;
  ZSTD_CCtx_params p = parameters(w, h, chain, search, m, t);
  ZSTD_CCtx *c;
  ZSTD_compressedBlockState_t *owners[2];
  n = read_file(input, &arena);
  source = arena.data + LEADING_BYTES;
  copy = buffer(arena.size);
  memcpy(copy.data, arena.data, arena.size);
  calls = schedule(schedule_text, sizes);
  for (i = 0; i < calls; ++i) {
    sum = plus(sum, sizes[i]);
    if (sizes[i])
      ++positive;
  }
  need(sum == n && (n == 0 || positive > 0), "schedule/source mismatch");
  c = begin(&p, n);
  owners[0] = c->blockState.prevCBlock;
  owners[1] = c->blockState.nextCBlock;
  need(owners[0] != owners[1], "aliased state owners");
  scratch = buffer(SCRATCH_BYTES);
  frame = buffer(plus(plus(n, times(positive + 1, 3)),
                      ZSTD_FRAMEHEADERSIZE_MAX + ZSTD_FRAMECHECKSUMSIZE));
  replay = buffer(n ? n : 1);
  before_table = buffer(times((size_t)1 << h, sizeof(U32)));
  before_second =
      buffer(row ? (size_t)1 << h : times((size_t)1 << chain, sizeof(U32)));
  need((uintptr_t)before_table.data % _Alignof(U32) == 0 &&
           (uintptr_t)before_second.data % _Alignof(U32) == 0,
       "table-copy alignment");
  {
    size_t j;
    const ZSTD_MatchState_t *ms = &c->blockState.matchState;
    for (j = 0; j < ((size_t)1 << h); ++j)
      need(ms->hashTable[j] == 0, "initial hash table");
    if (row) {
      for (j = 0; j < ((size_t)1 << h); ++j)
        need(ms->tagTable[j] == 0, "initial tags");
    } else {
      for (j = 0; j < ((size_t)1 << chain); ++j)
        need(ms->chainTable[j] == 0, "initial links");
    }
  }
  printf("{\"type\":\"begin\",\"case\":\"%s\",\"source_bytes\":%zu,\"source_"
         "fnv64\":\"%016" PRIx64 "\",\"applied\":",
         case_name, n, hash_bytes(source, n));
  print_params(&c->appliedParams.cParams);
  printf(
      ",\"block_limit\":%zu,\"pending_initialized\":false,\"initial_state\":",
      c->blockSizeMax);
  {
    State state = snapshot(owners[0]);
    print_state(&state);
  }
  printf(",\"branch\":\"%s\",\"sampling\":", row ? "row" : "chain");
  {
    Sampling initial = sampling(&c->blockState.matchState, row);
    print_sampling(&initial);
  }
  fputs("}\n", stdout);
  for (i = 0; i < calls; ++i) {
    size_t length = sizes[i], result, offset = 0;
    unsigned prev = owner(c->blockState.prevCBlock, owners[0], owners[1]),
             pending = 1 - prev;
    State before = snapshot(owners[prev]), candidate_state, after;
    Sampling sampling_before = sampling(&c->blockState.matchState, row),
             sampling_after;
    U32 trial[3];
    Block b;
    int fresh = length >= 7, first = c->isFirstBlock,
        pending_was_initialized = (int)initialized[pending];
    size_t maxseq = length / 3,
           bound = plus(plus(length, 5 + 4 + 3 * FSE_NCOUNTBOUND + 32),
                        plus(times(maxseq, 89), 34) / 8);
    need(length <= c->blockSizeMax && bound < SCRATCH_BYTES,
         "block/scratch capacity contract");
    memcpy(before_table.data, c->blockState.matchState.hashTable,
           before_table.size);
    memcpy(before_second.data,
           row ? (const void *)c->blockState.matchState.tagTable
               : (const void *)c->blockState.matchState.chainTable,
           before_second.size);
    memcpy(trial, semantic, sizeof(trial));
    result = checked(ZSTD_compressContinue(c, scratch.data, scratch.size,
                                           source + consumed, length),
                     "normal Continue");
    sampling_after = sampling(&c->blockState.matchState, row);
    if (!header_seen) {
      offset = header(scratch.data, result, n, w, 1);
      header_seen = 1;
    }
    if (!length) {
      need(result == offset && c->isFirstBlock == first &&
               c->blockState.prevCBlock == owners[prev],
           "zero Continue changed block publication");
      need(!memcmp(before_table.data, c->blockState.matchState.hashTable,
                   before_table.size) &&
               !memcmp(before_second.data,
                       row ? (const void *)c->blockState.matchState.tagTable
                           : (const void *)c->blockState.matchState.chainTable,
                       before_second.size) &&
               same_sampling(&sampling_before, &sampling_after),
           "zero Continue changed sampled tables");
      printf("{\"type\":\"zero_continue\",\"case\":\"%s\",\"call\":%zu,"
             "\"produced\":%zu,\"candidate_fresh\":false,\"sampling\":",
             case_name, i, result);
      print_sampling(&sampling_after);
      fputs(",\"first_table\":", stdout);
      table_changes(before_table.data,
                    (const BYTE *)c->blockState.matchState.hashTable,
                    (size_t)1 << h, h, 4);
      fputs(",\"second_table\":", stdout);
      table_changes(before_second.data,
                    row ? c->blockState.matchState.tagTable
                        : (const BYTE *)c->blockState.matchState.chainTable,
                    (size_t)1 << (row ? h : chain), row ? h : chain,
                    row ? 1 : 4);
      validate_tables(&c->blockState.matchState, source, consumed, row);
      fputs("}\n", stdout);
    } else {
      b = block(scratch.data, result, offset);
      need(!b.last && b.payload_offset + b.payload_size == result,
           "Continue must emit exactly one nonfinal block");
      need(b.payload_size <= length && (b.kind == 2 || b.field == length),
           "block regenerated size/payload contract");
      if (first)
        need(b.kind != 1, "first block emitted RLE");
      need((c->blockState.prevCBlock == owners[pending]) == (b.kind == 2),
           "publication pointer swap mismatch");
      need(
          c->blockState.nextCBlock ==
              owners[1 - owner(c->blockState.prevCBlock, owners[0], owners[1])],
          "next owner mismatch");
      if (fresh) {
        initialized[pending] = 1;
        candidate_state = snapshot(owners[pending]);
        ++candidate_generation;
      }
      after = snapshot(c->blockState.prevCBlock);
      printf("{\"type\":\"block\",\"case\":\"%s\",\"call\":%zu,\"source_"
             "start\":%zu,\"source_bytes\":%zu,\"kind\":%u,\"payload_bytes\":%"
             "zu,\"payload_fnv64\":\"%016" PRIx64
             "\",\"first\":%d,\"prev_owner_before\":%u,\"prev_owner_after\":%u,"
             "\"pending_was_initialized\":%s,\"before\":",
             case_name, i, consumed, length, b.kind, b.payload_size,
             hash_bytes(scratch.data + b.payload_offset, b.payload_size), first,
             prev, owner(c->blockState.prevCBlock, owners[0], owners[1]),
             pending_was_initialized ? "true" : "false");
      print_state(&before);
      fputs(",\"semantic_before\":", stdout);
      reps(semantic);
      fputs(",\"candidate\":", stdout);
      if (fresh) {
        candidate(&c->seqStore, source, replay.data, consumed, length, trial, w,
                  c->blockState.matchState.window.dictLimit);
        need(candidate_state.rep[0] == trial[0] &&
                 candidate_state.rep[1] == trial[1],
             "Greedy first-two candidate repeats mismatch");
        last_count = sequence_count(&c->seqStore);
        last_literals = literal_count(&c->seqStore);
        last_store_hash = hash_bytes(c->seqStore.litStart, last_literals);
      } else {
        printf("{\"fresh\":false,\"previous_generation\":%zu}",
               candidate_generation);
        if (candidate_generation)
          need(sequence_count(&c->seqStore) == last_count &&
                   literal_count(&c->seqStore) == last_literals &&
                   hash_bytes(c->seqStore.litStart, last_literals) ==
                       last_store_hash,
               "tiny path changed stale store");
        memcpy(replay.data + consumed, source + consumed, length);
        need(b.kind == 0, "tiny bypass emitted nonraw block");
        /* The frame loop clamps nextToUpdate to lowLimit before the tiny
         * sequence-store early return. It does not perform the later 384/192
         * adjustment or change cache/salt/skipping on this path. */
        {
          Sampling expected_sampling = sampling_before;
          U32 floor = c->blockState.matchState.window.lowLimit;
          if (expected_sampling.next < floor)
            expected_sampling.next = floor;
          need(!memcmp(before_table.data, c->blockState.matchState.hashTable,
                       before_table.size) &&
                   !memcmp(
                       before_second.data,
                       row ? (const void *)c->blockState.matchState.tagTable
                           : (const void *)c->blockState.matchState.chainTable,
                       before_second.size) &&
                   same_sampling(&expected_sampling, &sampling_after),
               "tiny bypass changed sampling beyond the window-floor clamp");
        }
      }
      if (b.kind == 2) {
        need(fresh, "compressed tiny block");
        memcpy(semantic, trial, sizeof(semantic));
      } else {
        need(!memcmp(after.rep, before.rep, sizeof(before.rep)),
             "raw/RLE published raw repeats");
        need(after.huf == before.huf && after.ll == before.ll &&
                 after.ml == before.ml &&
                 (after.of == before.of || (before.of == FSE_repeat_valid &&
                                            after.of == FSE_repeat_check)),
             "unexpected committed tag mutation");
        need(after.huf_log == before.huf_log &&
                 after.huf_max == before.huf_max &&
                 !memcmp(after.widths, before.widths, sizeof(before.widths)),
             "raw/RLE changed committed HUF widths");
      }
      need(after.rep[0] == semantic[0] && after.rep[1] == semantic[1],
           "committed first-two semantic repeats mismatch");
      fputs(",\"candidate_state\":", stdout);
      if (fresh)
        print_state(&candidate_state);
      else
        fputs("null", stdout);
      fputs(",\"after\":", stdout);
      print_state(&after);
      fputs(",\"semantic_after\":", stdout);
      reps(semantic);
      printf(",\"changed_candidate_first_two\":%s,\"window\":",
             fresh && (candidate_state.rep[0] != before.rep[0] ||
                       candidate_state.rep[1] != before.rep[1])
                 ? "true"
                 : "false");
      consumed += length;
      window(&c->blockState.matchState, source, consumed);
      fputs(",\"sampling_before\":", stdout);
      print_sampling(&sampling_before);
      fputs(",\"sampling_after\":", stdout);
      print_sampling(&sampling_after);
      need(sampling_after.salt == sampling_before.salt,
           "salt changed during one frame");
      fputs(",\"first_table\":", stdout);
      table_changes(before_table.data,
                    (const BYTE *)c->blockState.matchState.hashTable,
                    (size_t)1 << h, h, 4);
      fputs(",\"second_table\":", stdout);
      table_changes(before_second.data,
                    row ? c->blockState.matchState.tagTable
                        : (const BYTE *)c->blockState.matchState.chainTable,
                    (size_t)1 << (row ? h : chain), row ? h : chain,
                    row ? 1 : 4);
      validate_tables(&c->blockState.matchState, source, consumed, row);
      fputs("}\n", stdout);
    }
    need(result <= frame.size - written, "full frame bound before append");
    memcpy(frame.data + written, scratch.data, result);
    written += result;
    guards(&scratch);
    need(!memcmp(arena.data, copy.data, arena.size), "source/padding changed");
  }
  {
    size_t result = checked(
               ZSTD_compressEnd(c, scratch.data, scratch.size, source + n, 0),
               "normal End"),
           offset = 0;
    Block last;
    if (!header_seen) {
      offset = header(scratch.data, result, n, w, 1);
      header_seen = 1;
    }
    last = block(scratch.data, result, offset);
    need(last.last && last.kind == 0 && last.field == 0 &&
             last.payload_offset == result,
         "missing/excess terminal empty block");
    need(result <= frame.size - written, "final append bound");
    memcpy(frame.data + written, scratch.data, result);
    written += result;
    need(c->stage == ZSTDcs_created && c->consumedSrcSize == n && consumed == n,
         "finish/consumption mismatch");
  }
  need(!memcmp(replay.data, source, n), "incremental replay not complete");
  decode(frame.data, written, source, n);
  write_file(output, frame.data, written);
  printf("{\"type\":\"complete\",\"case\":\"%s\",\"calls\":%zu,\"blocks\":%zu,"
         "\"candidate_generations\":%zu,\"decoded_bytes\":%zu,\"frame_bytes\":%"
         "zu,\"frame_fnv64\":\"%016" PRIx64 "\",\"terminal_empty_raw\":true}\n",
         case_name, calls, positive, candidate_generation, n, written,
         hash_bytes(frame.data, written));
  if (strcmp(whole_path, "-"))
    whole_compare(&p, source, n, frame.data, written, whole_path, positive);
  need(!memcmp(arena.data, copy.data, arena.size),
       "final source/padding changed");
  checked(ZSTD_freeCCtx(c), "free normal context");
  drop(&before_second);
  drop(&before_table);
  drop(&replay);
  drop(&frame);
  drop(&scratch);
  drop(&copy);
  drop(&arena);
}
int main(int argc, char **argv) {
  need(ZSTD_versionNumber() == 10507 && CHAR_BIT == 8 && sizeof(U32) == 4 &&
           sizeof(int) == 4,
       "reference version/width mismatch");
  if (argc == 2 && !strcmp(argv[1], "metadata")) {
    printf("{\"type\":\"metadata\",\"revision\":"
           "\"f8745da6ff1ad1e7bab384bd1f9d742439278e99\",\"strategy\":3,"
           "\"schema_version\":2,"
           "\"pointer_bits\":%zu,\"int_bits\":%zu,\"u32_bytes\":%zu,\"source_"
           "cap\":%u,\"block_cap\":%u,\"call_cap\":%u,\"leading_bytes\":%u,"
           "\"suffix_bytes\":%u,\"guard_bytes\":%u,\"scratch_bytes\":%u}\n",
           sizeof(void *) * CHAR_BIT, sizeof(int) * CHAR_BIT, sizeof(U32),
           SOURCE_CAP, BLOCK_CAP, CALL_CAP, LEADING_BYTES, SUFFIX_BYTES,
           GUARD_BYTES, SCRATCH_BYTES);
    need(!ferror(stdout), "metadata output error");
    return 0;
  }
  if (argc == 4 && !strcmp(argv[1], "header")) {
    uint64_t pledge = number(argv[2]), wv = number(argv[3]);
    ZSTD_CCtx_params p;
    ZSTD_CCtx *c;
    Buffer out = buffer(128);
    BYTE dummy = 0;
    size_t n;
    need(pledge < UINT64_MAX && wv <= UINT_MAX, "header admission");
    p = parameters((unsigned)wv, 14, 12, 1, 4, 0);
    c = begin(&p, pledge);
    case_name = "header_only";
    n = checked(ZSTD_compressContinue(c, out.data, out.size, &dummy, 0),
                "header-only zero Continue");
    need(header(out.data, n, pledge, (unsigned)wv, 1) == n &&
             c->consumedSrcSize == 0 && c->isFirstBlock &&
             c->stage == ZSTDcs_ongoing,
         "header-only lifecycle");
    printf("{\"type\":\"partial\",\"completed_frame\":false,\"decoded\":false,"
           "\"pointer_bits\":%zu,\"int_bits\":%zu}\n",
           sizeof(void *) * CHAR_BIT, sizeof(int) * CHAR_BIT);
    checked(ZSTD_freeCCtx(c), "free partial context");
    drop(&out);
    return 0;
  }
  need(argc == 13 && !strcmp(argv[1], "frame"),
       "usage: frame CASE INPUT SCHEDULE OUTPUT WHOLE_OR_DASH W H C S M T | "
       "header PLEDGE W");
  case_name = argv[2];
  need(strlen(case_name) > 0 && strlen(case_name) <= 63, "case ID length");
  {
    const char *q = case_name;
    while (*q) {
      need((*q >= 'a' && *q <= 'z') || (*q >= '0' && *q <= '9') || *q == '_',
           "case ID charset");
      ++q;
    }
  }
  {
    uint64_t w = number(argv[7]), h = number(argv[8]), chain = number(argv[9]),
             search = number(argv[10]), m = number(argv[11]),
             t = number(argv[12]);
    need(w <= UINT_MAX && h <= UINT_MAX && chain <= UINT_MAX &&
             search <= UINT_MAX && m <= UINT_MAX && t <= UINT_MAX,
         "parameter narrowing");
    run_frame(argv[3], argv[4], argv[5], argv[6], (unsigned)w, (unsigned)h,
              (unsigned)chain, (unsigned)search, (unsigned)m, (unsigned)t);
  }
  need(!ferror(stdout), "record output error");
  return 0;
}
