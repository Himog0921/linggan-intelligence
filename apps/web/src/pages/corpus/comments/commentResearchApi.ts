import { requestJson } from '../../../shared/api';
import { parseOverview } from './overviewDtos';

export async function readResearchOverview(domain: string, signal?: AbortSignal) {
  return parseOverview(await requestJson(`/api/local/comment-research/v2/overview?${new URLSearchParams({domainRef:domain})}`, { signal }), domain);
}
