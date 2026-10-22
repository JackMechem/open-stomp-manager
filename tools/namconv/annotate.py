#!/usr/bin/env python3
"""Annotate an arm64 llvm-objdump listing: resolve adrp+add/ldr/str pairs to symbol names and
report which functions touch which globals.

usage: annotate.py <listing.dis> <nm -n output> [out.dis]       (prints an xref table)
"""
import bisect
import re
import subprocess
import sys
from collections import defaultdict


def load_syms(nm_path):
    syms = []
    for line in open(nm_path):
        parts = line.split()
        if len(parts) == 3:
            syms.append((int(parts[0], 16), parts[2]))
    syms.sort()
    names = [s[1] for s in syms]
    demangled = subprocess.run(["c++filt"], input="\n".join(names), capture_output=True, text=True).stdout.split("\n")
    return [(a, d or n) for (a, n), d in zip(syms, demangled)]


def main():
    dis, nmf = sys.argv[1], sys.argv[2]
    out = open(sys.argv[3], "w") if len(sys.argv) > 3 else None
    syms = load_syms(nmf)
    addrs = [a for a, _ in syms]

    def sym_at(a):
        i = bisect.bisect_right(addrs, a) - 1
        if i < 0:
            return None
        base, name = syms[i]
        return name if a == base else f"{name}+{a - base:#x}"

    pages = {}
    func = None
    xrefs = defaultdict(set)
    adrp = re.compile(r"adrp\s+(x\d+), (0x[0-9a-f]+)")
    use = re.compile(r"\b(?:add|ldr|ldrsw|str|ldp|stp|ldur|ldrb|strb)\s+\w+,\s*(?:\w+,\s*)?\[?(x\d+)(?:,\s*#?(0x[0-9a-f]+|\d+))?")
    addimm = re.compile(r"add\s+(x\d+), (x\d+), #(0x[0-9a-f]+|\d+)")
    for line in open(dis):
        m = re.match(r"^([0-9a-f]{16}) <(.+)>:", line)
        if m:
            func = m.group(2)
            pages.clear()
        note = ""
        m = adrp.search(line)
        if m:
            pages[m.group(1)] = int(m.group(2), 16)
        else:
            a = addimm.search(line)
            if a and a.group(2) in pages:
                tgt = pages[a.group(2)] + int(a.group(3), 0)
                s = sym_at(tgt)
                if s:
                    note = f"  ; {s}"
                    xrefs[s.split("+")[0]].add(func)
                pages[a.group(1)] = tgt if a.group(1) != a.group(2) else pages[a.group(2)]
            else:
                u = use.search(line)
                if u and u.group(1) in pages:
                    tgt = pages[u.group(1)] + (int(u.group(2), 0) if u.group(2) else 0)
                    s = sym_at(tgt)
                    if s:
                        note = f"  ; {s}"
                        xrefs[s.split("+")[0]].add(func)
        m = re.search(r"\bbl\s+(0x[0-9a-f]+)", line)
        if m:
            s = sym_at(int(m.group(1), 16))
            if s:
                note = f"  ; {s}"
        if out:
            out.write(line.rstrip("\n") + note + "\n")
    for g in sys.argv[4:] if len(sys.argv) > 4 else []:
        pass
    return xrefs


if __name__ == "__main__":
    x = main()
    want = ["ToneTest", "g1GainsDb", "NamFir", "G1IR", "G2IR", "ampCapture", "levelsTester", "mlCapture", "BLA_x",
            "Magnitude2MinPhase", "hilbert2", "getSpectrum", "FIRExecObj", "ImageUtility::nam"]
    for k in sorted(x):
        if any(w in k for w in want):
            print(f"{k}:")
            for f in sorted(x[k]):
                print(f"    {f}")
