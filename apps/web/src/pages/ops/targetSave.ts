import { ApiError, requestJson } from "../../shared/api";
import { receipt, type PeriodType } from "./api";
export interface TargetCommand {
  periodType: PeriodType;
  periodKey: string;
  targetValue: number;
  expectedRevision: string | null;
  requestId: string;
}
// Only numeric target metadata lives here, never source material. An unanswered command survives
// closing/reopening the dialog or navigating to another React page. Retrying freezes the whole body.
interface Pending {
  command: TargetCommand;
  sending: boolean;
}
const pending = new Map<string, Pending>();
const listeners = new Set<() => void>();
export function subscribeTargets(listener: () => void) {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}
function notify() {
  for (const listener of listeners) listener();
}
export const targetKey = (type: PeriodType, key: string) => `${type}:${key}`;
export const pendingTarget = (type: PeriodType, key: string) =>
  pending.get(targetKey(type, key));
export async function saveTarget(input: Omit<TargetCommand, "requestId">) {
  const key = targetKey(input.periodType, input.periodKey);
  let operation = pending.get(key);
  if (operation?.sending) throw new Error("target_save_pending");
  if (!operation) {
    operation = {
      command: { ...input, requestId: crypto.randomUUID() },
      sending: false,
    };
    pending.set(key, operation);
  }
  operation = { command: operation.command, sending: true };
  pending.set(key, operation);
  notify();
  try {
    const answer = receipt(
      await requestJson("/api/local/ops/targets", {
        method: "POST",
        body: operation.command,
      }),
    );
    if (
      answer.requestId !== operation.command.requestId ||
      (answer.outcome !== "stale" &&
        answer.current?.targetValue !== operation.command.targetValue) ||
      (answer.current &&
        (answer.current.periodType !== input.periodType ||
          answer.current.periodKey !== input.periodKey))
    )
      throw new ApiError({ kind: "invalid_response", status: 200 });
    pending.delete(key);
    return answer;
  } catch (error) {
    // A server failure can occur after COMMIT lost its acknowledgement. Keep the identity for
    // 5xx as well as transport/invalid-response errors; only a defined 4xx rejection settles it.
    if (
      error instanceof ApiError &&
      error.failure.kind === "http" &&
      error.failure.status >= 400 &&
      error.failure.status < 500
    )
      pending.delete(key);
    throw error;
  } finally {
    if (pending.get(key) === operation) {
      pending.set(key, { command: operation.command, sending: false });
    }
    notify();
  }
}
