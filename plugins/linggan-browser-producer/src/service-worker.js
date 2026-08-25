export const LINGGAN_LOOPBACK_HEALTH_URL = "http://localhost:3000/health";

const MESSAGE_TYPE = "linggan.producer.health_probe.v1";

function loopbackHealthTarget() {
  const target = new URL(LINGGAN_LOOPBACK_HEALTH_URL);
  if (target.protocol !== "http:" || target.hostname !== "localhost" || target.port !== "3000") {
    throw new Error("Linggan producer only permits the fixed localhost:3000 health target.");
  }
  return target;
}

function localHostResult(payload) {
  const isExpectedService = payload && payload.service === "linggan-local-web";

  if (!isExpectedService) {
    return {
      state: "LOOPBACK_UNREACHABLE",
      detail: "localhost:3000 responded, but it did not identify as the Linggan local host."
    };
  }

  return {
    state: "LOCAL_HOST_REACHABLE",
    detail: "Linggan local host is reachable; Discovery ingress is not implemented by this plugin card."
  };
}

export async function probeLingganHealth(fetchImplementation = globalThis.fetch) {
  const target = loopbackHealthTarget();

  try {
    const response = await fetchImplementation(target, {
      method: "GET",
      cache: "no-store",
      credentials: "omit"
    });

    if (!response.ok) {
      return {
        state: "LOOPBACK_UNREACHABLE",
        detail: `Linggan local host returned HTTP ${response.status}.`
      };
    }

    return localHostResult(await response.json());
  } catch {
    return {
      state: "LOOPBACK_UNREACHABLE",
      detail: "Linggan local host could not be reached."
    };
  }
}

chrome.runtime.onMessage.addListener((message, _sender, sendResponse) => {
  if (!message || message.type !== MESSAGE_TYPE) {
    return false;
  }

  probeLingganHealth()
    .then(sendResponse)
    .catch(() => {
      sendResponse({
        state: "LOOPBACK_UNREACHABLE",
        detail: "Linggan local host could not be reached."
      });
    });

  return true;
});
