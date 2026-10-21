# For an address inside a function, find the function start (prologue) and list call sites with context.
import re,sys,bisect
f=sys.argv[1]; addr=int(sys.argv[2],16); ctx=int(sys.argv[3]) if len(sys.argv)>3 else 25
lines=open(f).read().split('\n')
rx=re.compile(r'\s*([0-9a-f]+): (\S+)\s+(.*)')
addrs=[];starts=[]
for i,l in enumerate(lines):
    m=rx.match(l)
    if m:
        a=int(m.group(1),16); addrs.append((a,i))
        if m.group(2)=='stp' and m.group(3).startswith('x29, x30, [x15, #-0x10]!'): starts.append(a)
st=starts[bisect.bisect_right(starts,addr)-1]
print('function start %#x'%st)
pat='bl       %#x '%st
for i,l in enumerate(lines):
    if pat in l:
        cs=rx.match(l).group(1)
        fs=starts[bisect.bisect_right(starts,int(cs,16))-1]
        print('--- call at %s (in func %#x)'%(cs,fs))
        for j in range(max(0,i-ctx),i+1):
            s=lines[j]
            if re.search(r'String:|mov +x\d+, #|List\(|stp +x|bl ',s): print('   ',s.strip()[:150])
