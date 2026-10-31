import type { IdeaBoxSummary, WorkSummary } from "./workbenchModel";

/** The today view has one card per actual idea; the draft view keeps every manuscript. */
export function topicRows(works: WorkSummary[], ideas: IdeaBoxSummary[]): WorkSummary[] {
  const byIdea = new Map(ideas.map(idea => [idea.ideaRef, idea]));
  const byWork = new Map(works.map(work => [work.workId, work]));
  const seen = new Set<string>();
  return works.flatMap(work => {
    if (!work.ideaRef) return [work];
    if (seen.has(work.ideaRef)) return [];
    seen.add(work.ideaRef);
    const idea = byIdea.get(work.ideaRef);
    if (!idea) return [{ ...work, title: '选题信息暂不可用' }];
    const anchor = byWork.get(idea.anchorWorkId);
    return [{ ...(anchor ?? { ...work, workId: idea.anchorWorkId, revisionId: idea.anchorRevisionId, savedAt: idea.savedAt, copiedRevisionId: undefined }), title: idea.name }];
  });
}
