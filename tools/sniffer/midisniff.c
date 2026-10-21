// midisniff.dylib — CoreMIDI interposer for Stomp Manager.
// Logs every MIDI byte the app sends (TX) and receives (RX), with endpoint names
// and millisecond timestamps, to $SNIFF_LOG (one line per packet).
//
// Line format:
//   <unix_ms> <DIR> <len> [<endpoint name>] <hex bytes>
//   DIR: TX   = app -> device (MIDISend / MIDISendSysex / MIDISendEventList)
//        RX   = device -> app (input port read block)
//        VSRC = app emitting on its own virtual source (MIDIReceived)
//        VDST = something sending into the app's virtual destination
//        INFO = misc (port creation, connections)
//   Lines starting with '#' are comments / user marks.

#include <CoreMIDI/CoreMIDI.h>
#include <Block.h>
#include <fcntl.h>
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

#define DYLD_INTERPOSE(_replacement, _replacee)                                \
  __attribute__((used)) static struct {                                        \
    const void *replacement;                                                   \
    const void *replacee;                                                      \
  } _interpose_##_replacee __attribute__((section("__DATA,__interpose"))) = {  \
      (const void *)(unsigned long)&_replacement,                              \
      (const void *)(unsigned long)&_replacee};

static int g_fd = -1;
static pthread_mutex_t g_lock = PTHREAD_MUTEX_INITIALIZER;

// connRefCon -> source endpoint, so RX lines can be tagged with the source.
#define MAXCONN 64
static struct { void *refcon; MIDIEndpointRef ep; } g_conn[MAXCONN];
static int g_nconn = 0;

static long long now_ms(void) {
  struct timespec ts;
  clock_gettime(CLOCK_REALTIME, &ts);
  return (long long)ts.tv_sec * 1000 + ts.tv_nsec / 1000000;
}

static void ep_name(MIDIObjectRef obj, char *out, size_t n) {
  CFStringRef s = NULL;
  out[0] = 0;
  if (!obj) { snprintf(out, n, "?"); return; }
  if (MIDIObjectGetStringProperty(obj, kMIDIPropertyDisplayName, &s) != noErr || !s)
    MIDIObjectGetStringProperty(obj, kMIDIPropertyName, &s);
  if (s) {
    CFStringGetCString(s, out, (CFIndex)n, kCFStringEncodingUTF8);
    CFRelease(s);
  } else {
    snprintf(out, n, "ep#%u", (unsigned)obj);
  }
}

static void log_line(const char *dir, const char *name, const Byte *data, size_t len) {
  if (g_fd < 0) return;
  size_t cap = 96 + strlen(name) + len * 3;
  char *buf = malloc(cap);
  if (!buf) return;
  int off = snprintf(buf, cap, "%lld %s %zu [%s]", now_ms(), dir, len, name);
  for (size_t i = 0; i < len; i++) off += snprintf(buf + off, cap - off, " %02X", data[i]);
  buf[off++] = '\n';
  pthread_mutex_lock(&g_lock);
  write(g_fd, buf, off);
  pthread_mutex_unlock(&g_lock);
  free(buf);
}

static void log_info(const char *fmt, const char *a) {
  char msg[512];
  snprintf(msg, sizeof msg, fmt, a);
  log_line("INFO", msg, NULL, 0);
}

static void log_pktlist(const char *dir, MIDIEndpointRef ep, const MIDIPacketList *pl) {
  if (!pl) return;
  char name[256];
  ep_name(ep, name, sizeof name);
  const MIDIPacket *p = &pl->packet[0];
  for (UInt32 i = 0; i < pl->numPackets; i++) {
    log_line(dir, name, p->data, p->length);
    p = MIDIPacketNext(p);
  }
}

static void log_eventlist(const char *dir, MIDIEndpointRef ep, const MIDIEventList *el) {
  if (!el) return;
  char name[256];
  ep_name(ep, name, sizeof name);
  const MIDIEventPacket *p = &el->packet[0];
  for (UInt32 i = 0; i < el->numPackets; i++) {
    // UMP words, logged as big-endian 32-bit words
    Byte tmp[256 * 4];
    UInt32 n = p->wordCount > 256 ? 256 : p->wordCount;
    for (UInt32 w = 0; w < n; w++) {
      tmp[w * 4 + 0] = p->words[w] >> 24;
      tmp[w * 4 + 1] = p->words[w] >> 16;
      tmp[w * 4 + 2] = p->words[w] >> 8;
      tmp[w * 4 + 3] = p->words[w];
    }
    char d[16];
    snprintf(d, sizeof d, "%s-UMP", dir);
    log_line(d, name, tmp, n * 4);
    p = MIDIEventPacketNext(p);
  }
}

static MIDIEndpointRef lookup_src(void *refcon) {
  MIDIEndpointRef ep = 0;
  pthread_mutex_lock(&g_lock);
  for (int i = 0; i < g_nconn; i++)
    if (g_conn[i].refcon == refcon) { ep = g_conn[i].ep; break; }
  pthread_mutex_unlock(&g_lock);
  return ep;
}

// ---- send side ----
static OSStatus my_MIDISend(MIDIPortRef port, MIDIEndpointRef dest, const MIDIPacketList *pl) {
  log_pktlist("TX", dest, pl);
  return MIDISend(port, dest, pl);
}
DYLD_INTERPOSE(my_MIDISend, MIDISend)

static OSStatus my_MIDISendEventList(MIDIPortRef port, MIDIEndpointRef dest, const MIDIEventList *el) {
  log_eventlist("TX", dest, el);
  return MIDISendEventList(port, dest, el);
}
DYLD_INTERPOSE(my_MIDISendEventList, MIDISendEventList)

static OSStatus my_MIDISendSysex(MIDISysexSendRequest *req) {
  if (req) {
    char name[256];
    ep_name(req->destination, name, sizeof name);
    log_line("TX", name, req->data, req->bytesToSend);
  }
  return MIDISendSysex(req);
}
DYLD_INTERPOSE(my_MIDISendSysex, MIDISendSysex)

static OSStatus my_MIDIReceived(MIDIEndpointRef src, const MIDIPacketList *pl) {
  log_pktlist("VSRC", src, pl);
  return MIDIReceived(src, pl);
}
DYLD_INTERPOSE(my_MIDIReceived, MIDIReceived)

// ---- receive side ----
static OSStatus my_MIDIPortConnectSource(MIDIPortRef port, MIDIEndpointRef src, void *refcon) {
  char name[256];
  ep_name(src, name, sizeof name);
  log_info("connect source: %s", name);
  pthread_mutex_lock(&g_lock);
  if (g_nconn < MAXCONN) { g_conn[g_nconn].refcon = refcon; g_conn[g_nconn].ep = src; g_nconn++; }
  pthread_mutex_unlock(&g_lock);
  return MIDIPortConnectSource(port, src, refcon);
}
DYLD_INTERPOSE(my_MIDIPortConnectSource, MIDIPortConnectSource)

static OSStatus my_MIDIInputPortCreateWithBlock(MIDIClientRef c, CFStringRef name, MIDIPortRef *out,
                                                MIDIReadBlock blk) {
  log_info("%s", "MIDIInputPortCreateWithBlock");
  MIDIReadBlock orig = Block_copy(blk);
  MIDIReadBlock wrap = ^(const MIDIPacketList *pl, void *refcon) {
    log_pktlist("RX", lookup_src(refcon), pl);
    orig(pl, refcon);
  };
  return MIDIInputPortCreateWithBlock(c, name, out, Block_copy(wrap));
}
DYLD_INTERPOSE(my_MIDIInputPortCreateWithBlock, MIDIInputPortCreateWithBlock)

static OSStatus my_MIDIInputPortCreateWithProtocol(MIDIClientRef c, CFStringRef name, MIDIProtocolID proto,
                                                   MIDIPortRef *out, MIDIReceiveBlock blk) {
  log_info("%s", "MIDIInputPortCreateWithProtocol");
  MIDIReceiveBlock orig = Block_copy(blk);
  MIDIReceiveBlock wrap = ^(const MIDIEventList *el, void *refcon) {
    log_eventlist("RX", lookup_src(refcon), el);
    orig(el, refcon);
  };
  return MIDIInputPortCreateWithProtocol(c, name, proto, out, Block_copy(wrap));
}
DYLD_INTERPOSE(my_MIDIInputPortCreateWithProtocol, MIDIInputPortCreateWithProtocol)

typedef struct { MIDIReadProc proc; void *refcon; } procwrap;
static void wrapped_readproc(const MIDIPacketList *pl, void *readRefCon, void *srcRefCon) {
  procwrap *w = readRefCon;
  log_pktlist("RX", lookup_src(srcRefCon), pl);
  w->proc(pl, w->refcon, srcRefCon);
}
static OSStatus my_MIDIInputPortCreate(MIDIClientRef c, CFStringRef name, MIDIReadProc proc, void *refcon,
                                       MIDIPortRef *out) {
  log_info("%s", "MIDIInputPortCreate");
  procwrap *w = malloc(sizeof *w);
  w->proc = proc;
  w->refcon = refcon;
  return MIDIInputPortCreate(c, name, wrapped_readproc, w, out);
}
DYLD_INTERPOSE(my_MIDIInputPortCreate, MIDIInputPortCreate)

static OSStatus my_MIDIDestinationCreateWithBlock(MIDIClientRef c, CFStringRef name, MIDIEndpointRef *out,
                                                  MIDIReadBlock blk) {
  log_info("%s", "MIDIDestinationCreateWithBlock (virtual dest)");
  MIDIReadBlock orig = Block_copy(blk);
  MIDIEndpointRef *outp = out;
  __block MIDIEndpointRef self = 0;
  MIDIReadBlock wrap = ^(const MIDIPacketList *pl, void *refcon) {
    log_pktlist("VDST", self, pl);
    orig(pl, refcon);
  };
  OSStatus st = MIDIDestinationCreateWithBlock(c, name, outp, Block_copy(wrap));
  if (outp) self = *outp;
  return st;
}
DYLD_INTERPOSE(my_MIDIDestinationCreateWithBlock, MIDIDestinationCreateWithBlock)

__attribute__((constructor)) static void init(void) {
  const char *path = getenv("SNIFF_LOG");
  char def[1024];
  if (!path) {
    snprintf(def, sizeof def, "/tmp/midisniff-%lld.log", now_ms());
    path = def;
  }
  g_fd = open(path, O_WRONLY | O_CREAT | O_APPEND, 0644);
  if (g_fd >= 0) {
    char hdr[1200];
    int n = snprintf(hdr, sizeof hdr, "# midisniff attached pid=%d at %lld -> %s\n", getpid(), now_ms(), path);
    write(g_fd, hdr, n);
  }
}
