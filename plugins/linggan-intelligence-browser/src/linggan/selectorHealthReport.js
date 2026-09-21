// 结构自检的**durable 一侧**：页面看得见 DOM，但页面会随导航消失；background 会随
// MV3 回收休眠，但 `chrome.storage.local` 里的东西活得比两者都长。
//
// 这份记录只为一件事存在：报到的时候告诉 Linggan「这台机器的插件最近一次看页面，看到了
// 什么」。因此它存的是**受限快照**——平台、页面类型、能力、两个时刻和检查项名，没有
// 选择器串、没有 DOM 文本、没有页面地址（见 `shared/selectorHealth.js` 的字段表）。
//
// 时间同样分两种，不能混：
// - `snapshot.checkedAt`：这次检查发生的时刻（页面说的）；
// - `snapshot.verifiedAt`：这些选择器**上一次人工重验**的日期（可能是几个月前）。
//   一条记录里两个值一样才是巧合，不一样是常态——界面按「上次检查」和「上次人工验证」
//   分别显示，不拿检查时刻冒充验证日期。
//
// 存进本地之后还会多一层 `failureCounts`：某个检查项**连续多少次检查都是缺的**。
// 一次缺失说明不了什么（页面还在加载）；同一个检查项在每次检查里都缺，才是结构变了。
// 计的是连续次数，不是累计次数——中间有一次查过、在，就从零重新开始。

import { normalizeSelectorHealthSnapshot } from '../shared/selectorHealth.js';

/// 这份记录的存储位。版本后缀是给后来改形状的人留的：换了形状就换 key，旧记录自然作废，
/// 不做字段级 fallback。
export const SELECTOR_HEALTH_STORAGE_KEY = 'linggan.localTrusted.selectorHealth.v1';

/// 能上报的平台（闭集）。插件只在这两个站点上跑；别的一律不收。
const REPORTED_PLATFORMS = Object.freeze(['xhs', 'douyin']);
const MAX_FAILURE_ENTRIES = 16;

function emptyState() {
  return {};
}

async function readState(storage) {
  const raw = await storage.get(SELECTOR_HEALTH_STORAGE_KEY);
  const state = raw?.[SELECTOR_HEALTH_STORAGE_KEY];
  if (!state || typeof state !== 'object' || Array.isArray(state)) return emptyState();
  return state;
}

/// 上一次的记录 → 这一次的连续失败次数。
///
/// 三种情形要分开，混起来就是把「没看」说成「好了」：
/// - 这次查了它、它是缺的 → 次数 +1（并且挪到队尾：队尾是「最近还在缺」的那一批）；
/// - 这次查了它、它在 → 条目消失，连续失败到此为止；
/// - 这次没查它 → 次数原样留着，既不续，也不冒充已恢复。
///
/// 条目上限 16 与快照一次能带 16 个检查项对齐；超过就丢最早那条，
/// 让「最近还在缺的」优先留下。
function nextFailureCounts(previous, snapshot) {
  const checked = new Set(snapshot.checkedCategories);
  const missing = new Set(snapshot.missingCategories);
  const counts = new Map();

  for (const [category, count] of Object.entries(previous || {})) {
    if (missing.has(category)) continue;
    if (checked.has(category)) continue;
    counts.set(category, count);
  }
  for (const category of missing) {
    counts.set(category, Number(previous?.[category] || 0) + 1);
  }

  return Object.fromEntries([...counts].slice(-MAX_FAILURE_ENTRIES));
}

/// 页面报来的一次自检 → 落一份本机记录。
///
/// `platform` 是**来源**（background 从消息的发送页面读出来的），不是页面自己说的：
/// 两边对不上就拒收，免得一个页面替另一个平台写诊断。
export async function recordSelectorHealthReport(
  candidate,
  { platform = '', storage, now = Date.now() } = {},
) {
  const source = String(platform || '').trim().toLowerCase();
  if (!REPORTED_PLATFORMS.includes(source)) {
    return { accepted: false, reason: 'selector_health_source_unrecognized' };
  }
  const snapshot = normalizeSelectorHealthSnapshot(candidate);
  if (!snapshot) {
    return { accepted: false, reason: 'selector_health_snapshot_invalid' };
  }
  if (snapshot.platform !== source) {
    return { accepted: false, reason: 'selector_health_platform_mismatch' };
  }
  if (!storage || typeof storage.get !== 'function' || typeof storage.set !== 'function') {
    return { accepted: false, reason: 'selector_health_store_unavailable' };
  }

  try {
    const state = await readState(storage);
    const previous = state[source];
    await storage.set({
      [SELECTOR_HEALTH_STORAGE_KEY]: {
        ...state,
        [source]: {
          snapshot,
          failureCounts: nextFailureCounts(previous?.failureCounts, snapshot),
          recordedAt: new Date(Number(now)).toISOString(),
        },
      },
    });
  } catch {
    // 存不下就是没收下。诊断本身不影响采集，也不必让页面重试。
    return { accepted: false, reason: 'selector_health_store_unavailable' };
  }

  return { accepted: true, platform: source };
}

/// 报到时要带上的那份诊断：每个平台最近一次快照 + 连续失败次数。
///
/// 形状与快照字段表一致，只多一个 `failureCounts`；没有记录的平台整条不出现
/// （「没有记录」不是「一切正常」）。
export async function selectorHealthForCheckIn({ storage } = {}) {
  if (!storage || typeof storage.get !== 'function') return {};
  let state = {};
  try {
    state = await readState(storage);
  } catch {
    return {};
  }

  const payload = {};
  for (const platform of REPORTED_PLATFORMS) {
    const record = state[platform];
    const snapshot = normalizeSelectorHealthSnapshot(record?.snapshot);
    if (!snapshot || snapshot.platform !== platform) continue;
    payload[platform] = {
      ...snapshot,
      failureCounts: storedFailureCounts(record?.failureCounts),
    };
  }
  return payload;
}

/// 只收受限形状的次数：键是检查项名，值是非负整数。别的一律不带走。
function storedFailureCounts(counts) {
  const entries = Object.entries(counts && typeof counts === 'object' ? counts : {});
  return Object.fromEntries(entries
    .filter(([category, count]) => (
      /^[a-z0-9_]{1,32}$/.test(category) && Number.isInteger(count) && count > 0
    ))
    .slice(0, MAX_FAILURE_ENTRIES));
}
