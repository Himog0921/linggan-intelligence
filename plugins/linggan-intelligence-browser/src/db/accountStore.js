import db from './index.js';

function now() {
  return Date.now();
}

function cuid() {
  const timestamp = Date.now().toString(36);
  const random = Math.random().toString(36).slice(2, 10);
  return `ac_${timestamp}_${random}`;
}

function todayStr() {
  return new Date().toISOString().slice(0, 10);
}

function normalizeAccount(item = {}) {
  return {
    accountId: String(item.accountId || '').trim(),
    name: String(item.name || '').trim(),
    cookieJson: String(item.cookieJson || '').trim(),
    platform: String(item.platform || 'xhs').trim(),
    status: String(item.status || 'available').trim(),
    dailyQuotaUsed: Number.isFinite(Number(item.dailyQuotaUsed)) ? Math.floor(Number(item.dailyQuotaUsed)) : 0,
    dailyQuotaLimit: Number.isFinite(Number(item.dailyQuotaLimit)) ? Math.floor(Number(item.dailyQuotaLimit)) : 100,
    cooldownUntil: Number.isFinite(Number(item.cooldownUntil)) ? Number(item.cooldownUntil) : 0,
    lastUsedAt: Number.isFinite(Number(item.lastUsedAt)) ? Number(item.lastUsedAt) : 0,
    totalUsed: Number.isFinite(Number(item.totalUsed)) ? Math.floor(Number(item.totalUsed)) : 0,
    lastResetDate: String(item.lastResetDate || '').trim(),
    createdAt: Number.isFinite(Number(item.createdAt)) ? Number(item.createdAt) : now(),
  };
}

async function resetIfNewDay(account) {
  const today = todayStr();
  if (account.lastResetDate !== today) {
    const updated = { ...account, dailyQuotaUsed: 0, lastResetDate: today };
    await db.accounts.put(updated);
    return updated;
  }
  return account;
}

export const accountStore = {
  async create({ name, cookieJson, platform = 'xhs', dailyQuotaLimit = 100 } = {}) {
    const account = normalizeAccount({
      accountId: cuid(),
      name,
      cookieJson,
      platform,
      dailyQuotaLimit,
      status: 'available',
      lastResetDate: todayStr(),
      createdAt: now(),
    });
    if (!account.name || !account.cookieJson) {
      throw new Error('name and cookieJson are required');
    }
    await db.accounts.put(account);
    return account;
  },

  async getById(accountId) {
    const id = String(accountId || '').trim();
    if (!id) return null;
    return db.accounts.get(id);
  },

  async getAll() {
    const accounts = await db.accounts.toArray();
    const results = [];
    for (const account of accounts) {
      results.push(await resetIfNewDay(account));
    }
    return results;
  },

  async getAvailable(platform = 'xhs') {
    const accounts = await db.accounts
      .where('status')
      .equals('available')
      .toArray();

    const nowMs = now();
    const candidates = [];

    for (const account of accounts) {
      const reset = await resetIfNewDay(account);
      if (reset.platform !== platform) continue;
      if (reset.dailyQuotaUsed >= reset.dailyQuotaLimit) continue;
      if (reset.cooldownUntil > nowMs) continue;
      candidates.push(reset);
    }

    candidates.sort((a, b) => a.lastUsedAt - b.lastUsedAt);
    return candidates[0] || null;
  },

  async updateUsage(accountId) {
    const id = String(accountId || '').trim();
    if (!id) return;
    const account = await db.accounts.get(id);
    if (!account) return;
    await db.accounts.update(id, {
      dailyQuotaUsed: (account.dailyQuotaUsed || 0) + 1,
      totalUsed: (account.totalUsed || 0) + 1,
      lastUsedAt: now(),
    });
  },

  async markCooldown(accountId, cooldownMs = 2 * 60 * 60 * 1000) {
    const id = String(accountId || '').trim();
    if (!id) return;
    await db.accounts.update(id, {
      status: 'cooldown',
      cooldownUntil: now() + cooldownMs,
    });
  },

  async markAvailable(accountId) {
    const id = String(accountId || '').trim();
    if (!id) return;
    await db.accounts.update(id, {
      status: 'available',
      cooldownUntil: 0,
    });
  },

  async resetDailyQuota() {
    const today = todayStr();
    const accounts = await db.accounts.toArray();
    for (const account of accounts) {
      if (account.lastResetDate !== today) {
        await db.accounts.update(account.accountId, {
          dailyQuotaUsed: 0,
          lastResetDate: today,
          status: account.status === 'cooldown' && (account.cooldownUntil || 0) <= now()
            ? 'available'
            : account.status,
        });
      }
    }
  },

  async remove(accountId) {
    const id = String(accountId || '').trim();
    if (!id) return;
    await db.accounts.delete(id);
  },

  async update(accountId, patch = {}) {
    const id = String(accountId || '').trim();
    if (!id) return;
    await db.accounts.update(id, patch);
  },
};
