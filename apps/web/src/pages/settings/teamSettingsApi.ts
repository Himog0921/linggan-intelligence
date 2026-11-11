import { requestJson } from '../../shared/api';
import * as d from '../../shared/decode';

export interface TeamMember {
  memberRef: string;
  email: string;
  displayName: string;
  role: 'admin' | 'lead' | 'operator';
  state: 'active' | 'invited' | 'disabled';
  version: number;
  currentAccountCount: number;
  invitationRef: string | null;
  invitationState: string | null;
  groupRef: string | null;
  groupName: string | null;
  groupHasActiveLead: boolean | null;
  latestDeactivationRef: string | null;
}

export interface InvitationReceipt {
  ref: string;
  memberRef: string;
  token: string;
  expiresInSeconds: number;
}

export interface DeactivationReceipt {
  requestId: string;
  outcome: 'accepted' | 'unchanged' | 'stale' | 'rejected';
  code: string;
  unassignedAccountCount: number | null;
  memberRef: string | null;
  groupVacancyRef: string | null;
  transfers: { accountRef: string; accountName: string | null; nextMemberRef: string; nextMemberName: string | null }[];
  resultingVersion: number | null;
  replayed: boolean;
}

const role = (value: string): TeamMember['role'] => {
  if (value === 'admin' || value === 'lead' || value === 'operator') return value;
  throw new d.ShapeError('role');
};

const state = (value: string): TeamMember['state'] => {
  if (value === 'active' || value === 'invited' || value === 'disabled') return value;
  throw new d.ShapeError('state');
};

export function parseRoster(value: unknown): TeamMember[] {
  return d.decode(value, root => d.list(root, 'members', member => ({
    memberRef: d.str(member, 'memberRef'),
    email: d.str(member, 'email'),
    displayName: d.str(member, 'displayName'),
    role: role(d.str(member, 'role')),
    state: state(d.str(member, 'state')),
    version: d.num(member, 'version'),
    currentAccountCount: d.num(member, 'currentAccountCount'),
    invitationRef: d.optStr(member, 'invitationRef'),
    invitationState: d.optStr(member, 'invitationState'),
    groupRef: d.optStr(member, 'groupRef'),
    groupName: d.optStr(member, 'groupName'),
    groupHasActiveLead: d.optBool(member, 'groupHasActiveLead'),
    latestDeactivationRef: d.optStr(member, 'latestDeactivationRef'),
  })));
}

export function parseInvitation(value: unknown): InvitationReceipt {
  return d.decode(value, root => {
    const invitation = d.obj(root, 'invitation');
    return {
      ref: d.str(invitation, 'ref'),
      memberRef: d.str(invitation, 'memberRef'),
      token: d.str(invitation, 'token'),
      expiresInSeconds: d.num(invitation, 'expiresInSeconds'),
    };
  });
}

export function parseDeactivation(value: unknown): DeactivationReceipt {
  return d.decode(value, root => {
    const outcome = d.str(root, 'outcome');
    if (outcome !== 'accepted' && outcome !== 'unchanged' && outcome !== 'stale' && outcome !== 'rejected') throw new d.ShapeError('outcome');
    return {
      requestId: d.str(root, 'requestId'), outcome, code: d.str(root, 'code'),
      unassignedAccountCount: d.optNum(root, 'unassignedAccountCount'),
      memberRef: d.optStr(root, 'memberRef'), groupVacancyRef: d.optStr(root, 'groupVacancyRef'),
      transfers: root.transfers === undefined ? [] : d.list(root, 'transfers', item => ({
        accountRef: d.str(item, 'accountRef'), accountName: d.optStr(item, 'accountName'),
        nextMemberRef: d.str(item, 'nextMemberRef'), nextMemberName: d.optStr(item, 'nextMemberName'),
      })),
      resultingVersion: d.optNum(root, 'resultingVersion'), replayed: d.bool(root, 'replayed'),
    };
  });
}

export async function fetchRoster(signal?: AbortSignal) {
  const value = await requestJson('/api/local/auth/members', { signal });
  return { members: parseRoster(value), actorMemberRef: d.decode(value, root => d.optStr(root, 'actorMemberRef')) };
}

export async function createInvitation(input: { email: string; displayName: string; role: TeamMember['role'] }) {
  return parseInvitation(await requestJson('/api/local/auth/invitations', { method: 'POST', body: input }));
}

export interface Assignment {
  accountRef: string;
  responsibilityVersion: number;
  nextMemberRef: string;
  groupRef: string | null;
}

export interface StopRequest {
  requestId: string;
  expectedVersion: number;
  reason: string;
  assignments?: Assignment[];
}

export async function deactivateMember(memberRef: string, body: StopRequest) {
  return parseDeactivation(await requestJson(`/api/local/auth/members/${encodeURIComponent(memberRef)}/deactivate`, {
    method: 'POST', body,
  }));
}

export async function fetchOffboardingPreview(memberRef: string, signal?: AbortSignal) {
  const value = await requestJson(`/api/local/auth/members/${encodeURIComponent(memberRef)}/offboarding-preview`, { signal });
  return d.decode(value, root => ({
    memberVersion: d.num(root, 'memberVersion'), groupVacancyRef: d.optStr(root, 'groupVacancyRef'),
    accounts: d.list(root, 'accounts', item => ({
      accountRef: d.str(item, 'accountRef'), accountName: d.str(item, 'accountName'),
      platform: d.str(item, 'platform'), responsibilityVersion: d.num(item, 'responsibilityVersion'),
    })),
  }));
}

export async function fetchDeactivation(requestId: string, signal?: AbortSignal) {
  return parseDeactivation(await requestJson(`/api/local/auth/deactivations/${encodeURIComponent(requestId)}`, { signal }));
}

export async function revokeInvitation(ref: string) {
  await requestJson(`/api/local/auth/invitations/${encodeURIComponent(ref)}/revoke`, { method: 'POST' });
}

export interface PendingStop {
  memberRef: string;
  body: StopRequest;
}

const storageKey = (actor: string) => `ops-pending-stop:${actor}`;

export function readPendingStop(actor: string): PendingStop | null {
  try {
    const saved = sessionStorage.getItem(storageKey(actor));
    if (!saved) return null;
    const value = JSON.parse(saved);
    return typeof value?.memberRef === 'string' && typeof value?.body?.requestId === 'string' ? value : null;
  } catch {
    return null;
  }
}

export function savePendingStop(actor: string | null, value: PendingStop | null) {
  if (!actor) return;
  try {
    if (value) sessionStorage.setItem(storageKey(actor), JSON.stringify(value));
    else sessionStorage.removeItem(storageKey(actor));
  } catch {
    // A blocked storage policy still allows the current page to retry the in-memory request.
  }
}

export const pendingAccountsUrl = (memberRef: string) => `/ops/accounts?pending_from=${encodeURIComponent(memberRef)}`;
