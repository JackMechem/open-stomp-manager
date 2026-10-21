# Print a compact token stream (pool strings / ints / small immediates / calls) for an address range
# of the annotated Dart disassembly. Usage: token_stream.py annotated.txt start_hex end_hex
import re,sys
f,s,e=sys.argv[1],int(sys.argv[2],16),int(sys.argv[3],16)
rx=re.compile(r'\s*([0-9a-f]+): (\S+)\s+(.*)')
toks=[]
for l in open(f):
    m=rx.match(l)
    if not m: continue
    a=int(m.group(1),16)
    if a<s: continue
    if a>=e: break
    op,args=m.group(2),m.group(3)
    if '; String: ' in args: toks.append(args.split('; String: ',1)[1])
    elif '; ' in args and op=='ldr' and ('List' in args or 'Obj!' in args or 'Type' in args): toks.append('<'+args.split('; ',1)[1][:60]+'>')
    elif op=='mov':
        mm=re.match(r'x16, #(0x[0-9a-f]+|\d+)',args)
        if mm:
            v=int(mm.group(1),0); toks.append('#%d'%(v//2) if v%2==0 else '#raw%d'%v)
    elif op=='stur' and args.startswith('xzr, [x0'): toks.append('#0')
    elif op=='bl' and 'Allocate' in args: toks.append('{'+args.split('; ')[-1].replace('Stub','')[:30]+'}')
    elif op=='bl' and 'Map::factory' in args: toks.append('{Map}')
print(' '.join(toks))
