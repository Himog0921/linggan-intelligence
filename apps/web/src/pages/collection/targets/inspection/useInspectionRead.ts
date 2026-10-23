import { useQuery } from '@tanstack/react-query';
import { ApiError,requestJson } from '../../../../shared/api';

export interface InspectionRead<T>{data:T|null;error:string|null;pending:boolean;stale:boolean;reload:()=>void}
/** Keys include the exact scope/cutoff. Old responses never populate a different selection. */
export function useInspectionRead<T>(path:string|null,decode:(value:unknown)=>T):InspectionRead<T>{
  const query=useQuery({
    queryKey:['target-inspection',path],
    queryFn:({signal})=>requestJson(path??'',{signal}).then(decode),
    enabled:path!==null,retry:false,gcTime:0,
  });
  const error=query.error===null?null:query.error instanceof ApiError&&query.error.failure.kind==='http'&&query.error.failure.status===404
    ?'当前范围内无法读取所选对象；原列表仍可使用。'
    :query.error instanceof ApiError&&query.error.failure.kind==='http'&&query.error.failure.status===400
      ?'当前对照条件无效，请重新选择对象、轮次或时间。'
      :'本次读取未完成；未知不表示没有作品或没有变化。';
  return {data:path===null?null:query.data??null,error:path===null?null:error,
    pending:path!==null&&query.isFetching,stale:error!==null&&query.data!==undefined,
    reload:()=>{void query.refetch();}};
}
