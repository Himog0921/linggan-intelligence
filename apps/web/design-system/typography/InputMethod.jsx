import React,{createContext,useContext,useEffect,useState} from 'react';

const InputMethod=createContext({method:'pointer',setMethod:()=>{}});
export const useInputMethod=()=>useContext(InputMethod);

export function InputMethodScope({children}){
 const [method,setMethod]=useState('pointer');
 useEffect(()=>{
  const pointer=()=>setMethod('pointer');
  const key=e=>{if(e.key==='Tab'||(['ArrowDown','ArrowUp','Home','End'].includes(e.key)&&e.target?.closest('[role=combobox]')))setMethod('keyboard')};
  document.addEventListener('pointerdown',pointer,true);document.addEventListener('keydown',key,true);
  return()=>{document.removeEventListener('pointerdown',pointer,true);document.removeEventListener('keydown',key,true)};
 },[]);
 return <InputMethod.Provider value={{method,setMethod}}><div className="accepted-input-method" data-input-method={method}>{children}</div></InputMethod.Provider>;
}
