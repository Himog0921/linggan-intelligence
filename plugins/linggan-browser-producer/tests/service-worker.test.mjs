import assert from "node:assert/strict";
import test from "node:test";

import { runFirstDiscovery } from "../src/service-worker.js";

function healthReady() {
  return new Response(JSON.stringify({
    service: "linggan-local-web",
    dataState: "LOCAL_DISCOVERY_READ_PROJECTION",
    routes: { discoveryIngress: "/api/local/discovery-packages" }
  }), { status: 200, headers: { "content-type": "application/json" } });
}

test("manual Discovery uses one active-tab injection and submits only the frozen loopback package", async () => {
  const calls = [];
  const fetchImplementation = async (url, options) => {
    calls.push({ url: String(url), options });
    if (String(url).endsWith("/health")) return healthReady();
    return new Response(JSON.stringify({
      admission: "accepted",
      visibleCards: 2,
      stoppedReason: "unknown"
    }), { status: 200, headers: { "content-type": "application/json" } });
  };
  const result = await runFirstDiscovery({
    fetchImplementation,
    now: () => "2026-08-25T10:00:00+08:00",
    tabsApi: { query: async () => [{ id: 42 }] },
    scriptingApi: {
      executeScript: async (options) => {
        assert.equal(options.target.tabId, 42);
        assert.equal(typeof options.func, "function");
        return [{ result: {
          status: "ready",
          stoppedReason: "unknown",
          cards: [
            { platformContentId: "note-1", title: "第一张", resultPosition: 1 },
            { platformContentId: "note-2", resultPosition: 2 }
          ]
        } }];
      }
    }
  });

  assert.equal(result.state, "ACCEPTED");
  assert.equal(calls.length, 2);
  assert.equal(calls[1].url, "http://localhost:3000/api/local/discovery-packages");
  assert.equal(calls[1].options.credentials, "omit");
  const submitted = JSON.parse(calls[1].options.body);
  assert.equal(submitted.acquisitionSpec.query, "ADHD");
  assert.equal(submitted.coverage.visibleCards, 2);
  assert.equal(submitted.coverage.stoppedReason, "unknown");
});

test("unready XHS surface is not submitted", async () => {
  let submissions = 0;
  const result = await runFirstDiscovery({
    fetchImplementation: async (url) => {
      if (String(url).endsWith("/health")) return healthReady();
      submissions += 1;
      throw new Error("must not submit");
    },
    tabsApi: { query: async () => [{ id: 42 }] },
    scriptingApi: { executeScript: async () => [{ result: { status: "not_ready", code: "wrong_page", detail: "wrong page" } }] }
  });
  assert.equal(result.state, "NOT_READY");
  assert.equal(result.code, "wrong_page");
  assert.equal(submissions, 0);
});
