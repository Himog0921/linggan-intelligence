const HEALTH_MESSAGE = "linggan.producer.health_probe.v1";

function setHostState({ state, detail }) {
  const hostState = document.querySelector("#host-state");
  hostState.textContent = state;
  hostState.dataset.state = state.toLowerCase();
  hostState.title = detail;
}

async function refreshLoopbackState() {
  try {
    const response = await chrome.runtime.sendMessage({ type: HEALTH_MESSAGE });
    setHostState(response);
  } catch {
    setHostState({
      state: "LOOPBACK_UNREACHABLE",
      detail: "Linggan local host could not be reached."
    });
  }
}

void refreshLoopbackState();
