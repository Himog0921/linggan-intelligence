import { buildFirstDiscoveryPackage } from "./discovery-contract.js";
import { inspectXhsVisibleSearchSurface } from "./xhs-visible-search-adapter.js";

export const LINGGAN_LOOPBACK_HEALTH_URL = "http://localhost:3000/health";
export const LINGGAN_DISCOVERY_INGRESS_URL = "http://localhost:3000/api/local/discovery-packages";

const HEALTH_MESSAGE = "linggan.producer.health_probe.v1";
const DISCOVERY_MESSAGE = "linggan.producer.run_first_discovery.v1";

function fixedLoopbackUrl(value, label) {
  const target = new URL(value);
  if (target.protocol !== "http:" || target.hostname !== "localhost" || target.port !== "3000") {
    throw new Error(`Linggan producer only permits the fixed localhost:3000 ${label} target.`);
  }
  return target;
}

function healthResult(payload) {
  if (!payload || payload.service !== "linggan-local-web") {
    return {
      state: "LOOPBACK_UNREACHABLE",
      detail: "localhost:3000 responded, but it did not identify as the Linggan local host."
    };
  }
  if (payload.dataState !== "LOCAL_DISCOVERY_READ_PROJECTION" || payload.routes?.discoveryIngress !== "/api/local/discovery-packages") {
    return {
      state: "LOCAL_HOST_REACHABLE",
      detail: "Linggan local host is reachable, but its Discovery admission database is not connected."
    };
  }
  return {
    state: "LOCAL_INGRESS_READY",
    detail: "Linggan local Discovery admission is ready for a user-initiated visible-card run."
  };
}

export async function probeLingganHealth(fetchImplementation = globalThis.fetch) {
  try {
    const response = await fetchImplementation(fixedLoopbackUrl(LINGGAN_LOOPBACK_HEALTH_URL, "health"), {
      method: "GET",
      cache: "no-store",
      credentials: "omit"
    });
    if (!response.ok) {
      return { state: "LOOPBACK_UNREACHABLE", detail: `Linggan local host returned HTTP ${response.status}.` };
    }
    return healthResult(await response.json());
  } catch {
    return { state: "LOOPBACK_UNREACHABLE", detail: "Linggan local host could not be reached." };
  }
}

function noSubmit(code, detail) {
  return { state: "NOT_READY", code, detail, submitted: false };
}

export async function runFirstDiscovery({
  tabsApi = globalThis.chrome?.tabs,
  scriptingApi = globalThis.chrome?.scripting,
  fetchImplementation = globalThis.fetch,
  now = () => new Date().toISOString()
} = {}) {
  const health = await probeLingganHealth(fetchImplementation);
  if (health.state !== "LOCAL_INGRESS_READY") {
    return noSubmit("local_ingress_not_ready", health.detail);
  }
  if (!tabsApi || !scriptingApi) {
    return noSubmit("browser_api_unavailable", "当前浏览器不支持 Linggan 的用户手势采集。");
  }

  const [activeTab] = await tabsApi.query({ active: true, currentWindow: true });
  if (!activeTab?.id) {
    return noSubmit("active_tab_unavailable", "未找到当前浏览器页面；请回到小红书 ADHD 搜索页后重试。");
  }

  let injection;
  try {
    injection = await scriptingApi.executeScript({
      target: { tabId: activeTab.id },
      func: inspectXhsVisibleSearchSurface
    });
  } catch {
    return noSubmit("active_tab_permission_unavailable", "无法读取当前页面。请从小红书 ADHD 搜索结果页点击本插件后重试。");
  }

  const scan = injection?.[0]?.result;
  if (!scan || scan.status !== "ready") {
    return noSubmit(scan?.code || "search_surface_unavailable", scan?.detail || "当前页面未提供可审计的搜索卡片。");
  }

  let packageBody;
  try {
    packageBody = buildFirstDiscoveryPackage({
      observedAt: now(),
      stoppedReason: scan.stoppedReason,
      cards: scan.cards
    });
  } catch {
    return noSubmit("local_contract_build_failed", "当前页面结果不能安全转换为 Linggan Discovery 合同，因此没有提交。");
  }

  try {
    const response = await fetchImplementation(fixedLoopbackUrl(LINGGAN_DISCOVERY_INGRESS_URL, "Discovery ingress"), {
      method: "POST",
      cache: "no-store",
      credentials: "omit",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(packageBody)
    });
    const receipt = await response.json().catch(() => null);
    if (!response.ok || !receipt || !["accepted", "replay"].includes(receipt.admission)) {
      return {
        state: "NOT_ACCEPTED",
        code: receipt?.code || `ingress_http_${response.status}`,
        detail: "Linggan 本机没有接纳这次发现；卡片不会因此被写成已入库。",
        submitted: true,
        visibleCards: packageBody.coverage.visibleCards,
        stoppedReason: packageBody.coverage.stoppedReason
      };
    }
    return {
      state: receipt.admission === "replay" ? "REPLAY" : "ACCEPTED",
      detail: receipt.admission === "replay" ? "这份相同发现已存在；Linggan 记录为重放，不重复制造材料。" : "Linggan 已接纳这次 Discovery 卡片与 Coverage。",
      submitted: true,
      visibleCards: receipt.visibleCards,
      stoppedReason: receipt.stoppedReason,
      receipt
    };
  } catch {
    return noSubmit("ingress_unreachable", "提交时无法连接 Linggan 本机入口；没有把本次发现写成已接纳。");
  }
}

if (globalThis.chrome?.runtime?.onMessage) {
  chrome.runtime.onMessage.addListener((message, _sender, sendResponse) => {
    if (!message || ![HEALTH_MESSAGE, DISCOVERY_MESSAGE].includes(message.type)) return false;
    const operation = message.type === HEALTH_MESSAGE ? probeLingganHealth() : runFirstDiscovery();
    operation.then(sendResponse).catch(() => sendResponse(noSubmit("unexpected_plugin_error", "插件未能完成本次操作；没有提交材料。")));
    return true;
  });
}
