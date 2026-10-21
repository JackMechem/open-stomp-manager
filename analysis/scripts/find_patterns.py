import sys,struct,re
d=open(sys.argv[1],'rb').read()
pats={
 'raw F0 43 58 70':bytes([0xF0,0x43,0x58,0x70]),
 'raw 43 58 70':bytes([0x43,0x58,0x70]),
 'raw F0 00 21':bytes([0xF0,0x00,0x21]),
}
for n,p in pats.items():
    offs=[m.start() for m in re.finditer(re.escape(p),d)]
    print(n,len(offs),[hex(o) for o in offs[:20]])
# smi-tagged (value<<1) 64-bit sequences
def smiseq(vals): return b''.join(struct.pack('<q',v<<1) for v in vals)
for vals in ([0xF0,0x43,0x58,0x70],[0x43,0x58,0x70],[0xF0],[0xF7]):
    p=smiseq(vals); offs=[m.start() for m in re.finditer(re.escape(p),d)]
    print('smi',vals,len(offs),[hex(o) for o in offs[:10]])
