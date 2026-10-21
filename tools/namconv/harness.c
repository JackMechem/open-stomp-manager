/*
 * namconv harness: run the official app's NAM converter (imagelib.framework) headless and dump
 * its result and intermediate buffers. macOS only; needs Stomp Manager installed. Nothing is
 * sent to a pedal.
 *
 *   tools/namconv/build.sh
 *   tools/namconv/harness <model.nam> <outdir>
 *
 * imagelib's C++ API is not exported, but its symbol table names everything and __TEXT starts
 * at vmaddr 0, so `load address + nm offset` gives each function and buffer. The offsets below
 * are for Stomp Manager 1.0.2 (imagelib sha1 6b65b93c…, arm64); re-derive them with `nm -n` for
 * any other build.
 */
#include <CoreFoundation/CoreFoundation.h>
#include <dlfcn.h>
#include <mach-o/dyld.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <wchar.h>

#define IMAGELIB "/Applications/Stomp Manager.app/Contents/Frameworks/imagelib.framework/imagelib"

/* functions (static members of ImageUtility) */
#define OFF_loadNamFile           0x57928 /* ImageUtility::loadNamFile(wchar_t const*) */
#define OFF_setNamInfoCallBack    0x58de0 /* ImageUtility::setNamInfoCallBack(void (*)(char*)) */
#define OFF_setLoadNamEndCallBack 0x58dec /* ImageUtility::setLoadNamEndCallBack(void (*)(float*, float*, float*)) */

/* buffers: name, offset, size in bytes (sizes = distance to the next symbol) */
static const struct { const char *name; uintptr_t off; size_t bytes; } BUFS[] = {
    {"ToneTest_FftBins", 0x6d9d8, 0x178},           /* __TEXT,__const */
    {"ToneTest_FftBinsThird", 0x6db50, 0x14c},      /* __TEXT,__const */
    {"ToneTestOutputLevels", 0x11bd60, 0x178},
    {"ToneTestOutputLevelsThird", 0x11bed8, 0x14c},
    {"g1GainsDb", 0x11c024, 0x17c},
    {"NamFirG1", 0x126538, 0x1000},
    {"NamFirG2", 0x127538, 0x1000},
    {"G1IR", 0x128538, 0x8000},
    {"G2IR", 0x130538, 0x8000},
};

static const char *outdir;
static uintptr_t base;
static int finished;

static void write_file(const char *name, const void *p, size_t n) {
    char path[4096];
    snprintf(path, sizeof path, "%s/%s.bin", outdir, name);
    FILE *f = fopen(path, "wb");
    if (!f || fwrite(p, 1, n, f) != n) { perror(path); exit(1); }
    fclose(f);
    printf("  %-28s %6zu bytes\n", name, n);
}

static void dump_buffers(void) {
    for (size_t i = 0; i < sizeof BUFS / sizeof *BUFS; i++)
        write_file(BUFS[i].name, (const void *)(base + BUFS[i].off), BUFS[i].bytes);
}

static void on_info(char *s) {
    printf("info callback: %s\n", s ? s : "(null)");
    if (s) write_file("info", s, strlen(s));
}

static void on_end(float *g1, float *g2, float *factor) {
    printf("end callback: g1=%p g2=%p factor=%p", (void *)g1, (void *)g2, (void *)factor);
    if (factor) printf(" (*factor = %g)", *factor);
    printf("\n");
    if (g1) write_file("cb_g1", g1, 1024 * sizeof(float));
    if (g2) write_file("cb_g2", g2, 1024 * sizeof(float));
    if (factor) write_file("cb_factor", factor, sizeof(float));
    finished = 1;
    CFRunLoopStop(CFRunLoopGetMain());
}

int main(int argc, char **argv) {
    if (argc != 3) {
        fprintf(stderr, "usage: %s <model.nam> <outdir>\n", argv[0]);
        return 2;
    }
    outdir = argv[2];
    mkdir(outdir, 0755);

    if (!dlopen(IMAGELIB, RTLD_NOW)) { fprintf(stderr, "dlopen: %s\n", dlerror()); return 1; }
    for (uint32_t i = 0; i < _dyld_image_count(); i++)
        if (strstr(_dyld_get_image_name(i), "imagelib.framework/")) base = (uintptr_t)_dyld_get_image_header(i);
    if (!base) { fprintf(stderr, "imagelib not found among loaded images\n"); return 1; }
    printf("imagelib loaded at %#lx\n", (unsigned long)base);

    ((void (*)(void (*)(char *)))(base + OFF_setNamInfoCallBack))(on_info);
    ((void (*)(void (*)(float *, float *, float *)))(base + OFF_setLoadNamEndCallBack))(on_end);

    /* the API takes a wide-character path */
    size_t n = mbstowcs(NULL, argv[1], 0);
    wchar_t *wpath = calloc(n + 1, sizeof(wchar_t));
    mbstowcs(wpath, argv[1], n + 1);
    printf("converting %s …\n", argv[1]);
    ((void (*)(const wchar_t *))(base + OFF_loadNamFile))(wpath);

    /* the end callback is delivered through JUCE's message queue on the main run loop */
    for (int s = 0; s < 300 && !finished; s++) CFRunLoopRunInMode(kCFRunLoopDefaultMode, 1.0, false);
    if (!finished) fprintf(stderr, "no end callback after 300 s; dumping buffers anyway\n");
    printf("buffers:\n");
    dump_buffers();
    return finished ? 0 : 1;
}
