import { useQueryClient } from '@tanstack/react-query';
import { ActionButton } from '../../../shared/ActionButton';
import { DeepResearchLauncher } from './DeepResearchLauncher';
import { deepResearchWatchKey, useContentUnderstanding, type DeepResearchWatch } from './DossierUnderstanding';

export function InspectorResearchAction({ domain, work, revision, asOf, onSubmitted }: {
  domain: string;
  work: string;
  revision?: string;
  asOf?: string;
  onSubmitted: () => void;
}) {
  const queryClient = useQueryClient();
  const query = useContentUnderstanding(domain, work, revision ?? '', asOf ?? '', Boolean(revision && asOf));
  const analysis = query.data?.analysis;
  const requestState = query.data?.currentRequest?.state;
  const coreRef = analysis?.state === 'succeeded'
    && analysis.sourceChanged !== true
    && requestState !== 'pending'
    && requestState !== 'running'
    ? analysis.result?.card.analysisRef
    : undefined;

  if (!coreRef) {
    const title = !revision || !asOf
      ? '需要先完成 L1 内容理解'
      : query.isPending
        ? '正在读取 L1 理解结果'
        : query.isError
          ? '暂时无法读取 L1 理解结果'
          : '需要先完成 L1 内容理解';
    return <ActionButton variant="primary" disabled title={title}>深入研究</ActionButton>;
  }

  return <DeepResearchLauncher
    key={`${domain}:${work}:${coreRef}`}
    domain={domain}
    work={work}
    coreRef={coreRef}
    onSubmitted={() => {
      const watch: DeepResearchWatch = {
        count: query.data?.deepAnalyses?.length ?? 0,
        submittedAt: Date.now(),
      };
      queryClient.setQueryData(deepResearchWatchKey(domain, work), watch);
      onSubmitted();
    }}
  />;
}
