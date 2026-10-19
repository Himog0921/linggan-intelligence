import { useIsMutating, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  registerCreator,
  saveOverride,
  savePolicy,
  type OverrideRequest,
  type PolicyRequest,
  type RegisterRequest,
} from './creatorsApi';

function useRefreshingMutation<V, R>(kind: string, perform: (variables: V) => Promise<R>) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationKey: ['creator-write', kind],
    mutationFn: perform,
    // 不论回执到达时发起它的界面还在不在，服务器上的状态已经变了：列表照常重读。
    // 界面上要不要显示这个回执，由发起它的上下文（useWriteGuard）决定。
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: ['creators'] });
    },
  });
}

export const useJoinMutation = () => useRefreshingMutation('join', (request: RegisterRequest) => registerCreator(request));
export const usePolicyMutation = () => useRefreshingMutation('policy', (request: PolicyRequest) => savePolicy(request));
export const useOverrideMutation = () => useRefreshingMutation('override', (request: OverrideRequest) => saveOverride(request));
/** 某位作者的「加入观察」是否正在提交——行内按钮与抽屉按钮共同参照，重复点击只显示进行中。 */
export function useJoinPending(creatorKey: string): boolean {
  return (
    useIsMutating({
      mutationKey: ['creator-write', 'join'],
      predicate: mutation => (mutation.state.variables as RegisterRequest | undefined)?.creatorKey === creatorKey,
    }) > 0
  );
}
