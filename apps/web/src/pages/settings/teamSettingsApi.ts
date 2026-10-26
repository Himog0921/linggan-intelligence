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
  unassignedAccountCount: number;
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
      unassignedAccountCount: d.num(root, 'unassignedAccountCount'),
      resultingVersion: d.optNum(root, 'resultingVersion'), replayed: d.bool(root, 'replayed'),
    };
  });
}

export async function fetchRoster(signal?: AbortSignal) {
  return parseRoster(await requestJson('/api/local/auth/members', { signal }));
}

export async function createInvitation(input: { email: string; displayName: string; role: TeamMember['role'] }) {
  return parseInvitation(await requestJson('/api/local/auth/invitations', { method: 'POST', body: input }));
}

export async function deactivateMember(member: TeamMember, requestId: string, reason: string) {
  return parseDeactivation(await requestJson(`/api/local/auth/members/${encodeURIComponent(member.memberRef)}/deactivate`, {
    method: 'POST', body: { requestId, expectedVersion: member.version, reason },
  }));
}
