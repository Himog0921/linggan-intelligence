import { describe, expect, it } from 'vitest';
import { ApiError } from '../../shared/api';
import { parseDeactivation, parseInvitation, parseRoster } from './teamSettingsApi';

describe('团队成员接口解码', () => {
  it('只接纳完整的名册读模型，不把未知状态渲染成可管理成员', () => {
    expect(parseRoster({ members: [{ memberRef: 'm-1', email: 'owner@example.com', displayName: 'Owner', role: 'admin', state: 'active', version: 2, currentAccountCount: 3 }] })).toEqual([{
      memberRef: 'm-1', email: 'owner@example.com', displayName: 'Owner', role: 'admin', state: 'active', version: 2, currentAccountCount: 3,
    }]);
    expect(() => parseRoster({ members: [{ memberRef: 'm-1', email: 'owner@example.com', displayName: 'Owner', role: 'owner', state: 'active', version: 2, currentAccountCount: 3 }] })).toThrow(ApiError);
  });

  it('邀请和停用回执需要服务器明确提供全部边界字段', () => {
    expect(parseInvitation({ invitation: { ref: 'invite-1', memberRef: 'm-1', token: 'opaque', expiresInSeconds: 604800 } })).toMatchObject({ ref: 'invite-1', memberRef: 'm-1' });
    expect(parseDeactivation({ requestId: 'request-1', outcome: 'accepted', code: 'ops_member_deactivated', unassignedAccountCount: 2, resultingVersion: 3, replayed: false })).toMatchObject({ outcome: 'accepted', unassignedAccountCount: 2 });
    expect(() => parseDeactivation({ requestId: 'request-1', outcome: 'made-up', code: 'x', unassignedAccountCount: 0, resultingVersion: null, replayed: false })).toThrow(ApiError);
  });
});
