// This pure boundary keeps task-state truth testable without loading the page UI.
export function resolveDouyinBatchControlReceipt(controller, state) {
  return controller?.isRunning
    ? { success: true, state }
    : { success: false, state: 'no_active_task' };
}
