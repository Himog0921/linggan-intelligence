import { ApiError, isRecord } from "../../shared/api";
import { ShapeError } from "../../shared/decode";

type Body = Record<string, unknown>;
type Sender = <V>(
  path: string,
  body: string,
  parse: (value: unknown) => V,
) => Promise<V>;
interface Frozen {
  path: string;
  body: string;
  send: () => Promise<unknown>;
  confirmed: (value: unknown) => Promise<void>;
}
const uuid = (value: unknown) =>
  typeof value === "string" &&
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(
    value,
  );
const equal = (actual: unknown, expected: unknown) => {
  if (actual !== expected) throw new ShapeError("command receipt identity");
};
function validateReceipt(path: string, body: Body, value: unknown) {
  if (!isRecord(value)) throw new ShapeError("command receipt");
  if (path.endsWith("/structure/preview")) {
    if (
      typeof value.previewHash !== "string" ||
      !/^[0-9a-f]{64}$/.test(value.previewHash) ||
      !isRecord(value.impact)
    )
      throw new ShapeError("preview");
    return;
  }
  if (path.endsWith("/structure/apply")) {
    const plan = body.plan;
    if (!isRecord(plan)) throw new ShapeError("plan");
    equal(value.requestRef, plan.requestRef);
    equal(value.domainRef, plan.domainRef);
    equal(value.kind, plan.kind);
    equal(value.previewHash, body.previewHash);
    equal(value.state, "persisted");
    equal(value.oldReferencesRetained, true);
    equal(value.claimsInherited, false);
    if (
      !Array.isArray(value.destinations) ||
      !Array.isArray(plan.destinations) ||
      value.destinations.length !== plan.destinations.length ||
      value.destinations.some((v) => !isRecord(v) || !uuid(v.topicRef))
    )
      throw new ShapeError("destinations");
    return;
  }
  if (path === "/api/local/topic-map/commands") {
    equal(value.action, body.action);
    if (
      !uuid(value.receiptRef) ||
      !Number.isInteger(value.revision) ||
      Number(value.revision) < 1 ||
      typeof value.persistedAt !== "string"
    )
      throw new ShapeError("persisted receipt");
    if (value.subjectRef !== null && !uuid(value.subjectRef))
      throw new ShapeError("subjectRef");
    if (body.action === "saveAlternative" && !uuid(value.subjectRef))
      throw new ShapeError("alternativeRef");
    if (body.action === "bindTopic" || body.action === "viewed")
      equal(value.subjectRef, body.topicRef);
    return;
  }
  equal(value.requestRef, body.requestRef);
  if (path.endsWith("/research/commands")) {
    equal(value.domainRef, body.domainRef);
    const states: Record<string, string[]> = {
      configure: ["configured", "queued", "no_new_input"],
      start: ["queued", "no_new_input"],
      pause: ["paused"],
      resume: ["queued"],
      stop: ["stopped"],
    };
    if (!states[String(body.action)]?.includes(String(value.state)))
      throw new ShapeError("research state");
    if ("runRef" in body) equal(value.runRef, body.runRef);
    if (value.state === "configured") {
      equal(value.automaticEnabled, body.automaticEnabled);
      equal(value.collectionEnabled, body.collectionEnabled);
    }
    if (
      value.state === "queued" &&
      ["start", "configure"].includes(String(body.action)) &&
      !uuid(value.runRef)
    )
      throw new ShapeError("queued runRef");
    return;
  }
  if (!uuid(value.roundRef)) throw new ShapeError("roundRef");
  if (body.roundRef) equal(value.roundRef, body.roundRef);
  if (String(body.action).startsWith("stop_")) equal(value.state, "stopped");
  else if (body.action === "freeze_search_details") {
    equal(value.phase, "details");
    const requested = body.workRefs;
    if (
      !Array.isArray(value.workRefs) ||
      !Array.isArray(requested) ||
      value.workRefs.some((ref) => !requested.includes(ref))
    )
      throw new ShapeError("frozen workRefs");
    if (value.state === undefined && value.reused !== true)
      throw new ShapeError("freeze state");
  } else if (
    !["active", "partial", "blocked", "stopped", "completed"].includes(
      String(value.state),
    )
  )
    throw new ShapeError("collection state");
}

/** One in-memory unresolved write at a time; retries reuse the entire original JSON body. */
export function createTopicCommands(send: Sender, changed: () => void) {
  let frozen: Frozen | null = null;
  let busy = false;
  async function perform(command: Frozen) {
    if (busy) throw new Error("上一次动作仍在提交");
    busy = true;
    changed();
    let result: unknown;
    try {
      result = await command.send();
    } catch (error) {
      // Network failures, malformed receipts and 5xx can all occur after COMMIT.
      if (
        error instanceof ApiError &&
        error.failure.kind === "http" &&
        error.failure.status >= 400 &&
        error.failure.status < 500
      )
        frozen = null;
      throw error;
    } finally {
      busy = false;
      changed();
    }
    frozen = null;
    changed();
    await command.confirmed(result);
    return result;
  }
  return {
    get busy() {
      return busy;
    },
    get unconfirmed() {
      return frozen !== null && !busy;
    },
    get pendingBody() {
      return frozen ? (JSON.parse(frozen.body) as unknown) : null;
    },
    async submit<V>(
      path: string,
      body: Body,
      parse: (value: unknown) => V,
      confirmed: (value: V) => Promise<void> = async () => {},
    ) {
      if (frozen)
        throw new Error(
          "上一次动作结果未确认，请先重试原请求；当前修改不会替换原提交内容",
        );
      // The serialized body, parser and continuation all belong to the original operation.
      const serialized = JSON.stringify(body);
      const immutable: Body = JSON.parse(serialized) as Body;
      const decode = (value: unknown) => {
        try {
          validateReceipt(path, immutable, value);
          return parse(value);
        } catch (error) {
          if (error instanceof ShapeError)
            throw new ApiError({ kind: "invalid_response", status: 200 });
          throw error;
        }
      };
      const command: Frozen = {
        path,
        body: serialized,
        send: () => send(path, serialized, decode),
        confirmed: async (value) => confirmed(value as V),
      };
      frozen = command;
      return (await perform(command)) as V;
    },
    async retry() {
      if (!frozen) throw new Error("没有待确认的原请求");
      return perform(frozen);
    },
  };
}
