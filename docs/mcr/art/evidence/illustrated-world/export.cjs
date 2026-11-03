// Render editable SVG documents using native Chrome; preserve source PNG bytes.
const fs=require('node:fs/promises'),path=require('node:path'),crypto=require('node:crypto')
const root=path.resolve(__dirname,'../../../../..'),out=path.join(root,'frontend/public/world/illustrated')
const {chromium}=require(process.env.MCR_PLAYWRIGHT_MODULE||path.join(root,'frontend/node_modules/@playwright/test'))
const origin=process.env.MCR_ART_ORIGIN||'http://127.0.0.1:41865'
const sha=b=>'sha256:'+crypto.createHash('sha256').update(b).digest('hex')
;(async()=>{const browser=await chromium.launch({channel:'chrome',headless:true});const resources=[]
try{for(const theme of ['revenge','parade','danger'])for(const layer of ['ground','landmarks','scene','landmarks-atlas','terrain-tiles']){
 const name=`${theme}-${layer}`,width=layer==='landmarks-atlas'?768:layer==='terrain-tiles'?256:1152,height=layer==='landmarks-atlas'?512:layer==='terrain-tiles'?128:896
 const page=await browser.newPage({viewport:{width,height},deviceScaleFactor:1})
 await page.goto(`${origin}/frontend/public/world/illustrated/${name}.svg`)
 await page.evaluate(async()=>{await Promise.all([...document.querySelectorAll('image')].map(el=>new Promise((resolve,reject)=>{const im=new Image();im.onload=resolve;im.onerror=reject;im.src=el.href.baseVal})));await document.fonts.ready})
 await page.screenshot({path:path.join(out,`${name}.png`),omitBackground:true})
 const bytes=await fs.readFile(path.join(out,`${name}.png`));resources.push({path:`world/illustrated/${name}.png`,width_px:width,height_px:height,bytes:bytes.length,sha256:sha(bytes),source:`world/illustrated/${name}.svg`});await page.close()
}await fs.writeFile(path.join(out,'exported-resources.json'),JSON.stringify({kind:'ILLUSTRATED_EXPORT_SOURCE_RECEIPT',browser:browser.version(),resources},null,2)+'\n');console.log(`Exported ${resources.length} PNG layers/atlases from editable SVGs in Chrome ${browser.version()}`)
}finally{await browser.close()}})().catch(e=>{console.error(e);process.exitCode=1})
