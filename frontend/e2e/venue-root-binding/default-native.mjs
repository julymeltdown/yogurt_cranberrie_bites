import assert from 'node:assert/strict'
import { createRequire } from 'node:module'
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { pathToFileURL } from 'node:url'
import { createHash } from 'node:crypto'
import { defaultInputs, frontend, asset, canonical, pins, consumed } from './default-readset.mjs'
import { aliases } from '../../src/features/admin/service-policy/__tests__/vitest.config.mjs'
const dependencies=process.env.MCR_FRONTEND_DEPENDENCIES,scratch=process.env.MCR_FE38_SCRATCH,output=process.env.MCR_FE38_OUTPUT
if(!dependencies||!scratch||!output)throw Error('Fresh accepted default native allocation required')
const require=createRequire(resolve(dependencies,'package.json')), {chromium,expect}=require('@playwright/test')
// Resolve the retained subpath's declared ESM export without a dependency link.
const resolverPackage=require.resolve('@hookform/resolvers/package.json')
const resolverManifest=JSON.parse(readFileSync(resolverPackage,'utf8'))
const resolverESM=resolve(resolverPackage,'..',resolverManifest.exports['./zod'].import)
const nativeAliases=[{find:/^@hookform\/resolvers\/zod$/,replacement:resolverESM},...aliases]
const release={version:1,applicationVersion:'0.1.0',buildId:'a'.repeat(64),sourceRevision:'b'.repeat(40)}
const input={revision:'TEST_ONLY_REVIEWED_ROOT_not_C05_authority',reviewedBy:'Synthetic default-entry fixture',movementPolicy:{speedPxPerSecond:96,footWidthPx:12,footHeightPx:8,interactionRadiusPx:96},zoom:1}
const schema=JSON.parse(asset('contracts/assets/v2/reviewed/avatar-manifest.schema.json'))
const bootstrap=JSON.parse(asset('contracts/fixtures/core/bootstrap-open.json'))
const durable={avatar:{revision:7,config:schema.$defs.ReviewedMotionAvatarManifest.const.presets[0].config},participantRevision:bootstrap.participant.revision}
const report={classification:'ACTUAL_DEFAULT_MAIN_CONTROLLED_HTTP_C05_REVIEWED_CHARACTER_ART_NOT_LIVE_DEVICE_OR_WHOLE_VENU',canonical,widths:[],pageErrors:[],resourceFailures:[],screenshots:[],comparisons:[],requests:[],started:new Date().toISOString()}
report.retainedDependencyAlias={specifier:'@hookform/resolvers/zod',packageVersion:resolverManifest.version,packageSHA256:createHash('sha256').update(readFileSync(resolverPackage)).digest('hex'),esmSHA256:createHash('sha256').update(readFileSync(resolverESM)).digest('hex')}
mkdirSync(output,{recursive:true});let server,browser,page,stage='startup'
const shot=async(label,locator)=>{const file=resolve(output,label+'.png');await(locator??page).screenshot(locator?{path:file}:{path:file,fullPage:true});report.screenshots.push({path:file,capturedAt:new Date().toISOString(),timezone:'UTC',fixture:report.classification});return file}
try{
 const {createServer}=await import(pathToFileURL(require.resolve('vite'))),{default:react}=await import(pathToFileURL(require.resolve('@vitejs/plugin-react')))
 server=await createServer({configFile:false,root:frontend,plugins:[defaultInputs(),react(),{name:'default-main-http-fixture',configureServer(s){s.middlewares.use(async(req,res,next)=>{
   if(req.url==='/__reviewed_persistence_motion/state'){res.setHeader('Content-Type','application/json');res.setHeader('Cache-Control','no-store');if(req.method!=='GET'){res.statusCode=405;res.end('{}')}else res.end(JSON.stringify(durable));return}
   if(!req.headers.accept?.includes('text/html'))return next()
   res.setHeader('Content-Type','text/html');res.end(await s.transformIndexHtml(req.url,readFileSync(resolve(frontend,'e2e/venue-root-binding/default-native.html'),'utf8')))
 })}}],resolve:{alias:nativeAliases},optimizeDeps:{include:['@hookform/resolvers/zod','react-hook-form','zustand','browser-image-compression','react/jsx-runtime','react/jsx-dev-runtime']},cacheDir:resolve(scratch,'default-venue-vite'),define:{'import.meta.env.VITE_MCR_EVENT_ID':JSON.stringify('00000000-0000-4000-8000-000000000201'),'import.meta.env.VITE_MCR_LOCAL_INPUT_CONFIG':JSON.stringify(JSON.stringify(input)),'import.meta.env.VITE_MCR_APPLICATION_RELEASE':JSON.stringify(release)},server:{host:'127.0.0.1',port:5384,strictPort:true,watch:null,fs:{allow:[frontend,dependencies]}}})
 await server.listen();browser=await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true,args:['--disable-background-networking','--disable-component-update','--disable-default-apps','--no-first-run','--disable-crash-reporter']});report.browser=browser.version()
 for(const width of [360,390,430]){
  const context=await browser.newContext({viewport:{width,height:844},deviceScaleFactor:1,locale:'en-GB',timezoneId:'Asia/Seoul',reducedMotion:'reduce'});page=await context.newPage();page.setDefaultTimeout(12000);page.on('pageerror',e=>report.pageErrors.push(String(e)))
  page.on('requestfailed',r=>report.resourceFailures.push({width,url:r.url(),failure:r.failure()?.errorText}))
  page.on('response',r=>{if(r.status()>=400)report.resourceFailures.push({width,url:r.url(),status:r.status()})})
  page.on('request',r=>{if(r.url().includes('/venue/paradise-city/'))report.requests.push({width,url:r.url(),method:r.method()})})
  const state=()=>page.evaluate(()=>window.__REVIEWED_ROOT_ENTRY__.state())
  const proof=()=>page.evaluate(()=>window.__REVIEWED_ROOT_ENTRY__.worldProof())
  const nav=page.getByRole('navigation',{name:'Main navigation'})
  const openMap=async()=>{const link=page.getByRole('navigation',{name:'World tools',exact:true}).getByRole('link',{name:'Map',exact:true});if(!await link.isVisible())await page.getByRole('button',{name:'World destinations',exact:true}).click();await link.click()}
  stage=width+'/actual-ui-login';await page.goto('http://127.0.0.1:5384/login')
  await page.getByLabel('Email',{exact:true}).fill('fixture@example.test');await page.getByLabel('Password',{exact:true}).fill('fixture-password');await page.getByRole('button',{name:'Sign in with email',exact:true}).click()
  await expect.poll(async()=>{try{return(await proof()).actors.filter(a=>a.visible&&a.cell.alpha>0).length}catch{return 0}},{timeout:20000}).toBeGreaterThan(1)
  const initial=await state(),world=await proof();assert.equal(initial.peak,1);assert.equal(initial.active,1);assert.equal(world.canvasCount,1)
  for(const actor of world.actors){assert.equal(actor.sourceWidth,320);assert.equal(actor.sourceHeight,256);assert(actor.cell.alpha>0)}
  const canvas=await page.locator('.shell-world-host canvas').elementHandle()
  const worldCapture=await shot(width+'-default-world')
  stage=width+'/default-router-map';await openMap()
  await expect(page.getByRole('tab',{name:'MAP',exact:true})).toBeVisible();await expect(page.locator('.pc-map-plane img')).toBeVisible()
  const image=()=>page.locator('.pc-map-plane img').evaluate(el=>({src:el.src,width:el.naturalWidth,height:el.naturalHeight,complete:el.complete}))
  const before=await image();assert(before.complete&&before.width===1024&&before.height===1280);assert(before.src.startsWith('blob:'))
  await expect(page.getByRole('button',{name:'Share location',exact:true})).toBeDisabled()
  await page.getByRole('button',{name:'Fit complete siteplan',exact:true}).click();if(width!==390)await shot(width+'-default-map');const mapImage=width===390?await shot('390-primary-image',page.locator('.pc-map-plane img')):null
  stage=width+'/place-focus';const first=page.locator('[data-landmark-id]').first();await first.click();await expect(page.locator('.pc-sheet')).toBeVisible()
  const place=await shot(width+'-default-place',width===390?page.locator('.pc-map'):undefined);await page.keyboard.press('Escape');await expect(page.locator('.pc-sheet')).toHaveCount(0)
  assert(await first.evaluate(el=>el===document.activeElement),'Focus did not return to selected real landmark')
  await page.getByRole('tab',{name:'FRIENDS',exact:true}).click();const friends=await shot(width+'-default-friends',width===390?page.locator('.pc-map'):undefined)
  await expect(page.getByText('Friends are unavailable until current location visibility is authorized.',{exact:true})).toBeVisible()
  stage=width+'/actual-route-back';await page.getByRole('button',{name:'Back to previous panel',exact:true}).click();await expect(nav).toBeVisible()
  await openMap();await expect(page.getByRole('tab',{name:'FRIENDS',exact:true})).toHaveAttribute('aria-selected','true')
  assert.deepEqual(await image(),before);assert(await canvas.evaluate(el=>el.isConnected&&document.querySelectorAll('.shell-world-host canvas').length===1),'Incumbent actual Phaser canvas remounted')
  const returned=await state();assert.equal(returned.peak,1);assert(returned.active<=1); // Full panels preserve canonical World pause/resume; no concurrent connection may duplicate.
  await page.getByRole('tab',{name:'NOTES',exact:true}).click();const notes=await shot(width+'-default-notes',width===390?page.locator('.pc-map'):undefined);await expect(page.getByRole('button',{name:'+ NEW NOTE',exact:true})).toBeDisabled()
  const ownedRequests=report.requests.filter(r=>r.width===width);assert.equal(ownedRequests.filter(r=>r.url.endsWith('asset-interface.json')).length,1);assert.equal(ownedRequests.filter(r=>r.url.endsWith('siteplan-photo-5.jpg')).length,1)
  if(width===390){
   // Exact developer-measured source ROIs; uniform cover + centre crop only.
   // Missing authorized social/meeting rows remain visible discrepancies, never masked.
   const sheets={reference:[],overlay:[],diff:[]}
   for(const [name,file,roi,actual] of [['photo-05.jpg',mapImage,[0,0,1024,1280],'primary-image'],['parade-ink.jpg',worldCapture,[436,61,397,785],'world-frame'],['photo-09.jpg',friends,[463,113,386,653],'friends'],['photo-10.jpg',notes,[32,112,386,656],'notes'],['photo-02.jpg',place,[445,109,386,657],'place']]){
    const source=readFileSync(resolve(frontend,'../docs/mcr/evidence/venue-root-binding/default/references',name)),capture=readFileSync(file)
    const result=await page.evaluate(async({source,capture,roi})=>{
      const load=src=>new Promise((yes,no)=>{const im=new Image();im.onload=()=>yes(im);im.onerror=no;im.src=src})
      const ref=await load(source),app=await load(capture),w=app.width,h=app.height,s=Math.max(w/roi[2],h/roi[3]),dw=roi[2]*s,dh=roi[3]*s
      const c=document.createElement('canvas');c.width=w;c.height=h;const x=c.getContext('2d');x.drawImage(ref,...roi,(w-dw)/2,(h-dh)/2,dw,dh);const reference=c.toDataURL('image/png'),a=x.getImageData(0,0,w,h)
      x.drawImage(app,0,0);x.globalAlpha=.5;x.drawImage(ref,...roi,(w-dw)/2,(h-dh)/2,dw,dh);x.globalAlpha=1;const overlay=c.toDataURL('image/png')
      x.drawImage(app,0,0);const b=x.getImageData(0,0,w,h);let sum=0;for(let i=0;i<a.data.length;i+=4){for(let k=0;k<3;k++){const d=Math.abs(a.data[i+k]-b.data[i+k]);sum+=d;b.data[i+k]=d}b.data[i+3]=255}x.putImageData(b,0,0)
      return{reference,overlay,diff:c.toDataURL('image/png'),uniformScale:s,output:{w,h},meanRGBAbsoluteDifference:sum/(w*h*3)}
    },{source:'data:image/jpeg;base64,'+source.toString('base64'),capture:'data:image/png;base64,'+capture.toString('base64'),roi})
    const paths={};for(const key of ['reference','overlay','diff']){const target=resolve(output,`390-comparison-${key}.png`);sheets[key].push({image:result[key],case:actual});paths[key]={path:target,row:sheets[key].length-1};delete result[key]}
    report.comparisons.push({case:actual,originalPhoto:name,originalPhotoSHA256:createHash('sha256').update(source).digest('hex'),developerMeasuredSourceROI:roi,actualViewport:{width,height:844,dpr:1},actualCapturePath:file,paths,...result,masks:[],status:actual==='primary-image'?'SAME_APPROVED_SOURCE_RESAMPLING_RECORDED':'MATERIAL_DIFFERENCES_OPEN',limitations:'Actual default shell, Photo5 base art; current clock/physical/Friends/Notes/Meeting authority absent. Reference populated states are anatomy targets; no fabricated data, acceptance or whole-photo identity.'})
   }
   for(const [key,rows] of Object.entries(sheets)){
    const sheet=await page.evaluate(async rows=>{const images=await Promise.all(rows.map(row=>new Promise((yes,no)=>{const image=new Image();image.onload=()=>yes(image);image.onerror=no;image.src=row.image})));const c=document.createElement('canvas');c.width=Math.max(...images.map(i=>i.width));c.height=images.reduce((v,i)=>v+i.height,0);const x=c.getContext('2d');x.fillStyle='#fff';x.fillRect(0,0,c.width,c.height);let y=0;const rois=[];for(let i=0;i<images.length;i++){const image=images[i];x.drawImage(image,0,y);rois.push({case:rows[i].case,x:0,y,width:image.width,height:image.height});y+=image.height}return{image:c.toDataURL('image/png'),rois}},rows)
    const file=resolve(output,`390-comparison-${key}.png`);writeFileSync(file,Buffer.from(sheet.image.split(',')[1],'base64'));report.comparisonSheets??=[];report.comparisonSheets.push({kind:key,path:file,rows:sheet.rois,createdAt:new Date().toISOString()})
   }
  }
  stage=width+'/canonical-signout';await page.getByRole('button',{name:'Back to previous panel',exact:true}).click();await nav.getByRole('link',{name:'Me',exact:true}).click();await page.getByText('Profile options',{exact:true}).click();await page.getByRole('link',{name:'Account',exact:true}).click();await page.getByRole('button',{name:'Sign out',exact:true}).click();await page.getByRole('button',{name:'Confirm sign out',exact:true}).click()
  await expect.poll(async()=>(await state()).canvases).toBe(0);await expect.poll(async()=>(await state()).active).toBe(0);await expect.poll(async()=>(await state()).blobURLs).toBe(0)
  await expect(page.locator('.pc-map-plane img')).toHaveCount(0)
  report.widths.push({width,height:844,dpr:1,actualDefaultMain:true,uiLogin:true,world,initial,returned,image:before,sourceImageRequests:ownedRequests,sameActualPhaserCanvas:true,singlePresencePeak:1,retainedFriends:true,focusReturn:true,canonicalSignoutOwnedZero:true})
  await context.close()
 }
 assert.deepEqual(report.pageErrors,[]);report.status='PASS_FUNCTIONAL_VISUAL_OPEN'
}catch(error){report.status='FAIL';report.failure=String(error.stack??error);report.stage=stage;if(page&&!page.isClosed()){report.failureState=await page.evaluate(()=>({url:location.href,text:document.body.innerText,state:window.__REVIEWED_ROOT_ENTRY__?.state()})).catch(()=>null);await shot('failure').catch(()=>{})}throw error}
finally{await browser?.close();await server?.close();report.pins=pins;report.consumed=[...consumed];report.ended=new Date().toISOString();writeFileSync(resolve(output,'report.json'),JSON.stringify(report,null,2)+'\n')}
