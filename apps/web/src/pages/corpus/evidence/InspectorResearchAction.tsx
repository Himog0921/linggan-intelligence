import { useQueryClient } from '@tanstack/react-query';
import { ActionButton } from '../../../shared/ActionButton';
import { DeepResearchLauncher } from './DeepResearchLauncher';
import { deepResearchWatchKey, useContentUnderstanding, type DeepResearchWatch } from './contentUnderstandingApi';

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
    return <ActionButton variant="primary" disabled>深入研究</ActionButton>;
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
