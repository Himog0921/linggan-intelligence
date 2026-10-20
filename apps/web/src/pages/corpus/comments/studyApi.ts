import { isRecord, requestJson } from '../../../shared/api';
import { useQuery } from '@tanstack/react-query';

const endpoint = '/api/local/comment-study/';
export const studyKey = (domain: string | null, path: string) => ['comment-study', domain, path] as const;

export function studyPath(domain: string | null, path: string) {
  const url = new URL(endpoint + path, 'http://localhost');
  if (domain) url.searchParams.set('domain', domain);
  return url.pathname + url.search;
}

export function assertStudyReply(value: unknown, domain: string | null): asserts value is Record<string, unknown> {
  if (!isRecord(value) || (value.domainRef != null && value.domainRef !== domain)) {
    throw new Error('读取回执与当前范围不一致，请重新读取。');
  }
}

/** Endpoint-specific parsers remain with their DTOs; reads carry cancellation and domain identity. */
export async function studyRead<T>(domain: string | null, path: string, parse: (value: Record<string, unknown>) => T, signal?: AbortSignal): Promise<T> {
  const value = await requestJson(studyPath(domain, path), signal ? { signal } : {});
  assertStudyReply(value, domain);
  if (signal?.aborted) throw new DOMException('Aborted', 'AbortError');
  return parse(value);
}

export function useStudyRead<T>(domain: string | null, path: string, parse: (value: Record<string, unknown>) => T, enabled = true) {
  return useQuery({ queryKey: studyKey(domain, path), queryFn: ({ signal }) => studyRead(domain, path, parse, signal), enabled: Boolean(domain) && enabled, retry: false, gcTime: 0, refetchOnWindowFocus: false });
}

/** Commands are never retried here. The view owns immutable intent and explicit retry identity. */
export async function studyPost<T>(domain: string, path: string, body: unknown, parse: (value: Record<string, unknown>) => T, signal?: AbortSignal): Promise<T> {
  const value = await requestJson(endpoint + path, { method: 'POST', body, ...(signal ? { signal } : {}) });
  assertStudyReply(value, domain);
  return parse(value);
}

export interface StudyPage { nextCursor?: string | null; hasMore?: boolean; totalCount?: number | null }
export interface CommentKey { workRef: string; commentExternalId: string }
export const commentIdentity = (key: CommentKey) => JSON.stringify([key.workRef, key.commentExternalId]);
export interface StudyNavigation {
  view(view: 'overview' | 'comments' | 'runs' | 'problems'): void;
  run(runRef: string, panel?: string, targetRef?: string | null, signalRef?: string | null): void;
  problem(problemRef: string): void;
  comment(key: CommentKey): void;
  candidates(state: string): void;
}

export interface SelectedComment { commentKey: CommentKey; observationRole: string }
