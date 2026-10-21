import sys, struct, json
# Flutter StandardMessageCodec decoder
def rd_size(b,i):
    v=b[i]; i+=1
    if v<254: return v,i
    if v==254: return struct.unpack_from('<H',b,i)[0],i+2
    return struct.unpack_from('<I',b,i)[0],i+4
def align(i,n): return (i+n-1)//n*n
def rd(b,i):
    t=b[i]; i+=1
    if t==0: return None,i
    if t==1: return True,i
    if t==2: return False,i
    if t==3: return struct.unpack_from('<i',b,i)[0],i+4
    if t==4: return struct.unpack_from('<q',b,i)[0],i+8
    if t==6:
        i=align(i,8); return struct.unpack_from('<d',b,i)[0],i+8
    if t in (5,7):
        n,i=rd_size(b,i); return b[i:i+n].decode('utf8'),i+n
    if t==8:
        n,i=rd_size(b,i); return list(b[i:i+n]),i+n
    if t==12:
        n,i=rd_size(b,i); out=[]
        for _ in range(n):
            v,i=rd(b,i); out.append(v)
        return out,i
    if t==13:
        n,i=rd_size(b,i); out={}
        for _ in range(n):
            k,i=rd(b,i); v,i=rd(b,i); out[k]=v
        return out,i
    raise ValueError('type %d at %d'%(t,i-1))
b=open(sys.argv[1],'rb').read()
v,_=rd(b,0)
print(json.dumps(v,indent=1,ensure_ascii=False))
