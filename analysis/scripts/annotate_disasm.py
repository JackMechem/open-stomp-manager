# Annotate llvm-objdump output of Dart AOT code with object-pool entries (from blutter pp.txt)
# and function names (from blutter ida_script/addNames.py). Usage: annotate_disasm.py disasm pp.txt addNames.py out
import re,sys
dis,ppf,namesf,out=sys.argv[1:5]
pp={}
for line in open(ppf,encoding='utf8',errors='replace'):
    m=re.match(r'\[pp\+0x([0-9a-f]+)\] (.*)',line)
    if m: pp[int(m.group(1),16)]=m.group(2)[:160]
names={}
for line in open(namesf,encoding='utf8',errors='replace'):
    m=re.match(r'idaapi\.set_name\(0x([0-9a-f]+), "(.*)"\)',line)
    if m and not m.group(2).endswith('_check'): names[int(m.group(1),16)]=m.group(2)
base={}
rx_line=re.compile(r'\s*([0-9a-f]+):\s+(\S+)\s*(.*)')
with open(out,'w') as o:
    for line in open(dis):
        m=rx_line.match(line)
        if not m:
            continue
        addr=int(m.group(1),16); op=m.group(2); args=m.group(3)
        ann=''
        if addr in names: o.write(f'\n;;;;; FUNC {names[addr]} @ {addr:#x}\n')
        mm=re.match(r'(x\d+), x27, #0x([0-9a-f]+), lsl #12',args)
        if op=='add' and mm:
            base[mm.group(1)]=int(mm.group(2),16)<<12
        else:
            mm=re.match(r'(x\d+), \[(x\d+), #0x([0-9a-f]+)\]',args)
            if op=='ldr' and mm:
                r=mm.group(2); off=int(mm.group(3),16)
                if r=='x27': ann=pp.get(off,'?pp+%#x'%off)
                elif r in base: ann=pp.get(base[r]+off,'?pp+%#x'%(base[r]+off))
            mm2=re.match(r'(x\d+), \[x27\]',args)
            mm3=re.match(r'(x\d+), x(\d+), #0x([0-9a-f]+)$',args)
            mi=re.match(r'([xw]\d+), #(-?0x[0-9a-f]+|-?\d+)$',args)
            if op in('mov','movz') and mi:
                v=int(mi.group(2),0)
                if v%2==0: ann='smi %d (0x%x)'%(v//2,v//2)
            mb=re.match(r'(0x[0-9a-f]+)',args)
            if op=='bl' and mb:
                t=int(mb.group(1),16); ann=names.get(t,'')
            # invalidate base if dest reg overwritten
            d=re.match(r'(x\d+),',args)
            if d and d.group(1) in base and not (op=='ldr' and mm and mm.group(2)==d.group(1)):
                pass
            if op=='ldr' and mm and mm.group(2) in base and mm.group(1)==mm.group(2):
                base.pop(mm.group(2),None)
        o.write(f'{addr:8x}: {op:8s} {args}' + (f'    ; {ann}' if ann else '') + '\n')
