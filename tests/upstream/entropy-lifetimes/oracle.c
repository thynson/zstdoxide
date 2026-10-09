/* Offline fixture generator and test oracle; never linked into Rust.
 * Derived from facebook/zstd v1.5.7 tests/zstreamtest.c:145-166, :263-302,
 * :1652-1718, :1831-1919. Copyright (c) Meta Platforms, Inc. and affiliates.
 * Copyright (c) zstdoxide contributors. BSD-3-Clause; see root LICENSE.zstd.
 */
#define ZSTD_STATIC_LINKING_ONLY
#define ZDICT_STATIC_LINKING_ONLY
#include "zstd.h"
#include "zdict.h"
#include "datagen.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define TRAINING_SIZE (10U * 1024U * 1024U)
#define SAMPLE_SIZE (16U * 1024U)
#define DICTIONARY_CAPACITY (48U * 1024U)
#define BLOCK_SIZE (128U * 1024U)
#define INPUT_SIZE (2U * BLOCK_SIZE)
#define FLUSH_SIZE 6U
#define FLUSH_COUNT ((256U * 1024U + FLUSH_SIZE - 1) / FLUSH_SIZE)
#define SMALL_BLOCK_INPUT_SIZE (FLUSH_COUNT * FLUSH_SIZE + 9U * 128U)
/* Same output allocation as basicUnitTests; bounds both original campaigns. */
#define OUTPUT_CAPACITY (8U + 200U * 1024U + ZSTD_compressBound(TRAINING_SIZE))

static void require(int condition, const char* message) {
    if (!condition) { fprintf(stderr, "%s\n", message); exit(2); }
}

static uint32_t next_random(uint32_t* seed) {
    uint32_t value = *seed * UINT32_C(2654435761) + UINT32_C(2246822519);
    *seed = (value << 13) | (value >> 19);
    return *seed >> 5;
}

static uint32_t case_seed(unsigned maximum_window_log) {
    uint32_t seed = 0;
    unsigned window_log, index;
    for (window_log = ZSTD_WINDOWLOG_MIN; window_log <= maximum_window_log; ++window_log) {
        for (index = 1; index < 10000; ++index) {
            (void)next_random(&seed); /* offset */
            (void)next_random(&seed); /* literal length */
        }
    }
    return seed;
}

static uint64_t fnv1a(const void* input, size_t size) {
    const unsigned char* bytes = input;
    uint64_t value = UINT64_C(14695981039346656037);
    size_t index;
    for (index = 0; index < size; ++index) {
        value = (value ^ bytes[index]) * UINT64_C(1099511628211);
    }
    return value;
}

static void save(const char* directory, const char* name, const void* data, size_t size) {
    char path[4096];
    int length = snprintf(path, sizeof(path), "%s/%s", directory, name);
    FILE* file;
    require(length > 0 && (size_t)length < sizeof(path), "output path too long");
    file = fopen(path, "wb");
    require(file != NULL, "cannot open output");
    require(fwrite(data, 1, size, file) == size, "cannot write output");
    require(fclose(file) == 0, "cannot close output");
}

static size_t load(const char* path, void* output, size_t capacity) {
    FILE* file = fopen(path, "rb");
    size_t size;
    require(file != NULL, "cannot open input");
    size = fread(output, 1, capacity, file);
    require(!ferror(file) && fgetc(file) == EOF, "unreadable or oversized input");
    require(fclose(file) == 0, "cannot close input");
    return size;
}

static void codec_ok(size_t result) {
    require(!ZSTD_isError(result), ZSTD_getErrorName(result));
}

static void generate(const char* directory) {
    unsigned char* training = malloc(TRAINING_SIZE);
    unsigned char* dictionary = malloc(DICTIONARY_CAPACITY);
    unsigned char* source = malloc(INPUT_SIZE);
    unsigned char* output = malloc(ZSTD_compressBound(INPUT_SIZE));
    unsigned char* decoded = malloc(INPUT_SIZE);
    size_t const training_prefix = TRAINING_SIZE / 3;
    unsigned const sample_count = (unsigned)((training_prefix + SAMPLE_SIZE - 1) / SAMPLE_SIZE);
    size_t sample_sizes[(TRAINING_SIZE / 3 + SAMPLE_SIZE - 1) / SAMPLE_SIZE];
    size_t dictionary_size, header_size;
    unsigned sample, mode;
    ZSTD_CCtx* compressor = ZSTD_createCCtx();
    ZSTD_DCtx* decompressor = ZSTD_createDCtx();
    ZSTD_compressionParameters parameters;
    ZSTD_CDict* prepared;
    require(training && dictionary && source && output && decoded && compressor && decompressor,
            "oracle allocation failed");
    /* The dictionary is constructed before any FUZ_rand(&seed) call. */
    RDG_genBuffer(training, TRAINING_SIZE, 0.50, 0., 0);
    for (sample = 0; sample + 1 < sample_count; ++sample) sample_sizes[sample] = SAMPLE_SIZE;
    sample_sizes[sample_count - 1] = training_prefix - SAMPLE_SIZE * (sample_count - 1);
    dictionary_size = ZDICT_trainFromBuffer(dictionary, DICTIONARY_CAPACITY, training,
                                            sample_sizes, sample_count);
    require(!ZDICT_isError(dictionary_size), ZDICT_getErrorName(dictionary_size));
    header_size = ZDICT_getDictHeaderSize(dictionary, dictionary_size);
    require(!ZDICT_isError(header_size) && dictionary_size >= 384, "invalid prepared dictionary");
    save(directory, "original-default.dict", dictionary, dictionary_size);
    parameters = ZSTD_getCParams(3, 0, dictionary_size);
    prepared = ZSTD_createCDict_advanced(dictionary, dictionary_size, ZSTD_dlm_byRef,
                                        ZSTD_dct_fullDict, parameters, ZSTD_defaultCMem);
    require(prepared != NULL, "original full/by-reference CDict creation failed");
    printf("{\"dictionary_size\":%zu,\"dictionary_id\":%u,\"header_size\":%zu,"
           "\"window_log\":%u,\"sample_count\":%u,\"last_sample_size\":%zu,"
           "\"training_fnv1a\":\"%016llx\",\"cases\":[",
           dictionary_size, ZDICT_getDictID(dictionary, dictionary_size), header_size,
           parameters.windowLog, sample_count, sample_sizes[sample_count - 1],
           (unsigned long long)fnv1a(training, TRAINING_SIZE));
    for (mode = 0; mode < 2; ++mode) {
        unsigned maximum_log = mode ? 29 : 26;
        uint32_t seed = case_seed(maximum_log);
        size_t compressed, regenerated;
        ZSTD_frameHeader frame_header;
        char filename[64];
        RDG_genBuffer(source, BLOCK_SIZE, 0., 0., seed);
        RDG_genBuffer(source + BLOCK_SIZE, BLOCK_SIZE - 256, 0.05, 0., seed);
        memcpy(source + INPUT_SIZE - 256, dictionary + 256, 128);
        memcpy(source + INPUT_SIZE - 128, dictionary + 128, 128);
        compressed = ZSTD_compress_usingCDict(compressor, output, ZSTD_compressBound(INPUT_SIZE),
                                             source, INPUT_SIZE, prepared);
        codec_ok(compressed);
        require(ZSTD_getFrameHeader(&frame_header, output, compressed) == 0,
                "incomplete original frame header");
        regenerated = ZSTD_decompress_usingDict(decompressor, decoded, INPUT_SIZE, output,
                                                compressed, dictionary, dictionary_size);
        codec_ok(regenerated);
        require(regenerated == INPUT_SIZE && memcmp(source, decoded, INPUT_SIZE) == 0,
                "original zstreamtest:1831 output differs");
        snprintf(filename, sizeof(filename), "source-%s.bin", mode ? "big" : "small");
        save(directory, filename, source, INPUT_SIZE);
        snprintf(filename, sizeof(filename), "original-%s.zst", mode ? "big" : "small");
        save(directory, filename, output, compressed);
        printf("%s{\"maximum_prior_window_log\":%u,\"rng_draws\":%u,\"seed\":%u,"
               "\"source_size\":%u,\"source_fnv1a\":\"%016llx\",\"frame_size\":%zu,"
               "\"frame_window\":%llu}",
               mode ? "," : "", maximum_log, (maximum_log - ZSTD_WINDOWLOG_MIN + 1) * 9999 * 2,
               seed, INPUT_SIZE, (unsigned long long)fnv1a(source, INPUT_SIZE), compressed,
               (unsigned long long)frame_header.windowSize);
    }
    puts("]}");
    ZSTD_freeCDict(prepared);
    ZSTD_freeCCtx(compressor);
    ZSTD_freeDCtx(decompressor);
    free(decoded); free(output); free(source); free(dictionary); free(training);
}

static void small_blocks(const char* dictionary_path, const char* directory) {
    unsigned char* dictionary = malloc(DICTIONARY_CAPACITY);
    unsigned char* source = malloc(SMALL_BLOCK_INPUT_SIZE);
    unsigned char* output = malloc(OUTPUT_CAPACITY);
    unsigned char* decoded = malloc(SMALL_BLOCK_INPUT_SIZE);
    unsigned char* boundaries = malloc(4U * (FLUSH_COUNT + 9U));
    ZSTD_CCtx* compressor = ZSTD_createCCtx();
    ZSTD_DCtx* decompressor = ZSTD_createDCtx();
    size_t dictionary_size, regenerated, source_position = 0;
    unsigned calls = 0, zero_flush_results = 0;
    int remaining_input = 256 * 1024, offset;
    ZSTD_compressionParameters parameters;
    ZSTD_CDict* prepared;
    ZSTD_outBuffer out;
    ZSTD_frameHeader frame_header;
    require(dictionary && source && output && decoded && boundaries && compressor && decompressor,
            "small-block oracle allocation failed");
    dictionary_size = load(dictionary_path, dictionary, DICTIONARY_CAPACITY);
    require(dictionary_size >= 1152, "dictionary does not cover original ranges");
    parameters = ZSTD_getCParams(3, 0, dictionary_size);
    prepared = ZSTD_createCDict_advanced(dictionary, dictionary_size, ZSTD_dlm_byRef,
                                        ZSTD_dct_fullDict, parameters, ZSTD_defaultCMem);
    require(prepared != NULL, "original full/by-reference CDict creation failed");
    out.dst = output; out.size = OUTPUT_CAPACITY; out.pos = 0;
    codec_ok(ZSTD_CCtx_reset(compressor, ZSTD_reset_session_and_parameters));
    codec_ok(ZSTD_CCtx_refCDict(compressor, prepared));
    codec_ok(ZSTD_CCtx_setParameter(compressor, ZSTD_c_checksumFlag, 1));
    while (remaining_input > 0) {
        unsigned char test_buffer[FLUSH_SIZE] = {0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa};
        ZSTD_inBuffer in = {test_buffer, sizeof(test_buffer), 0};
        size_t result = ZSTD_compressStream2(compressor, &out, &in, ZSTD_e_flush);
        unsigned shift;
        codec_ok(result);
        require(in.pos == in.size, "original six-byte flush did not consume input");
        if (result == 0) ++zero_flush_results;
        memcpy(source + source_position, test_buffer, sizeof(test_buffer));
        source_position += sizeof(test_buffer);
        require(out.pos <= UINT32_MAX, "boundary exceeds u32");
        for (shift = 0; shift < 4; ++shift)
            boundaries[4U * calls + shift] = (unsigned char)(out.pos >> (8U * shift));
        ++calls;
        remaining_input -= (int)sizeof(test_buffer);
    }
    require(calls == FLUSH_COUNT && zero_flush_results == FLUSH_COUNT,
            "original six-byte flush count or completion differs");
    for (offset = 1024; offset >= 0; offset -= 128) {
        ZSTD_inBuffer in = {dictionary + offset, 128, 0};
        ZSTD_EndDirective directive = offset > 0 ? ZSTD_e_continue : ZSTD_e_end;
        size_t result = ZSTD_compressStream2(compressor, &out, &in, directive);
        unsigned shift;
        codec_ok(result);
        require(in.pos == in.size, "original dictionary range was not consumed");
        require(offset > 0 || result == 0, "original final range did not finish frame");
        memcpy(source + source_position, dictionary + offset, 128);
        source_position += 128;
        for (shift = 0; shift < 4; ++shift)
            boundaries[4U * calls + shift] = (unsigned char)(out.pos >> (8U * shift));
        ++calls;
    }
    require(source_position == SMALL_BLOCK_INPUT_SIZE && calls == FLUSH_COUNT + 9,
            "original small-block input size differs");
    require(ZSTD_getFrameHeader(&frame_header, output, out.pos) == 0,
            "incomplete original small-block header");
    regenerated = ZSTD_decompress_usingDict(decompressor, decoded, SMALL_BLOCK_INPUT_SIZE,
                                            output, out.pos, dictionary, dictionary_size);
    codec_ok(regenerated);
    require(regenerated == source_position && memcmp(source, decoded, regenerated) == 0,
            "original zstreamtest:1884 output differs");
    save(directory, "source-six-byte.bin", source, source_position);
    save(directory, "original-six-byte.zst", output, out.pos);
    save(directory, "original-six-byte-boundaries.le32", boundaries, 4U * calls);
    printf("{\"dictionary_size\":%zu,\"dictionary_id\":%u,\"window_log\":%u,"
           "\"frame_window\":%llu,\"content_size_unknown\":%s,\"checksum\":%u,"
           "\"flush_calls\":%u,\"complete_flush_calls\":%u,\"dictionary_ranges\":9,"
           "\"output_capacity\":%zu,\"source_size\":%zu,\"source_fnv1a\":\"%016llx\","
           "\"frame_size\":%zu,\"boundary_fnv1a\":\"%016llx\"}\n",
           dictionary_size, ZDICT_getDictID(dictionary, dictionary_size), parameters.windowLog,
           (unsigned long long)frame_header.windowSize,
           frame_header.frameContentSize == ZSTD_CONTENTSIZE_UNKNOWN ? "true" : "false",
           frame_header.checksumFlag, FLUSH_COUNT, zero_flush_results, OUTPUT_CAPACITY,
           source_position, (unsigned long long)fnv1a(source, source_position), out.pos,
           (unsigned long long)fnv1a(boundaries, 4U * calls));
    ZSTD_freeCDict(prepared); ZSTD_freeCCtx(compressor); ZSTD_freeDCtx(decompressor);
    free(boundaries); free(decoded); free(output); free(source); free(dictionary);
}

static void decode(const char* dictionary_path, const char* frame_path, const char* expected_path) {
    unsigned char* dictionary = malloc(DICTIONARY_CAPACITY);
    unsigned char* frame = malloc(OUTPUT_CAPACITY);
    unsigned char* expected = malloc(SMALL_BLOCK_INPUT_SIZE);
    unsigned char* output = malloc(SMALL_BLOCK_INPUT_SIZE);
    ZSTD_DCtx* context = ZSTD_createDCtx();
    size_t dictionary_size, frame_size, expected_size, decoded;
    require(dictionary && frame && expected && output && context, "decode allocation failed");
    dictionary_size = load(dictionary_path, dictionary, DICTIONARY_CAPACITY);
    frame_size = load(frame_path, frame, OUTPUT_CAPACITY);
    expected_size = load(expected_path, expected, SMALL_BLOCK_INPUT_SIZE);
    decoded = ZSTD_decompress_usingDict(context, output, SMALL_BLOCK_INPUT_SIZE, frame, frame_size,
                                       dictionary, dictionary_size);
    codec_ok(decoded);
    require(decoded == expected_size && memcmp(output, expected, decoded) == 0,
            "independent C decode differs");
    printf("%zu\n", decoded);
    ZSTD_freeDCtx(context);
    free(output); free(expected); free(frame); free(dictionary);
}

int main(int argc, char** argv) {
    require(ZSTD_versionNumber() == 10507, "wrong oracle version");
    if (argc == 3 && !strcmp(argv[1], "generate")) generate(argv[2]);
    else if (argc == 4 && !strcmp(argv[1], "small-blocks")) small_blocks(argv[2], argv[3]);
    else if (argc == 5 && !strcmp(argv[1], "decode")) decode(argv[2], argv[3], argv[4]);
    else return 1;
    return 0;
}
