import { describe, expect, it, vi } from "vitest";
import { pendingTarget, saveTarget } from "./targetSave";
const input = (key: string) => ({
  periodType: "month" as const,
  periodKey: key,
  targetValue: 50,
  expectedRevision: null,
});
function answer(
  body: {
    requestId: string;
    periodType: string;
    periodKey: string;
    targetValue: number;
  },
  extra = {},
) {
  return new Response(
    JSON.stringify({
      requestId: body.requestId,
      outcome: "accepted",
      replayed: false,
      current: {
        revisionRef: "revision",
        periodType: body.periodType,
        periodKey: body.periodKey,
        targetValue: body.targetValue,
        setAtBeijing: "10-16 12:00",
      },
      ...extra,
    }),
    { headers: { "content-type": "application/json" } },
  );
}
describe("target save identity and receipt", () => {
  it("freezes the full unanswered body, including revision and value, on retry", async () => {
    const bodies: unknown[] = [];
    const fetch = vi.fn(async (_url, options) => {
      const body = JSON.parse(String(options.body));
      bodies.push(body);
      if (bodies.length === 1) throw new Error("lost response after commit");
      return answer(body, { replayed: true });
    });
    vi.stubGlobal("fetch", fetch);
    const command = input("2026-01");
    await expect(saveTarget(command)).rejects.toThrow();
    expect(pendingTarget("month", "2026-01")).toBeDefined();
    const receipt = await saveTarget({
      ...command,
      targetValue: 999,
      expectedRevision: "changed",
    });
    expect(receipt.replayed).toBe(true);
    expect(bodies[1]).toEqual(bodies[0]);
    expect(pendingTarget("month", "2026-01")).toBeUndefined();
  });
  it("does not accept a receipt for another request or period as success", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async (_url, options) =>
        answer(JSON.parse(String(options.body)), { requestId: "wrong" }),
      ),
    );
    await expect(saveTarget(input("2026-02"))).rejects.toThrow();
    expect(pendingTarget("month", "2026-02")).toBeDefined();
    vi.stubGlobal(
      "fetch",
      vi.fn(async (_url, options) => answer(JSON.parse(String(options.body)))),
    );
    await saveTarget(input("2026-02"));
  });
  it("keeps the request when a server failure may follow a committed transaction", async () => {
    const bodies: unknown[] = [];
    vi.stubGlobal(
      "fetch",
      vi.fn(async (_url, options) => {
        const body = JSON.parse(String(options.body));
        bodies.push(body);
        return bodies.length === 1
          ? new Response(JSON.stringify({ code: "ops_read_unavailable" }), {
              status: 503,
              headers: { "content-type": "application/json" },
            })
          : answer(body, { replayed: true });
      }),
    );
    await expect(saveTarget(input("2026-07"))).rejects.toThrow("503");
    expect(pendingTarget("month", "2026-07")).toBeDefined();
    await saveTarget({ ...input("2026-07"), targetValue: 999 });
    expect(bodies[1]).toEqual(bodies[0]);
  });
  it("does not confirm a different accepted value", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async (_url, options) => {
        const body = JSON.parse(String(options.body));
        return answer({ ...body, targetValue: 999 });
      }),
    );
    await expect(saveTarget(input("2026-06"))).rejects.toThrow();
    expect(pendingTarget("month", "2026-06")).toBeDefined();
    vi.stubGlobal(
      "fetch",
      vi.fn(async (_url, options) => answer(JSON.parse(String(options.body)))),
    );
    await saveTarget(input("2026-06"));
  });
  it("settles explicit 409 but never silently changes a body under the old key", async () => {
    const keys: string[] = [];
    vi.stubGlobal(
      "fetch",
      vi.fn(async (_url, options) => {
        const body = JSON.parse(String(options.body));
        keys.push(body.requestId);
        return keys.length === 1
          ? new Response(
              JSON.stringify({ code: "ops_target_request_conflict" }),
              { status: 409, headers: { "content-type": "application/json" } },
            )
          : answer(body);
      }),
    );
    await expect(saveTarget(input("2026-03"))).rejects.toThrow("409");
    expect(pendingTarget("month", "2026-03")).toBeUndefined();
    await saveTarget(input("2026-03"));
    expect(keys[0]).not.toBe(keys[1]);
  });
  it("prevents concurrent duplicate submissions and isolates another period", async () => {
    let resolve: ((response: Response) => void) | undefined;
    let original: Parameters<typeof answer>[0] | undefined;
    vi.stubGlobal(
      "fetch",
      vi.fn((_url, options) => {
        original = JSON.parse(String(options.body));
        return new Promise<Response>((done) => {
          resolve = done;
        });
      }),
    );
    const first = saveTarget(input("2026-04"));
    await expect(saveTarget(input("2026-04"))).rejects.toThrow("pending");
    expect(pendingTarget("month", "2026-05")).toBeUndefined();
    if (!resolve || !original) throw new Error("request not started");
    resolve(answer(original));
    await first;
  });
});
