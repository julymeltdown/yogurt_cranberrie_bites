import{candidateServer}from'./server.mjs'
const server=candidateServer();await new Promise(r=>server.listen(0,'127.0.0.1',r));const base=`http://127.0.0.1:${server.address().port}`
console.log('Combined preset/wardrobe fixture: '+base+'/tools/wardrobe-review.html')
console.log('Preset direction fixture: '+base+'/tools/direction-review.html')
console.log('Original extraction fixture: '+base+'/tools/review.html')
for(const signal of['SIGINT','SIGTERM'])process.on(signal,()=>server.close(()=>process.exit(0)))
