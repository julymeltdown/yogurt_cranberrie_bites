// Root's literal single-fixture exception. Stored shared sources stay unchanged.
import assert from 'node:assert/strict'
import {createHash} from 'node:crypto'
const hash=bytes=>createHash('sha256').update(bytes).digest('hex')
export function transformAdmission(preimage){
 assert.equal(hash(preimage),'e2af02c405caef3143e7cb0c5eddc69b2ecdf14a5bf6e27d91dce03b2c5376ee','Exact Root-granted Wall fixture preimage')
 const source=preimage.toString(),site="  requests.push({ path, method: request.method, key: request.headers.get('Idempotency-Key'), authenticated: !!request.headers.get('Authorization') })"
 assert.equal(source.split(site).length,2,'Exactly one authorized insertion site')
 const prefix=`import { admission } from '/src/features/auth/__tests__/enrollment-consent/fixtures'
const pi09AdmissionRequests: {method:string;protected:boolean;status:number;response:ReturnType<typeof admission>|null;atOriginalRequestIndex:number}[]=[]
Object.assign(window,{__PI09_ADMISSION_AUDIT__:pi09AdmissionRequests})
`
 const branch=`  if (url.origin === location.origin && path === '/api/auth/enrollment/admission') {
    const protectedRequest = request.headers.get('Authorization') === 'Bearer ' + authResponse().accessToken
    const status = request.method !== 'GET' ? 405 : protectedRequest ? 200 : 401
    const response = status === 200 ? admission() : null
    pi09AdmissionRequests.push({method:request.method,protected:protectedRequest,status,response,atOriginalRequestIndex:requests.length})
    return response ? Response.json(response) : new Response(null,{status})
  }
`
 const code=prefix+source.replace(site,branch+site)
 return {code,readset:{preimageSha256:hash(preimage),postimageSha256:hash(code),prefix,insertBefore:site,branch,storedFixtureChanged:false,qualification:'TEST_ONLY_SYNTHETIC_ADMITTED_HTTP_RESPONSE_NOT_LIVE'}}
}
