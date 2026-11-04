import {test} from 'node:test'
import assert from 'node:assert/strict'
import {requiredRuntimeChecks,runtimeStatus,runtimeExit} from './status.mjs'
test('empty and partial evidence cannot pass the original runtime gate',()=>{assert.equal(runtimeStatus({}),'NOT_RUN');assert.equal(runtimeExit({} ),1);assert.equal(runtimeStatus({independentAccounts:{status:'PASS'}}),'NOT_RUN')})
test('every original runtime criterion is required',()=>{for(const missing of requiredRuntimeChecks){const checks=Object.fromEntries(requiredRuntimeChecks.filter(k=>k!==missing).map(k=>[k,{status:'PASS'}]));assert.equal(runtimeExit(checks),1)}})
test('a service or runtime failure remains nonzero even with other passing evidence',()=>{const checks=Object.fromEntries(requiredRuntimeChecks.map(k=>[k,{status:'PASS'}]));checks.independentAccounts={status:'FAIL'};assert.equal(runtimeStatus(checks),'FAIL');assert.equal(runtimeExit(checks),1)})
test('complete runtime evidence keeps art inspection explicit',()=>{const checks=Object.fromEntries(requiredRuntimeChecks.map(k=>[k,{status:'PASS'}]));assert.equal(runtimeStatus(checks),'RUNTIME_PASS_PENDING_ART_REVIEW');assert.equal(runtimeExit(checks),0)})

test('late browser, stream or resource faults cannot certify an otherwise complete run',()=>{const checks=Object.fromEntries(requiredRuntimeChecks.map(k=>[k,{status:'PASS'}]));assert.equal(runtimeStatus(checks,[{code:'RESOURCE_OR_LIVE_GUARD'}]),'FAIL');assert.equal(runtimeExit(checks,[{code:'PAGE_ERROR'}]),1)})
