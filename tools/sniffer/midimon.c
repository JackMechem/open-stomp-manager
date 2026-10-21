// midimon — standalone CoreMIDI tool.
//   midimon list                         list sources/destinations
//   midimon listen [filter] [logfile]    log everything from sources whose name contains filter
//   midimon send <filter> <hex...>       send raw bytes (e.g. "F0 00 ... F7") to a destination,
//                                        then print replies for 1.5 s
// Log line format matches midisniff (DIR = MON for passively observed device output).

#include <CoreMIDI/CoreMIDI.h>
#include <CoreFoundation/CoreFoundation.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>
#include <time.h>
#include <unistd.h>

static FILE *g_log;

static long long now_ms(void) {
  struct timespec ts;
  clock_gettime(CLOCK_REALTIME, &ts);
  return (long long)ts.tv_sec * 1000 + ts.tv_nsec / 1000000;
}

static void name_of(MIDIObjectRef o, char *out, size_t n) {
  CFStringRef s = NULL;
  out[0] = 0;
  if (MIDIObjectGetStringProperty(o, kMIDIPropertyDisplayName, &s) == noErr && s) {
    CFStringGetCString(s, out, (CFIndex)n, kCFStringEncodingUTF8);
    CFRelease(s);
  }
}

static void dump(const char *dir, const char *name, const MIDIPacketList *pl) {
  const MIDIPacket *p = &pl->packet[0];
  for (UInt32 i = 0; i < pl->numPackets; i++) {
    FILE *outs[2] = {stdout, g_log};
    for (int k = 0; k < 2; k++) {
      if (!outs[k]) continue;
      fprintf(outs[k], "%lld %s %u [%s]", now_ms(), dir, p->length, name);
      for (UInt16 j = 0; j < p->length; j++) fprintf(outs[k], " %02X", p->data[j]);
      fputc('\n', outs[k]);
      fflush(outs[k]);
    }
    p = MIDIPacketNext(p);
  }
}

static int contains(const char *hay, const char *needle) {
  if (!needle || !*needle) return 1;
  for (; *hay; hay++)
    if (strncasecmp(hay, needle, strlen(needle)) == 0) return 1;
  return 0;
}

static MIDIPortRef connect_sources(MIDIClientRef client, const char *filter) {
  MIDIPortRef in;
  MIDIInputPortCreateWithBlock(client, CFSTR("midimon-in"), &in, ^(const MIDIPacketList *pl, void *ref) {
    dump("MON", (const char *)ref, pl);
  });
  for (ItemCount i = 0; i < MIDIGetNumberOfSources(); i++) {
    MIDIEndpointRef src = MIDIGetSource(i);
    char *nm = malloc(256);
    name_of(src, nm, 256);
    if (contains(nm, filter)) {
      MIDIPortConnectSource(in, src, nm);
      fprintf(stderr, "listening: %s\n", nm);
    }
  }
  return in;
}

int main(int argc, char **argv) {
  if (argc < 2) {
    fprintf(stderr, "usage: midimon list | listen [filter] [logfile] | send <filter> <hex...>\n");
    return 1;
  }
  MIDIClientRef client;
  MIDIClientCreate(CFSTR("midimon"), NULL, NULL, &client);
  char nm[256];

  if (!strcmp(argv[1], "list")) {
    for (ItemCount i = 0; i < MIDIGetNumberOfSources(); i++) {
      name_of(MIDIGetSource(i), nm, sizeof nm);
      printf("source %lu: %s\n", (unsigned long)i, nm);
    }
    for (ItemCount i = 0; i < MIDIGetNumberOfDestinations(); i++) {
      name_of(MIDIGetDestination(i), nm, sizeof nm);
      printf("dest   %lu: %s\n", (unsigned long)i, nm);
    }
    return 0;
  }

  if (!strcmp(argv[1], "listen")) {
    const char *filter = argc > 2 ? argv[2] : "";
    if (argc > 3) g_log = fopen(argv[3], "a");
    connect_sources(client, filter);
    CFRunLoopRun();
    return 0;
  }

  if (!strcmp(argv[1], "send") && argc > 3) {
    const char *filter = argv[2];
    MIDIEndpointRef dest = 0;
    for (ItemCount i = 0; i < MIDIGetNumberOfDestinations(); i++) {
      name_of(MIDIGetDestination(i), nm, sizeof nm);
      if (contains(nm, filter)) { dest = MIDIGetDestination(i); break; }
    }
    if (!dest) { fprintf(stderr, "no destination matching '%s'\n", filter); return 1; }
    Byte data[4096];
    size_t n = 0;
    for (int a = 3; a < argc; a++) {
      char *s = argv[a];
      while (*s && n < sizeof data) {
        while (*s == ' ' || *s == ',') s++;
        if (!*s) break;
        data[n++] = (Byte)strtoul(s, &s, 16);
      }
    }
    connect_sources(client, filter);
    MIDIPortRef out;
    MIDIOutputPortCreate(client, CFSTR("midimon-out"), &out);
    Byte buf[8192];
    MIDIPacketList *pl = (MIDIPacketList *)buf;
    MIDIPacket *p = MIDIPacketListInit(pl);
    p = MIDIPacketListAdd(pl, sizeof buf, p, 0, n, data);
    printf("%lld SEND %zu [%s]", now_ms(), n, nm);
    for (size_t i = 0; i < n; i++) printf(" %02X", data[i]);
    printf("\n");
    MIDISend(out, dest, pl);
    CFRunLoopRunInMode(kCFRunLoopDefaultMode, 1.5, false);
    return 0;
  }
  fprintf(stderr, "bad args\n");
  return 1;
}
