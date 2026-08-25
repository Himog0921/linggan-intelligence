const HEALTH_MESSAGE = "linggan.producer.health_probe.v1";
const DISCOVERY_MESSAGE = "linggan.producer.run_first_discovery.v1";

function setHostState({ state, detail }) {
  const hostState = document.querySelector("#host-state");
  hostState.textContent = state;
  hostState.dataset.state = state.toLowerCase();
  hostState.title = detail;
}

function setDiscoveryState({ state, detail, visibleCards, stoppedReason }) {
  const access = document.querySelector("#discovery-access");
  const message = document.querySelector("#discovery-state");
  const button = document.querySelector("#discovery-button");
  access.textContent = state;
  access.dataset.state = state.toLowerCase();
  message.dataset.discoveryStatus = state;
  message.textContent = [state, detail, visibleCards === undefined ? "" : `可见卡片 ${visibleCards}`, stoppedReason ? `停止：${stoppedReason}` : ""]
    .filter(Boolean)
    .join(" · ");
  const enabled = ["LOCAL_INGRESS_READY", "ACCEPTED", "REPLAY", "NOT_ACCEPTED"].includes(state);
  button.disabled = !enabled;
  button.setAttribute("aria-disabled", String(!enabled));
}

async function refreshLoopbackState() {
  try {
    const response = await chrome.runtime.sendMessage({ type: HEALTH_MESSAGE });
    setHostState(response);
    setDiscoveryState(response.state === "LOCAL_INGRESS_READY"
      ? { state: "LOCAL_INGRESS_READY", detail: "本机已就绪；仅在当前 ADHD 综合搜索页手动读取前 20 张卡片。" }
      : { state: "NOT_CONNECTED", detail: response.detail });
  } catch {
    setHostState({
      state: "LOOPBACK_UNREACHABLE",
      detail: "Linggan local host could not be reached."
    });
    setDiscoveryState({ state: "NOT_CONNECTED", detail: "Linggan 本机不可达；不能开始采集。" });
  }
}

void refreshLoopbackState();

document.querySelector("#discovery-button").addEventListener("click", async (event) => {
  const button = event.currentTarget;
  button.disabled = true;
  button.setAttribute("aria-disabled", "true");
  setDiscoveryState({ state: "RUNNING", detail: "正在读取当前搜索卡片；不会滚动、打开详情或读取评论。" });
  try {
    const result = await chrome.runtime.sendMessage({ type: DISCOVERY_MESSAGE });
    setDiscoveryState(result);
  } catch {
    setDiscoveryState({ state: "NOT_READY", detail: "插件未完成本次操作；没有提交材料。" });
  }
});
