# Extract (name, msgId=x3, dir=x5, other immediates) from call sites of NEP request builder functions.
import re,sys
f=sys.argv[1]; targets=[int(x,16) for x in sys.argv[2:]]
lines=open(f).read().split('\n')
rx=re.compile(r'\s*([0-9a-f]+): (\S+)\s+(.*)')
for i,l in enumerate(lines):
    for t in targets:
        if ('bl       %#x '%t) in l:
            regs={};name=None;strs=[];named=None;stack=[]
            for j in range(i-1,max(0,i-40),-1):
                m=rx.match(lines[j])
                if not m: continue
                op,args=m.group(2),m.group(3)
                if op=='stp' and args.startswith('x29, x30'): break
                if op=='bl' and j<i-1 and not 'Allocate' in args and 'Stub' not in args: break
                mm=re.match(r'(x\d+), #(0x[0-9a-f]+|\d+)',args)
                if op=='mov' and mm and mm.group(1) not in regs: regs[mm.group(1)]=int(mm.group(2),0)
                if 'String: ' in args and name is None and re.match(r'x2,',args): name=args.split('String: ')[1]
                elif 'String: ' in args: strs.append(args.split('String: ')[1])
                if 'List(' in args and re.match(r'x4,',args) and named is None: named=args.split('; ')[1]
            cs=rx.match(l).group(1)
            print(f'{cs} target={t:#x} name={name} x3(msgId)={regs.get("x3")} x5(dir)={regs.get("x5")} x16={regs.get("x16")} argdesc={named} otherstr={strs[:2]}')
