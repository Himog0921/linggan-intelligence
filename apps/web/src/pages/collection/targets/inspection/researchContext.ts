import { createContext,useContext } from 'react';
import type { Drawer } from '../api';
import type { ReadScope } from './inspectionApi';
export type Panel='compare'|'trace'|'similar';
export interface ResearchContext {
  data:Drawer;params:URLSearchParams;scope:ReadScope;selected:string[];
  panel:string|null;focusedWork:string|null;message:string|null;
  patch:(values:Record<string,string|null>,replace?:boolean)=>void;
  toggle:(work:string)=>void;open:(panel:Panel,work?:string)=>void;back:()=>void;
  clear:()=>void;pinCutoff:(cutoff:string)=>void;
}
export const Context=createContext<ResearchContext|null>(null);
export function useTargetResearch():ResearchContext {const value=useContext(Context);if(!value)throw new Error('目标研究必须位于当前检查器内');return value;}
export function useOptionalTargetResearch(){return useContext(Context);}
