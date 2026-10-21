/*
 * namconv harness: run the official app's NAM converter (imagelib.framework) headless and dump
 * its result and intermediate buffers. macOS only; needs Stomp Manager installed. Nothing is
 * sent to a pedal.
 *
 *   tools/namconv/build.sh
 *   tools/namconv/harness <model.nam> <outdir> [--log-io]
 *
 * --log-io wraps the model's process() (vtable slot) and appends every call to <outdir>/model_io.bin
 * as records of: int32 n, float32 input[n], float32 output[n]. That captures the BLA run and every
 * tone of the tone test, so the rest of the pipeline can be checked without a NAM engine.
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
#include <sys/mman.h>
#include <sys/stat.h>
#include <unistd.h>
#include <wchar.h>

#define IMAGELIB "/Applications/Stomp Manager.app/Contents/Frameworks/imagelib.framework/imagelib"

/* functions (static members of ImageUtility) */
#define OFF_loadNamFile           0x57928 /* ImageUtility::loadNamFile(wchar_t const*) */
#define OFF_setNamInfoCallBack    0x58de0 /* ImageUtility::setNamInfoCallBack(void (*)(char*)) */
#define OFF_setLoadNamEndCallBack 0x58dec /* ImageUtility::setLoadNamEndCallBack(void (*)(float*, float*, float*)) */

/* vtables (address point = symbol + 0x10); process(float**, float**, int) is the 4th virtual: +0x18 */
#define OFF_vtable_SlimmableWavenet 0x808f0
#define OFF_vtable_WaveNet          0x80810
#define OFF_vtable_ContainerModel   0x812e0 /* what a SlimmableContainer .nam loads as */
#define VT_PROCESS_SLOT             (0x10 + 0x18)

/* buffers: name, offset, size in bytes (sizes = distance to the next symbol) */
static const struct { const char *name; uintptr_t off; size_t bytes; } BUFS[] = {
    {"oddPrimeNumbers", 0x69f64, 0x1dac},           /* __TEXT,__const */
    {"ToneTest_FftBins", 0x6d9d8, 0x178},           /* __TEXT,__const */
    {"BLA_x", 0x6dc9c, 0x8000},                     /* __TEXT,__const: the BLA excitation */
    {"ToneTest_FftBinsThird", 0x6db50, 0x14c},      /* __TEXT,__const */
    {"ToneTestOutputLevels", 0x11bd60, 0x178},
    {"ToneTestOutputLevelsThird", 0x11bed8, 0x14c},
    {"g1GainsDb", 0x11c024, 0x17c},
    {"NamFirG1", 0x126538, 0x1000},
    {"NamFirG2", 0x127538, 0x1000},
    {"G1IR", 0x128538, 0x8000},
    {"G2IR", 0x130538, 0x8000},
    {"levelsTesterItem", 0x85820, 0x8000},          /* multisine generated at the end of run() */
};

typedef void (*process_fn)(void *self, float **in, float **out, int n);
static process_fn orig_slim, orig_wavenet, orig_container;
static FILE *iolog;
static int depth; /* only the outermost process() call is logged (containers call their submodels) */

static void log_call(float **in, float **out, int n) {
    int32_t n32 = n;
    fwrite(&n32, sizeof n32, 1, iolog);
    fwrite(in[0], sizeof(float), (size_t)n, iolog);
    fwrite(out[0], sizeof(float), (size_t)n, iolog);
}

#define HOOK(name, orig)                                       \
    static void name(void *self, float **in, float **out, int n) { \
        depth++;                                               \
        orig(self, in, out, n);                                \
        if (--depth == 0) log_call(in, out, n);                \
    }
HOOK(hook_slim, orig_slim)
HOOK(hook_wavenet, orig_wavenet)
HOOK(hook_container, orig_container)

/* swap a vtable slot (in __DATA_CONST, read-only after load) */
static process_fn patch(uintptr_t slot_addr, process_fn hook) {
    long page = sysconf(_SC_PAGESIZE);
    uintptr_t start = slot_addr & ~(uintptr_t)(page - 1);
    if (mprotect((void *)start, (size_t)page, PROT_READ | PROT_WRITE) != 0) { perror("mprotect"); exit(1); }
    process_fn old = *(process_fn *)slot_addr;
    *(process_fn *)slot_addr = hook;
    mprotect((void *)start, (size_t)page, PROT_READ);
    return old;
}

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
    if (argc < 3 || argc > 4 || (argc == 4 && strcmp(argv[3], "--log-io") != 0)) {
        fprintf(stderr, "usage: %s <model.nam> <outdir> [--log-io]\n", argv[0]);
        return 2;
    }
    outdir = argv[2];
    mkdir(outdir, 0755);

    if (!dlopen(IMAGELIB, RTLD_NOW)) { fprintf(stderr, "dlopen: %s\n", dlerror()); return 1; }
    for (uint32_t i = 0; i < _dyld_image_count(); i++)
        if (strstr(_dyld_get_image_name(i), "imagelib.framework/")) base = (uintptr_t)_dyld_get_image_header(i);
    if (!base) { fprintf(stderr, "imagelib not found among loaded images\n"); return 1; }
    printf("imagelib loaded at %#lx\n", (unsigned long)base);

    if (argc == 4) {
        char path[4096];
        snprintf(path, sizeof path, "%s/model_io.bin", outdir);
        if (!(iolog = fopen(path, "wb"))) { perror(path); return 1; }
        orig_slim = patch(base + OFF_vtable_SlimmableWavenet + VT_PROCESS_SLOT, hook_slim);
        orig_wavenet = patch(base + OFF_vtable_WaveNet + VT_PROCESS_SLOT, hook_wavenet);
        orig_container = patch(base + OFF_vtable_ContainerModel + VT_PROCESS_SLOT, hook_container);
        printf("logging model I/O to %s\n", path);
    }

    ((void (*)(void (*)(char *)))(base + OFF_setNamInfoCallBack))(on_info);
    ((void (*)(void (*)(float *, float *, float *)))(base + OFF_setLoadNamEndCallBack))(on_end);

    /* the API takes a wide-character path */
    size_t n = mbstowcs(NULL, argv[1], 0);
    wchar_t *wpath = calloc(n + 1, sizeof(wchar_t));
    mbstowcs(wpath, argv[1], n + 1);
    printf("converting %s …\n", argv[1]);
    ((void (*)(const wchar_t *))(base + OFF_loadNamFile))(wpath);

    {
        void *model = *(void **)(base + 0x1262b0); /* ImageUtility::namModel */
        if (model) {
            uintptr_t vptr = *(uintptr_t *)model;
            uintptr_t proc = *(uintptr_t *)(vptr + 0x18);
            printf("model %p: vtable at image+%#lx, process at image+%#lx\n", model,
                   (unsigned long)(vptr - base), (unsigned long)(proc - base));
        }
    }

    /* the end callback is delivered through JUCE's message queue on the main run loop */
    for (int s = 0; s < 300 && !finished; s++) CFRunLoopRunInMode(kCFRunLoopDefaultMode, 1.0, false);
    if (!finished) fprintf(stderr, "no end callback after 300 s; dumping buffers anyway\n");
    if (iolog) fclose(iolog);
    printf("buffers:\n");
    dump_buffers();
    return finished ? 0 : 1;
}
