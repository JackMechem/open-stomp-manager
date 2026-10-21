# Scan annotated Dart AOT disassembly for inline list literals built from small-int constants:
# AllocateArrayStub followed by "mov x16,#imm ; stur x16,[x0,#0x17+8k]" (Smi-tagged values)
import re,sys
lines=open(sys.argv[1]).read().split('\n')
rx=re.compile(r'\s*([0-9a-f]+): (\S+)\s+(.*)')
out=[]
cur_func='?'
for i,l in enumerate(lines):
    if l.startswith(';;;;; FUNC'): cur_func=l.split()[2]
    if 'AllocateArrayStub' in l or 'AllocateUint8ArrayStub' in l:
        kind='Array' if 'AllocateArrayStub' in l else 'Uint8'
        vals={}; regs={}
        for j in range(i+1,min(i+400,len(lines))):
            m=rx.match(lines[j])
            if not m: break
            op,args=m.group(2),m.group(3)
            if op in ('bl','blr','ret','b') : break
            mm=re.match(r'(x\d+|w\d+), #(0x[0-9a-f]+|\d+)',args)
            if op=='mov' and mm: regs[mm.group(1)[1:]]=int(mm.group(2),0); continue
            if op=='mov' and re.match(r'x\d+, xzr',args): regs[args.split(',')[0][1:]]=0; continue
            ms=re.match(r'(x\d+|xzr), \[x0, #0x([0-9a-f]+)\]',args)
            if op=='stur' and ms and kind=='Array':
                r=ms.group(1); off=int(ms.group(2),16)
                v=0 if r=='xzr' else regs.get(r[1:])
                if v is not None and v%2==0: vals[(off-0x17)//8]=v//2
                else: vals[(off-0x17)//8]=None
            ms=re.match(r'(w\d+|wzr), \[x\d+, #0x([0-9a-f]+)\]',args)
            if op in('strb','sturb') and ms:
                r=ms.group(1); v=0 if r=='wzr' else regs.get(r[1:])
                vals[int(ms.group(2),16)]=v
        known=[v for v in vals.values() if v is not None]
        if len(known)>=2:
            addr=rx.match(l).group(1)
            seq=[vals.get(k) for k in range(max(vals)+1)] if vals else []
            out.append(f'{addr} {kind} {cur_func}: '+' '.join('??' if v is None else hex(v) for v in seq))
print('\n'.join(out))
