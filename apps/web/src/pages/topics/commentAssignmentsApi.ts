import { requestJson } from '../../shared/api';
import * as d from '../../shared/decode';
export type AssignmentState = 'not_analyzed' | 'pending' | 'complete' | 'failed' | 'unavailable';
export type CommentAssignmentScope = { domain: string; work: string; request?: string };
function state(row: d.Json): AssignmentState {
  const value = d.str(row, 'assignmentState');
  if (value === 'not_analyzed' || value === 'pending' || value === 'complete' || value === 'failed' || value === 'unavailable') return value;
  throw new d.ShapeError('assignmentState');
}
function assignment(row: d.Json) {
  const status = d.str(row, 'status');
  if (status !== 'matched' && status !== 'uncertain' && status !== 'out_of_scope') throw new d.ShapeError('assignment.status');
  return { status, reason: d.str(row, 'reason'), matches: d.list(row, 'matches', match => ({ topicRef: d.str(match, 'topicRef'), definitionRef: d.str(match, 'definitionRef'), label: d.str(match, 'label'), reason: d.str(match, 'reason') })) };
}
export function parseCommentAssignments(value: unknown, scope: CommentAssignmentScope) {
  return d.decode(value, root => {
    if (d.str(root, 'domainRef') !== scope.domain) throw new d.ShapeError('domainRef');
    return d.list(root, 'items', row => {
      const requestRef = d.str(row, 'requestRef'), workRef = d.str(row, 'workRef');
      if (workRef !== scope.work || (scope.request && requestRef !== scope.request)) throw new d.ShapeError('assignmentScope');
      const sourceAvailable = d.bool(row, 'sourceAvailable');
      return { requestRef, workRef, windowRef: d.optStr(row, 'windowRef'), sourceRef: d.str(row, 'sourceRef'), commentExternalId: d.str(row, 'commentExternalId'), sourceAvailable, assignmentState: state(row),
        signals: sourceAvailable ? d.list(row, 'signals', signal => {
          const evidence = d.obj(signal, 'evidence'), linked = d.optObj(signal, 'assignment');
          return { ordinal: d.num(signal, 'signalOrdinal'), kind: d.str(signal, 'kind'), proposition: d.str(signal, 'proposition'), assignmentState: state(signal),
            evidence: { sourceRef: d.str(evidence, 'sourceRef'), start: d.num(evidence, 'start'), end: d.num(evidence, 'end'), text: d.str(evidence, 'text') },
            assignment: linked ? assignment(linked) : null };
        }) : [],
      };
    });
  });
}
export type CommentAssignment = ReturnType<typeof parseCommentAssignments>[number];
export async function readCommentAssignments(scope: CommentAssignmentScope, signal: AbortSignal) {
  const params = new URLSearchParams({ domainRef: scope.domain, workRef: scope.work });
  if (scope.request) params.set('requestRef', scope.request);
  return parseCommentAssignments(await requestJson(`/api/local/topic-map/comment-assignments?${params}`, { signal }), scope);
}
