export const requiredRuntimeChecks=Object.freeze(['independentAccounts','authorizedWorld','bidirectionalMovementAndAck','persistentPanels','themeRuntimeAndLocalPersistence','customization','realBrowserReconnect','singleActualPhaserGame','finalWorldInvariants','sustainedCurrentAuthWorld'])
/** Runtime qualification is fail-closed; successful captures still need explicit art inspection. */
export function runtimeStatus(checks,faults=[]){
 if(faults.length)return 'FAIL'
 if(Object.values(checks).some(c=>c.status==='FAIL'))return 'FAIL'
 return requiredRuntimeChecks.every(k=>checks[k]?.status==='PASS')?'RUNTIME_PASS_PENDING_ART_REVIEW':'NOT_RUN'
}
export function runtimeExit(checks,faults=[]){return runtimeStatus(checks,faults)==='RUNTIME_PASS_PENDING_ART_REVIEW'?0:1}
