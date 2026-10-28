import { describe, expect, it } from "vitest";
import { ApiError } from "../../shared/api";
import { createTopicCommands } from "./topicCommands";
import { decodeCommand, decodeCollectionReceipt } from "./topicDtos";
import { topicDomain } from "./topicTestFixture";
const id = "50000000-0000-4000-8000-000000000001";
const receipt = {
  receiptRef: id,
  action: "ownCreator",
  subjectRef: null,
  revision: 1,
  persistedAt: "2026-10-16T00:00:00Z",
};
const payload = () => ({
  action: "ownCreator",
  idempotencyKey: "synthetic-frozen",
  domainRef: topicDomain,
  active: true,
  platform: "xhs",
  authorExternalId: "synthetic-author",
});
describe("Topic commands preserve whole requests until a qualified answer", () => {
  it.each([
    new ApiError({ kind: "network" }),
    new ApiError({ kind: "http", status: 503, code: "unavailable" }),
    new ApiError({ kind: "invalid_response", status: 200 }),
  ])("freezes body across ambiguous failure %s", async (error) => {
    const calls: string[] = [];
    let attempts = 0,
      confirmations = 0;
    const commands = createTopicCommands(
      async (_path, body, parse) => {
        calls.push(body);
        if (++attempts === 1) throw error;
        return parse(receipt);
      },
      () => {},
    );
    const body = payload();
    await expect(
      commands.submit(
        "/api/local/topic-map/commands",
        body,
        decodeCommand,
        async () => {
          confirmations++;
        },
      ),
    ).rejects.toBe(error);
    body.active = false;
    await expect(
      commands.submit("/api/local/topic-map/commands", body, decodeCommand),
    ).rejects.toThrow("先重试原请求");
    expect(calls).toHaveLength(1);
    expect(commands.unconfirmed).toBe(true);
    await commands.retry();
    expect(calls).toEqual([calls[0], calls[0]]);
    expect(JSON.parse(calls[1] || "{}").active).toBe(true);
    expect(confirmations).toBe(1);
    expect(commands.unconfirmed).toBe(false);
  });
  it.each([400, 403, 409, 422])(
    "releases a definitively rejected %i identity",
    async (status) => {
      let reject = true;
      const commands = createTopicCommands(
        async (_p, _b, parse) => {
          if (reject)
            throw new ApiError({ kind: "http", status, code: "rejected" });
          return parse(receipt);
        },
        () => {},
      );
      await expect(
        commands.submit(
          "/api/local/topic-map/commands",
          payload(),
          decodeCommand,
        ),
      ).rejects.toBeInstanceOf(ApiError);
      expect(commands.unconfirmed).toBe(false);
      reject = false;
      await commands.submit(
        "/api/local/topic-map/commands",
        payload(),
        decodeCommand,
      );
    },
  );
  it("holds a malformed or wrong-action 200 receipt as unconfirmed", async () => {
    let value: unknown = { ...receipt, action: "viewed" };
    const commands = createTopicCommands(
      async (_p, _b, parse) => parse(value),
      () => {},
    );
    await expect(
      commands.submit(
        "/api/local/topic-map/commands",
        payload(),
        decodeCommand,
      ),
    ).rejects.toBeInstanceOf(ApiError);
    expect(commands.unconfirmed).toBe(true);
    value = receipt;
    await commands.retry();
    expect(commands.unconfirmed).toBe(false);
  });
  it("validates independent collection settings with request, domain and revision", async () => {
    let value = {
      requestRef: id,
      domainRef: topicDomain,
      state: "configured",
      collectionEnabled: true,
      revision: 1,
    };
    const commands = createTopicCommands(
      async (_p, _b, parse) => parse(value),
      () => {},
    );
    const body = {
      action: "configure",
      requestRef: id,
      domainRef: topicDomain,
      collectionEnabled: true,
      expectedRevision: 0,
    };
    await commands.submit(
      "/api/local/topic-map/collection/commands",
      body,
      decodeCollectionReceipt,
    );
    value = { ...value, domainRef: id };
    await expect(
      commands.submit(
        "/api/local/topic-map/collection/commands",
        body,
        decodeCollectionReceipt,
      ),
    ).rejects.toBeInstanceOf(ApiError);
  });
  it("does not repeat a confirmed mutation if refreshing its read model fails", async () => {
    const commands = createTopicCommands(
      async (_p, _b, parse) => parse(receipt),
      () => {},
    );
    await expect(
      commands.submit(
        "/api/local/topic-map/commands",
        payload(),
        decodeCommand,
        async () => {
          throw new Error("read failed");
        },
      ),
    ).rejects.toThrow("read failed");
    expect(commands.unconfirmed).toBe(false);
  });
});
