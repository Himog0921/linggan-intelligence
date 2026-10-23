import { useSyncExternalStore } from 'react';
export interface ComparisonPayload {
  commandRef:string;targetRef:string;domainRef:string|null;workRefs:string[];
  knowledgeCutoff:string;expectedMaterialRevisions:string[];
  taskConfigRef:string;expectedTaskRevision:number;
}
type Intent={payload:ComparisonPayload;state:'pending'|'unknown'|'applied';requestRef?:string};
// Retain only command references and version identities while a panel is closed.
// A late response belongs to this scope; it must not navigate a different panel.
const intents=new Map<string,Intent>();
const listeners=new Set<()=>void>();
function subscribe(listener:()=>void){listeners.add(listener);return()=>{listeners.delete(listener);};}
export function setComparisonIntent(key:string,intent:Intent|null){
  if(intent)intents.set(key,intent);else intents.delete(key);
  for(const notify of listeners)notify();
}
export function comparisonIntent(key:string){return intents.get(key);}
export function useComparisonIntent(key:string){return useSyncExternalStore(subscribe,()=>intents.get(key));}
