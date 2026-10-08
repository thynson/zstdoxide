/* Offline oracle only; never compiled or linked into the Rust crate.
 * Contracts derive from facebook/zstd v1.5.7 dictionary and prefix tests.
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 * Copyright (c) zstdoxide contributors. BSD-3-Clause; see LICENSE.zstd.
 */
#define ZSTD_STATIC_LINKING_ONLY
#include "zstd.h"
#ifndef DICTIONARY_PROBE_ONLY
#define FSE_STATIC_LINKING_ONLY
#define HUF_STATIC_LINKING_ONLY
#include "zdict.h"
#include "fse.h"
#include "huf.h"
#endif
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define CAPACITY (1024 * 1024)

static void require(int valid, const char* message) {
    if (!valid) { fprintf(stderr, "%s\n", message); exit(2); }
}

static size_t read_file(const char* path, unsigned char* data) {
    FILE* file = fopen(path, "rb");
    size_t size;
    require(file != NULL, "cannot open oracle input");
    size = fread(data, 1, CAPACITY, file);
    require(!ferror(file) && fgetc(file) == EOF, "oversized or unreadable oracle input");
    require(fclose(file) == 0, "cannot close oracle input");
    return size;
}

static ZSTD_dictContentType_e dictionary_mode(const char* mode) {
    if (!strcmp(mode, "raw")) return ZSTD_dct_rawContent;
    if (!strcmp(mode, "auto")) return ZSTD_dct_auto;
    require(!strcmp(mode, "formatted"), "unknown dictionary content mode");
    return ZSTD_dct_fullDict;
}

/* probe DICT FRAME EXPECTED MODE METHOD OUTPUT_CHUNK */
static void probe(char** args) {
    unsigned char* dictionary = malloc(CAPACITY);
    unsigned char* frame = malloc(CAPACITY);
    unsigned char* expected = malloc(CAPACITY);
    unsigned char* actual = malloc(CAPACITY);
    unsigned char scratch[1024];
    size_t dictionary_size, frame_size, expected_size, size = 0, consumed = 0;
    size_t result = 0, calls = 0;
    ZSTD_DCtx* context = ZSTD_createDCtx();
    ZSTD_dictContentType_e mode = dictionary_mode(args[3]);
    const char* method = args[4];
    require(dictionary && frame && expected && actual && context, "oracle allocation failed");
    dictionary_size = read_file(args[0], dictionary);
    frame_size = read_file(args[1], frame);
    expected_size = read_file(args[2], expected);
    if (!strcmp(method, "prepared")) {
        ZSTD_DDict* prepared = ZSTD_createDDict_advanced(dictionary, dictionary_size,
            ZSTD_dlm_byRef, mode, ZSTD_defaultCMem);
        require(prepared != NULL, "prepared dictionary loading failed");
        result = ZSTD_decompress_usingDDict(context, actual, CAPACITY, frame, frame_size, prepared);
        if (!ZSTD_isError(result)) { size = result; consumed = frame_size; }
        ZSTD_freeDDict(prepared);
    } else {
        int prefix = !strcmp(method, "prefix") || !strcmp(method, "prefix-stream");
        result = prefix ? ZSTD_DCtx_refPrefix_advanced(context, dictionary, dictionary_size, mode) :
            ZSTD_DCtx_loadDictionary_advanced(context, dictionary, dictionary_size, ZSTD_dlm_byRef, mode);
        require(!ZSTD_isError(result), "direct dictionary loading failed");
        if (!strcmp(method, "direct") || !strcmp(method, "prefix")) {
            result = ZSTD_decompressDCtx(context, actual, CAPACITY, frame, frame_size);
            if (!ZSTD_isError(result)) { size = result; consumed = frame_size; }
        } else {
            size_t chunk = (size_t)strtoul(args[5], NULL, 10);
            require(!strcmp(method, "stream") || !strcmp(method, "prefix-stream"), "unknown oracle method");
            require(chunk > 0 && chunk <= sizeof(scratch), "invalid oracle output chunk");
            do {
                size_t offered = consumed < frame_size ? 1 : 0;
                ZSTD_inBuffer input = {frame + consumed, offered, 0};
                ZSTD_outBuffer output = {scratch, chunk, 0};
                result = ZSTD_decompressStream(context, &output, &input);
                consumed += input.pos;
                require(size + output.pos <= CAPACITY, "oracle output exceeds bound");
                memcpy(actual + size, scratch, output.pos);
                size += output.pos;
                require(++calls < 2000000, "oracle made no forward progress");
                require(ZSTD_isError(result) || !result || offered || output.pos,
                        "oracle requested input after source exhaustion");
            } while (!ZSTD_isError(result) && (result || consumed < frame_size));
        }
    }
    printf("{\"success\":%s,\"error\":\"%s\",\"produced\":%zu,"
           "\"consumed\":%zu,\"matches_expected\":%s,\"calls\":%zu}\n",
           ZSTD_isError(result) ? "false" : "true", ZSTD_getErrorName(result), size,
           consumed, size == expected_size && !memcmp(actual, expected, size) ? "true" : "false", calls);
    ZSTD_freeDCtx(context);
    free(actual); free(expected); free(frame); free(dictionary);
}

#ifndef DICTIONARY_PROBE_ONLY
static void codec_ok(size_t result) {
    if (ZSTD_isError(result)) {
        fprintf(stderr, "codec oracle failed: %s\n", ZSTD_getErrorName(result));
        exit(2);
    }
}

static void path_join(char* path, size_t capacity, const char* directory, const char* name) {
    int length = snprintf(path, capacity, "%s/%s", directory, name);
    require(length > 0 && (size_t)length < capacity, "oracle path too long");
}

static void save_file(const char* directory, const char* name, const void* data, size_t size) {
    char path[4096];
    FILE* file;
    path_join(path, sizeof(path), directory, name);
    file = fopen(path, "wb");
    require(file != NULL, "cannot open oracle output");
    require(fwrite(data, 1, size, file) == size && fclose(file) == 0, "cannot save oracle output");
}

static uint32_t little32(const unsigned char* bytes) {
    return (uint32_t)bytes[0] | (uint32_t)bytes[1] << 8 |
           (uint32_t)bytes[2] << 16 | (uint32_t)bytes[3] << 24;
}

static void put_number(unsigned char* bytes, uint32_t number, unsigned length) {
    unsigned i;
    for (i = 0; i < length; ++i) bytes[i] = (unsigned char)(number >> (8 * i));
}

static void dictionary_metadata(const char* name, const unsigned char* dictionary, size_t size) {
    unsigned char weights[256];
    unsigned ranks[16] = {0}, symbols = 0, log = 0, i, table;
    const char* names[] = {"offset", "match_length", "literal_length"};
    const unsigned maxima[] = {31, 52, 35};
    size_t offset = 8, length;
    printf("{\"path\":\"%s\",\"id\":%u,\"header_size\":%zu,\"huffman\":{\"offset\":8,",
           name, ZSTD_getDictID_fromDict(dictionary, size), ZDICT_getDictHeaderSize(dictionary, size));
    length = HUF_readStats(weights, sizeof(weights), ranks, &symbols, &log, dictionary + offset, size - offset);
    codec_ok(length);
    printf("\"size\":%zu,\"table_log\":%u,\"weights\":[", length, log);
    for (i = 0; i < symbols; ++i) printf("%s%u", i ? "," : "", weights[i]);
    printf("]},\"fse\":[");
    offset += length;
    for (table = 0; table < 3; ++table) {
        short counts[256] = {0};
        unsigned maximum = maxima[table];
        length = FSE_readNCount(counts, &maximum, &log, dictionary + offset, size - offset);
        codec_ok(length);
        printf("%s{\"kind\":\"%s\",\"offset\":%zu,\"size\":%zu,\"table_log\":%u,\"counts\":[",
               table ? "," : "", names[table], offset, length, log);
        for (i = 0; i <= maximum; ++i) printf("%s%d", i ? "," : "", counts[i]);
        printf("]}");
        offset += length;
    }
    require(offset + 12 == ZDICT_getDictHeaderSize(dictionary, size), "dictionary header metadata mismatch");
    printf("],\"repeat_offset_start\":%zu,\"repeat_offsets\":[%u,%u,%u],\"content_size\":%zu}",
           offset, little32(dictionary + offset), little32(dictionary + offset + 4),
           little32(dictionary + offset + 8), size - offset - 12);
}

static void configure(ZSTD_CCtx* context, const void* dictionary, size_t dictionary_size,
                      ZSTD_dictContentType_e mode, int level, int omit_id, int prefix) {
    codec_ok(ZSTD_CCtx_reset(context, ZSTD_reset_session_and_parameters));
    codec_ok(ZSTD_CCtx_setParameter(context, ZSTD_c_compressionLevel, level));
    codec_ok(ZSTD_CCtx_setParameter(context, ZSTD_c_windowLog, 10));
    codec_ok(ZSTD_CCtx_setParameter(context, ZSTD_c_checksumFlag, 1));
    codec_ok(ZSTD_CCtx_setParameter(context, ZSTD_c_dictIDFlag, !omit_id));
    codec_ok(prefix ? ZSTD_CCtx_refPrefix_advanced(context, dictionary, dictionary_size, mode) :
             ZSTD_CCtx_loadDictionary_advanced(context, dictionary, dictionary_size, ZSTD_dlm_byRef, mode));
}

static size_t compress_frame(ZSTD_CCtx* context, unsigned char* frame, const void* source, size_t size) {
    size_t length = ZSTD_compress2(context, frame, CAPACITY, source, size);
    codec_ok(length);
    return length;
}

static void save_frame(const char* directory, const char* name,
                       const unsigned char* frame, size_t frame_size,
                       const unsigned char* source, size_t source_size,
                       const char* dictionary_name, const unsigned char* dictionary, size_t dictionary_size,
                       ZSTD_dictContentType_e mode, int level, int prefix, int first) {
    unsigned char* restored = malloc(CAPACITY);
    ZSTD_DCtx* decoder = ZSTD_createDCtx();
    ZSTD_FrameHeader header;
    size_t decoded, without_dictionary;
    char file_name[256];
    require(restored && decoder, "frame verifier allocation failed");
    codec_ok(ZSTD_DCtx_loadDictionary_advanced(decoder, dictionary, dictionary_size, ZSTD_dlm_byRef, mode));
    decoded = ZSTD_decompressDCtx(decoder, restored, CAPACITY, frame, frame_size);
    codec_ok(decoded);
    require(decoded == source_size && !memcmp(restored, source, source_size), "generated frame output differs");
    codec_ok(ZSTD_DCtx_reset(decoder, ZSTD_reset_session_and_parameters));
    without_dictionary = ZSTD_decompressDCtx(decoder, restored, CAPACITY, frame, frame_size);
    require(ZSTD_getFrameHeader(&header, frame, frame_size) == 0, "generated frame header incomplete");
    snprintf(file_name, sizeof(file_name), "%s.zst", name);
    save_file(directory, file_name, frame, frame_size);
    snprintf(file_name, sizeof(file_name), "%s.expected", name);
    save_file(directory, file_name, source, source_size);
    printf("%s{\"name\":\"%s\",\"dictionary\":\"%s\",\"mode\":\"%s\",\"compression_level\":%d,"
           "\"prefix_compression\":%s,\"frame_dictionary_id\":%u,\"window\":%llu,"
           "\"content_size\":%llu,\"checksum\":%s,\"without_dictionary_success\":%s}",
           first ? "" : ",", name, dictionary_name, mode == ZSTD_dct_rawContent ? "raw" : "formatted",
           level, prefix ? "true" : "false", header.dictID, (unsigned long long)header.windowSize,
           header.frameContentSize, header.checksumFlag ? "true" : "false",
           ZSTD_isError(without_dictionary) ? "false" : "true");
    ZSTD_freeDCtx(decoder);
    free(restored);
}

static size_t treeless_frame(unsigned char* frame, const unsigned char* dictionary, size_t dictionary_size,
                             const unsigned char* source, size_t source_size, int four_streams) {
    HUF_CElt table[HUF_CTABLE_SIZE_ST(255)];
    unsigned maximum = 255, zero_weights = 0;
    unsigned char encoded[8192];
    size_t length, payload_size;
    uint32_t literals_header;
    memset(table, 0, sizeof(table));
    codec_ok(HUF_readCTable(table, &maximum, dictionary + 8, dictionary_size - 8, &zero_weights));
    length = four_streams ? HUF_compress4X_usingCTable(encoded, sizeof(encoded), source, source_size, table, 0) :
                           HUF_compress1X_usingCTable(encoded, sizeof(encoded), source, source_size, table, 0);
    codec_ok(length);
    require(length > 0 && length < 1024 && source_size < 1024, "treeless witness does not fit three-byte literals header");
    /* Unknown content size, window 1024, four-byte dictionary ID, no checksum. */
    memcpy(frame, "\x28\xb5\x2f\xfd\x03\x00", 6);
    put_number(frame + 6, ZSTD_getDictID_fromDict(dictionary, dictionary_size), 4);
    payload_size = 3 + length + 1;
    put_number(frame + 10, (uint32_t)(payload_size << 3) | 5, 3);
    literals_header = 3 | (four_streams ? 4u : 0u) | (uint32_t)source_size << 4 | (uint32_t)length << 14;
    put_number(frame + 13, literals_header, 3);
    memcpy(frame + 16, encoded, length);
    frame[16 + length] = 0; /* No sequences; dictionary entropy alone is required. */
    return 17 + length;
}

static unsigned first_sequence_modes(const unsigned char* frame, size_t size) {
    ZSTD_FrameHeader header;
    const unsigned char* block;
    size_t literals_header, literals_size, offset, sequences;
    unsigned format, kind;
    uint32_t bits;
    require(ZSTD_getFrameHeader(&header, frame, size) == 0, "cannot inspect frame header");
    offset = header.headerSize;
    if (((frame[offset] >> 1) & 3) != 2) return 0;
    block = frame + offset + 3;
    kind = block[0] & 3;
    format = (block[0] >> 2) & 3;
    bits = little32(block);
    if (kind < 2) {
        literals_header = format == 1 ? 2 : format == 3 ? 3 : 1;
        literals_size = literals_header == 1 ? block[0] >> 3 :
            literals_header == 2 ? (bits & 65535) >> 4 : (bits & 0xffffff) >> 4;
        if (kind == 1) literals_size = 1;
    } else {
        literals_header = format < 2 ? 3 : format + 2;
        literals_size = format < 2 ? ((bits >> 14) & 1023) :
            format == 2 ? bits >> 18 : ((bits >> 22) | (size_t)block[4] << 10);
    }
    offset = literals_header + literals_size;
    sequences = block[offset];
    if (!sequences) return 0;
    offset += sequences < 128 ? 1 : sequences < 255 ? 2 : 3;
    return block[offset];
}

/* Inputs are made by the independent Python importer; samples are 256 x 512 bytes. */
static void generate(const char* directory) {
    unsigned char raw[CAPACITY], samples[CAPACITY], trained[4096], formatted[4096], duplicate[4096];
    unsigned char frame[CAPACITY], first_frame[CAPACITY], source[8192], treeless_source[768];
    size_t sample_sizes[256], raw_size, samples_size, trained_size, formatted_size, header_size;
    size_t size, first_size, i, j;
    ZSTD_CCtx* context = ZSTD_createCCtx();
    ZDICT_params_t parameters = {3, 0, 0x12345678};
    char path[4096];
    require(context != NULL, "compression context allocation failed");
    path_join(path, sizeof(path), directory, "boundary-dictionary.raw");
    raw_size = read_file(path, raw);
    path_join(path, sizeof(path), directory, "training-samples.bin");
    samples_size = read_file(path, samples);
    require(raw_size == 2048 && samples_size == 256 * 512, "unexpected sample input sizes");
    for (i = 0; i < 256; ++i) sample_sizes[i] = 512;
    trained_size = ZDICT_trainFromBuffer(trained, 2048, samples, sample_sizes, 256);
    codec_ok(trained_size);
    formatted_size = ZDICT_finalizeDictionary(formatted, sizeof(formatted), raw, raw_size,
                                               samples, sample_sizes, 256, parameters);
    codec_ok(formatted_size);
    header_size = ZDICT_getDictHeaderSize(formatted, formatted_size);
    codec_ok(header_size);
    require(formatted_size - header_size == raw_size && !memcmp(formatted + header_size, raw, raw_size),
            "formatted dictionary must retain the complete raw content");
    memcpy(duplicate, formatted, formatted_size);
    for (i = 0; i < 3; ++i) put_number(duplicate + header_size - 12 + i * 4, 10, 4);
    memset(duplicate + formatted_size - 8, 'x', 8);
    save_file(directory, "trained.dict", trained, trained_size);
    save_file(directory, "formatted.dict", formatted, formatted_size);
    save_file(directory, "duplicate-reps.dict", duplicate, formatted_size);
    printf("{\"dictionaries\":[");
    dictionary_metadata("trained.dict", trained, trained_size);
    printf(","); dictionary_metadata("formatted.dict", formatted, formatted_size);
    printf(","); dictionary_metadata("duplicate-reps.dict", duplicate, formatted_size);
    printf("],\"cases\":[");

    memcpy(source, raw, 256);
    configure(context, raw, raw_size, ZSTD_dct_rawContent, 3, 0, 0);
    size = compress_frame(context, frame, source, 256);
    save_frame(directory, "raw-small-window", frame, size, source, 256, "boundary-dictionary.raw",
               raw, raw_size, ZSTD_dct_rawContent, 3, 0, 1);
    configure(context, formatted, formatted_size, ZSTD_dct_fullDict, 3, 0, 0);
    size = compress_frame(context, frame, raw, 256);
    save_frame(directory, "formatted-id", frame, size, raw, 256, "formatted.dict",
               formatted, formatted_size, ZSTD_dct_fullDict, 3, 0, 0);
    memcpy(first_frame, frame, size); first_size = size;
    memcpy(frame + size, frame, size);
    memcpy(source, raw, 256); memcpy(source + 256, raw, 256);
    save_frame(directory, "persistent-concatenation", frame, size * 2, source, 512, "formatted.dict",
               formatted, formatted_size, ZSTD_dct_fullDict, 3, 0, 0);
    configure(context, formatted, formatted_size, ZSTD_dct_fullDict, 3, 1, 0);
    size = compress_frame(context, frame, raw, 256);
    save_frame(directory, "formatted-omitted-id", frame, size, raw, 256, "formatted.dict",
               formatted, formatted_size, ZSTD_dct_fullDict, 3, 0, 0);
    configure(context, trained, trained_size, ZSTD_dct_fullDict, 3, 0, 0);
    size = compress_frame(context, frame, samples + 37 * 512, 512);
    save_frame(directory, "trained-sample", frame, size, samples + 37 * 512, 512, "trained.dict",
               trained, trained_size, ZSTD_dct_fullDict, 3, 0, 0);
    memset(source, 'x', 1024);
    configure(context, duplicate, formatted_size, ZSTD_dct_fullDict, 19, 0, 0);
    size = compress_frame(context, frame, source, 1024);
    save_frame(directory, "duplicate-reps", frame, size, source, 1024, "duplicate-reps.dict",
               duplicate, formatted_size, ZSTD_dct_fullDict, 19, 0, 0);
    /* Unlike compressor heuristics, this explicit LL=0/OF=0 sequence must use
     * dictionary rep2=10. Default rep2=4 would yield different bytes. */
    memcpy(frame, "\x28\xb5\x2f\xfd\x03\x00", 6);
    put_number(frame + 6, parameters.dictID, 4);
    put_number(frame + 10, (7u << 3) | 5, 3);
    memcpy(frame + 13, "\x00\x01\x54\x00\x00\x00\x01", 7);
    save_frame(directory, "duplicate-reps-initial", frame, 20, duplicate + formatted_size - 10, 3,
               "duplicate-reps.dict", duplicate, formatted_size, ZSTD_dct_fullDict, 0, 0, 0);

    for (i = 0; i < sizeof(treeless_source); ++i) treeless_source[i] = samples[37 * 512 + i % 512];
    for (i = 0; i < 2; ++i) {
        size = treeless_frame(frame, formatted, formatted_size, treeless_source, sizeof(treeless_source), (int)i);
        save_frame(directory, i ? "dictionary-treeless-four-streams" : "dictionary-treeless-literals",
                   frame, size, treeless_source, sizeof(treeless_source), "formatted.dict",
                   formatted, formatted_size, ZSTD_dct_fullDict, 0, 0, 0);
    }

    /* Retain the first deterministic C-compressed witness using all three initial FSE tables. */
    for (i = 0; i < 256; ++i) {
        configure(context, trained, trained_size, ZSTD_dct_fullDict, 3, 0, 0);
        size = compress_frame(context, frame, samples + i * 512, 512);
        if ((first_sequence_modes(frame, size) & 0xfc) == 0xfc) break;
    }
    require(i < 256, "no first-block dictionary FSE repeat witness found");
    save_frame(directory, "dictionary-repeat-sequences", frame, size, samples + i * 512, 512, "trained.dict",
               trained, trained_size, ZSTD_dct_fullDict, 3, 0, 0);

    memcpy(source, raw + raw_size - 256, 256);
    configure(context, raw, raw_size, ZSTD_dct_rawContent, 3, 0, 1);
    first_size = compress_frame(context, first_frame, source, 256);
    save_frame(directory, "prefix-first", first_frame, first_size, source, 256, "boundary-dictionary.raw",
               raw, raw_size, ZSTD_dct_rawContent, 3, 1, 0);
    size = compress_frame(context, frame, source, 256);
    save_frame(directory, "prefix-second", frame, size, source, 256, "boundary-dictionary.raw",
               raw, raw_size, ZSTD_dct_rawContent, 3, 1, 0);
    memcpy(first_frame + first_size, frame, size);
    for (j = 0; j < 2; ++j) memcpy(source + j * 256, raw + raw_size - 256, 256);
    save_frame(directory, "prefix-once-concatenation", first_frame, first_size + size, source, 512,
               "boundary-dictionary.raw", raw, raw_size, ZSTD_dct_rawContent, 3, 1, 0);
    memcpy(first_frame + first_size, first_frame, first_size);
    save_frame(directory, "prefix-reused-concatenation", first_frame, first_size * 2, source, 512,
               "boundary-dictionary.raw", raw, raw_size, ZSTD_dct_rawContent, 3, 1, 0);
    printf("],\"repeat_witness_sample\":%zu}\n", i);
    ZSTD_freeCCtx(context);
}
#endif

int main(int argc, char** argv) {
    require(ZSTD_versionNumber() == 10507, "oracle must be zstd v1.5.7");
    if (argc == 8 && !strcmp(argv[1], "probe")) {
        probe(argv + 2);
        return 0;
    }
#ifndef DICTIONARY_PROBE_ONLY
    if (argc == 3 && !strcmp(argv[1], "generate")) {
        generate(argv[2]);
        return 0;
    }
#endif
    require(0, "usage: dictionary-vectors probe DICT FRAME EXPECTED MODE METHOD OUTPUT_CHUNK | generate DIRECTORY");
    return 2;
}
