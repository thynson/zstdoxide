/* Original facebook/zstd v1.5.7 tests/zstreamtest.c:171-226, :1497-1510,
 * :1551-1599 sequence campaign. The unchanged tests/seqgen.c is compiled only
 * into this external test oracle, never into the Rust implementation.
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 * Copyright (c) zstdoxide contributors. BSD-3-Clause; see root LICENSE.zstd.
 *
 * baseline DICTIONARY FRAME: execute the original source/streaming schedule,
 * including inherited workers=2/jobSize=4MiB, and write the resulting frame.
 * decode DICTIONARY FRAME: independently decode a Rust frame using 1024-byte
 * input/output buffers and compare every byte against a second SEQ stream.
 * Generation, compression-output and decompression-output buffers are 1024
 * bytes, as in SEQ_generateRoundTrip/SEQ_roundTrip. Only the dictionary is
 * loaded whole; frame and regenerated data are never accumulated in memory.
 */
#define ZSTD_STATIC_LINKING_ONLY
#define ZSTD_DISABLE_DEPRECATE_WARNINGS
#define XXH_STATIC_LINKING_ONLY
#include "zstd.h"
#include "seqgen.h"
#include "xxhash.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define CHUNK 1024U
#define GUARD 8U
#define REQUEST_COUNT 971U
#define SOURCE_BYTES UINT64_C(34639743)
#define SOURCE_DIGEST UINT64_C(0xd6a9522c4b14b2a5)
#define FINAL_SEED UINT32_C(0x0f900e80)
#define INITIAL_SEED UINT32_C(0x87654321)
#define DICTIONARY_CAPACITY (48U * 1024U)
#define FRAME_LIMIT (64U * 1024U * 1024U)
#define CALL_LIMIT 1000000U

static void require(int condition, const char* message) {
    if (!condition) { fprintf(stderr, "%s\n", message); exit(2); }
}

static void codec_ok(size_t result) {
    require(!ZSTD_isError(result), ZSTD_getErrorName(result));
}

static void guard(const unsigned char* buffer) {
    unsigned index;
    for (index = CHUNK; index < CHUNK + GUARD; ++index)
        require(buffer[index] == 0xa5, "oracle output guard changed");
}

static size_t load_dictionary(const char* path, unsigned char* bytes) {
    FILE* file = fopen(path, "rb");
    size_t size;
    require(file != NULL, "cannot open dictionary");
    size = fread(bytes, 1, DICTIONARY_CAPACITY, file);
    require(!ferror(file) && fgetc(file) == EOF, "invalid or oversized dictionary");
    require(fclose(file) == 0, "cannot close dictionary");
    require(size == DICTIONARY_CAPACITY, "wrong original dictionary size");
    require(ZSTD_getDictID_fromDict(bytes, size) == 2004100296U, "wrong dictionary ID");
    return size;
}

static void request(unsigned index, SEQ_gen_type* type, unsigned* value) {
    require(index < REQUEST_COUNT, "request index out of bounds");
    if (index < 900) {
        *type = (SEQ_gen_type)(index % 3);
        *value = index / 3;
    } else if (index < 954) {
        unsigned const relative = index - 900;
        unsigned const base = 1U << (8 + relative / 6);
        *type = (SEQ_gen_type)((relative % 6) / 2);
        *value = base + ((relative % 2) ? base / 4 : 0);
    } else {
        *type = SEQ_gen_of;
        *value = (1U << (8 + index - 954)) - 1;
    }
}

typedef struct {
    SEQ_stream stream;
    unsigned request_index;
    size_t remaining, position, size;
    unsigned char bytes[CHUNK];
    uint64_t compared;
} Expected;

static Expected expected_new(void) {
    Expected expected;
    memset(&expected, 0, sizeof(expected));
    expected.stream = SEQ_initStream(INITIAL_SEED);
    return expected;
}

static int refill(Expected* expected) {
    while (expected->position == expected->size && expected->request_index < REQUEST_COUNT) {
        SEQ_gen_type type;
        unsigned value;
        SEQ_outBuffer out = {expected->bytes, CHUNK, 0};
        request(expected->request_index, &type, &value);
        expected->remaining = SEQ_gen(&expected->stream, type, value, &out);
        expected->position = 0;
        expected->size = out.pos;
        if (!expected->remaining) ++expected->request_index;
        if (out.pos) return 1;
    }
    return expected->position != expected->size;
}

static void compare(Expected* expected, const unsigned char* bytes, size_t size) {
    size_t position = 0;
    while (position < size) {
        size_t length;
        require(refill(expected), "decoded output exceeds original schedule");
        length = expected->size - expected->position;
        if (length > size - position) length = size - position;
        require(memcmp(expected->bytes + expected->position, bytes + position, length) == 0,
                "decoded bytes differ from original sequence source");
        expected->position += length;
        expected->compared += length;
        position += length;
    }
}

static void expected_finish(Expected* expected) {
    require(!refill(expected), "decoded output ended before original source");
    require(expected->request_index == REQUEST_COUNT && expected->compared == SOURCE_BYTES,
            "decoded schedule length differs");
    require(SEQ_digest(&expected->stream) == SOURCE_DIGEST, "expected source digest differs");
    require(expected->stream.seed == FINAL_SEED, "expected final random seed differs");
}

typedef struct {
    ZSTD_CCtx* compressor;
    ZSTD_DCtx* decompressor;
    FILE* frame;
    Expected expected;
    XXH64_state_t decoded_hash, encoded_hash;
    uint64_t generated, encoded, decoded;
    unsigned generation_calls, compression_calls, decompression_calls, finished;
    unsigned char header[18];
    size_t header_size;
} Campaign;

static void decode_chunk(Campaign* campaign, const unsigned char* bytes, size_t size, int ending) {
    ZSTD_inBuffer in = {bytes, size, 0};
    while (in.pos < in.size || ending) {
        unsigned char scratch[CHUNK + GUARD];
        ZSTD_outBuffer out = {scratch, CHUNK, 0};
        size_t previous = in.pos, result;
        memset(scratch, 0xa5, sizeof(scratch));
        require(++campaign->decompression_calls < CALL_LIMIT, "decoder call bound exceeded");
        result = ZSTD_decompressStream(campaign->decompressor, &out, &in);
        codec_ok(result); guard(scratch);
        compare(&campaign->expected, scratch, out.pos);
        XXH64_update(&campaign->decoded_hash, scratch, out.pos);
        campaign->decoded += out.pos;
        if (result == 0) {
            require(in.pos == in.size, "trailing frame bytes");
            require(++campaign->finished == 1, "more than one frame");
            break;
        }
        require(in.pos != previous || out.pos != 0, "decoder made no progress");
    }
}

static void round_trip(Campaign* campaign, const unsigned char* bytes, size_t size,
                       ZSTD_EndDirective directive) {
    ZSTD_inBuffer in = {bytes, size, 0};
    size_t result;
    do {
        unsigned char scratch[CHUNK + GUARD];
        ZSTD_outBuffer out = {scratch, CHUNK, 0};
        size_t copy;
        memset(scratch, 0xa5, sizeof(scratch));
        require(++campaign->compression_calls < CALL_LIMIT, "compressor call bound exceeded");
        result = ZSTD_compressStream2(campaign->compressor, &out, &in, directive);
        codec_ok(result); guard(scratch);
        require(fwrite(scratch, 1, out.pos, campaign->frame) == out.pos, "cannot write frame");
        XXH64_update(&campaign->encoded_hash, scratch, out.pos);
        campaign->encoded += out.pos;
        require(campaign->encoded <= FRAME_LIMIT, "frame limit exceeded");
        copy = sizeof(campaign->header) - campaign->header_size;
        if (copy > out.pos) copy = out.pos;
        memcpy(campaign->header + campaign->header_size, scratch, copy);
        campaign->header_size += copy;
        decode_chunk(campaign, scratch, out.pos, directive == ZSTD_e_end && result == 0);
    } while (in.pos < in.size || (directive != ZSTD_e_continue && result != 0));
    require(in.pos == size, "source was not consumed completely");
}

static void baseline(const char* dictionary_path, const char* frame_path) {
    unsigned char* dictionary = malloc(DICTIONARY_CAPACITY);
    size_t dictionary_size;
    ZSTD_compressionParameters parameters = ZSTD_getCParams(3, 1ULL << 24, 1024);
    ZSTD_CDict* prepared;
    ZSTD_DDict* decoder_dictionary;
    SEQ_stream source = SEQ_initStream(INITIAL_SEED);
    ZSTD_frameHeader header;
    Campaign campaign;
    unsigned index;
    int workers, job_size, requested_window;
    require(dictionary != NULL, "dictionary allocation failed");
    dictionary_size = load_dictionary(dictionary_path, dictionary);
    memset(&campaign, 0, sizeof(campaign));
    campaign.expected = expected_new();
    campaign.compressor = ZSTD_createCCtx(); campaign.decompressor = ZSTD_createDCtx();
    campaign.frame = fopen(frame_path, "wb");
    require(campaign.compressor && campaign.decompressor && campaign.frame, "campaign setup failed");
    XXH64_reset(&campaign.decoded_hash, 0); XXH64_reset(&campaign.encoded_hash, 0);
    parameters.windowLog = 24;
    prepared = ZSTD_createCDict_advanced(dictionary, dictionary_size, ZSTD_dlm_byRef,
                                        ZSTD_dct_fullDict, parameters, ZSTD_defaultCMem);
    decoder_dictionary = ZSTD_createDDict(dictionary, dictionary_size);
    require(prepared && decoder_dictionary, "prepared dictionary creation failed");
    /* :1444 installs all explicit level-5 cParams and frame parameters. The
     * later level-3 request does not erase those explicit values. :1497 adds
     * worker/job controls; :1551 resets only the session. */
    {
        ZSTD_parameters inherited = ZSTD_getParams(5, 0, 0);
        inherited.fParams.contentSizeFlag = 1;
        codec_ok(ZSTD_initCStream_advanced(campaign.compressor, NULL, 0, inherited, 0));
        codec_ok(ZSTD_CCtx_reset(campaign.compressor, ZSTD_reset_session_only));
        codec_ok(ZSTD_CCtx_setPledgedSrcSize(campaign.compressor, ZSTD_CONTENTSIZE_UNKNOWN));
        codec_ok(ZSTD_CCtx_setParameter(campaign.compressor, ZSTD_c_compressionLevel, 3));
    }
    {
        size_t const result = ZSTD_CCtx_setParameter(campaign.compressor, ZSTD_c_nbWorkers, 2);
        require(!ZSTD_isError(result), "original baseline requires a multithread-enabled libzstd.a");
    }
    codec_ok(ZSTD_CCtx_setParameter(campaign.compressor, ZSTD_c_jobSize, 4 * 1024 * 1024));
    codec_ok(ZSTD_CCtx_reset(campaign.compressor, ZSTD_reset_session_only));
    codec_ok(ZSTD_resetDStream(campaign.decompressor));
    codec_ok(ZSTD_CCtx_refCDict(campaign.compressor, prepared));
    codec_ok(ZSTD_initDStream_usingDDict(campaign.decompressor, decoder_dictionary));
    codec_ok(ZSTD_DCtx_setParameter(campaign.decompressor, ZSTD_d_windowLogMax, 24));
    codec_ok(ZSTD_CCtx_getParameter(campaign.compressor, ZSTD_c_nbWorkers, &workers));
    codec_ok(ZSTD_CCtx_getParameter(campaign.compressor, ZSTD_c_jobSize, &job_size));
    codec_ok(ZSTD_CCtx_getParameter(campaign.compressor, ZSTD_c_windowLog, &requested_window));
    require(workers == 2 && job_size == 4 * 1024 * 1024, "inherited controls were not preserved");
    for (index = 0; index < REQUEST_COUNT; ++index) {
        SEQ_gen_type type;
        unsigned value;
        size_t remaining;
        request(index, &type, &value);
        do {
            unsigned char scratch[CHUNK + GUARD];
            SEQ_outBuffer out = {scratch, CHUNK, 0};
            memset(scratch, 0xa5, sizeof(scratch));
            remaining = SEQ_gen(&source, type, value, &out);
            guard(scratch);
            campaign.generated += out.pos;
            ++campaign.generation_calls;
            round_trip(&campaign, scratch, out.pos, ZSTD_e_continue);
        } while (remaining != 0);
    }
    round_trip(&campaign, NULL, 0, ZSTD_e_end);
    require(campaign.generated == SOURCE_BYTES && campaign.decoded == SOURCE_BYTES && campaign.finished == 1,
            "original campaign counters differ");
    require(SEQ_digest(&source) == SOURCE_DIGEST && XXH64_digest(&campaign.decoded_hash) == SOURCE_DIGEST,
            "original full-width source/output hashes differ");
    require(source.seed == FINAL_SEED, "original final random seed differs");
    expected_finish(&campaign.expected);
    require(ZSTD_getFrameHeader(&header, campaign.header, campaign.header_size) == 0,
            "incomplete original frame header");
    require(header.windowSize == (1U << 21) && requested_window == 21 &&
            header.frameContentSize == ZSTD_CONTENTSIZE_UNKNOWN &&
            header.checksumFlag == 0 && header.dictID == 2004100296U, "original frame flags differ");
    require(fclose(campaign.frame) == 0, "cannot close frame");
    printf("{\"requests\":%u,\"source_bytes\":%llu,\"source_xxh64\":\"%016llx\","
           "\"final_seed\":\"%08x\",\"generation_calls\":%u,\"compression_calls\":%u,"
           "\"decompression_calls\":%u,\"frame_bytes\":%llu,\"frame_xxh64\":\"%016llx\","
           "\"window\":%llu,\"cdict_window_log\":%u,\"requested_window_log\":%d,"
           "\"checksum\":%u,\"dictionary_id\":%u,\"workers\":%d,\"job_size\":%d}\n",
           REQUEST_COUNT, (unsigned long long)campaign.generated, (unsigned long long)SEQ_digest(&source),
           source.seed, campaign.generation_calls, campaign.compression_calls, campaign.decompression_calls,
           (unsigned long long)campaign.encoded, (unsigned long long)XXH64_digest(&campaign.encoded_hash),
           (unsigned long long)header.windowSize, parameters.windowLog, requested_window,
           header.checksumFlag, header.dictID, workers, job_size);
    ZSTD_freeCDict(prepared); ZSTD_freeDDict(decoder_dictionary);
    ZSTD_freeCCtx(campaign.compressor); ZSTD_freeDCtx(campaign.decompressor); free(dictionary);
}

static void decode(const char* dictionary_path, const char* frame_path) {
    unsigned char* dictionary = malloc(DICTIONARY_CAPACITY);
    size_t dictionary_size;
    FILE* frame = fopen(frame_path, "rb");
    Campaign campaign;
    require(dictionary && frame, "decode setup failed");
    dictionary_size = load_dictionary(dictionary_path, dictionary);
    memset(&campaign, 0, sizeof(campaign));
    campaign.expected = expected_new(); campaign.decompressor = ZSTD_createDCtx();
    require(campaign.decompressor != NULL, "decoder allocation failed");
    codec_ok(ZSTD_initDStream_usingDict(campaign.decompressor, dictionary, dictionary_size));
    codec_ok(ZSTD_DCtx_setParameter(campaign.decompressor, ZSTD_d_windowLogMax, 24));
    XXH64_reset(&campaign.decoded_hash, 0); XXH64_reset(&campaign.encoded_hash, 0);
    for (;;) {
        unsigned char scratch[CHUNK];
        size_t size = fread(scratch, 1, sizeof(scratch), frame);
        require(!ferror(frame), "cannot read frame");
        if (!size) break;
        require(campaign.finished == 0, "frame file contains trailing data");
        campaign.encoded += size;
        require(campaign.encoded <= FRAME_LIMIT, "frame input limit exceeded");
        XXH64_update(&campaign.encoded_hash, scratch, size);
        decode_chunk(&campaign, scratch, size, 0);
    }
    if (!campaign.finished) decode_chunk(&campaign, NULL, 0, 1);
    require(fclose(frame) == 0, "cannot close frame");
    expected_finish(&campaign.expected);
    require(campaign.finished == 1 && campaign.decoded == SOURCE_BYTES &&
            XXH64_digest(&campaign.decoded_hash) == SOURCE_DIGEST, "decoded counters/digest differ");
    printf("{\"decoded_bytes\":%llu,\"decoded_xxh64\":\"%016llx\",\"frame_bytes\":%llu,"
           "\"frame_xxh64\":\"%016llx\",\"requests\":%u}\n",
           (unsigned long long)campaign.decoded, (unsigned long long)XXH64_digest(&campaign.decoded_hash),
           (unsigned long long)campaign.encoded, (unsigned long long)XXH64_digest(&campaign.encoded_hash),
           REQUEST_COUNT);
    ZSTD_freeDCtx(campaign.decompressor); free(dictionary);
}

int main(int argc, char** argv) {
    require(ZSTD_versionNumber() == 10507, "wrong oracle version");
    require(argc == 4, "expected baseline/decode DICTIONARY FRAME");
    if (!strcmp(argv[1], "baseline")) baseline(argv[2], argv[3]);
    else if (!strcmp(argv[1], "decode")) decode(argv[2], argv[3]);
    else return 1;
    return 0;
}
