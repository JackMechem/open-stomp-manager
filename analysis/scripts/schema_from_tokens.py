import re,sys,json
t=open(sys.argv[1]).read()
t=re.sub(r'<TypeArguments: [^>]*>+','',t)
t=re.sub(r'\{Allocate[^}]*\}','',t)
toks=re.findall(r'"(?:[^"\\]|\\.)*"|#\w+|\{Map\}',t)
structs=[];cur=None;pm={};key=None;i=0
def val(x): return int(x[1:]) if x.startswith('#') and x[1:].isdigit() else x.strip('"')
while i<len(toks):
    tk=toks[i]
    if tk=='"id"' and i+4<len(toks) and toks[i+2]=='"name"' and toks[i+4]=='"params"':
        cur={'id':val(toks[i+1]),'name':val(toks[i+3]),'params':[]}; structs.append(cur); pm={}; i+=5; continue
    if tk=='{Map}':
        if pm and cur is not None and 'type' in pm: cur['params'].append(pm)
        pm={}; i+=1; continue
    if tk.startswith('"') and i+1<len(toks) and tk.strip('"') in ('type','name','unit','size','bits','id'):
        pm[tk.strip('"')]=val(toks[i+1]); i+=2; continue
    i+=1
for s in structs:
    print(f"struct {s['id']} {s['name']}:")
    off=0
    for p in s['params']:
        d=p.get('type')
        if d=='Array': d=f"{p.get('unit')}[{p.get('size')}]"
        if d=='struct': d=f"struct {p.get('id')}"+(f" x{p['size']}" if 'size' in p else '')
        b=f" bits={p['bits']}" if 'bits' in p else ''
        print(f"    {p.get('name','?'):16s} {d}{b}")
json.dump(structs,open(sys.argv[2],'w'),indent=1)
