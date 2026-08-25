export function createManagedTaskController(runTask, {
  onFinally = null,
} = {}) {
  const state = {
    isRunning: false,
    isPaused: false,
    isStopping: false,
    pauseResolve: null,
  };

  const controller = {
    get isRunning() {
      return state.isRunning;
    },
    pause() {
      if (!state.isRunning || state.isStopping) return;
      state.isPaused = true;
    },
    resume() {
      if (!state.isRunning || state.isStopping) return;
      if (!state.isPaused) return;
      state.isPaused = false;
      const resolve = state.pauseResolve;
      state.pauseResolve = null;
      if (resolve) resolve();
    },
    stop() {
      if (!state.isRunning) return;
      state.isStopping = true;
      if (state.pauseResolve) {
        state.isPaused = false;
        const resolve = state.pauseResolve;
        state.pauseResolve = null;
        resolve();
        return;
      }
      controller.resume();
    },
    shouldStop() {
      return state.isStopping;
    },
    async waitIfPaused() {
      if (!state.isPaused) return;
      if (state.isStopping) return;
      await new Promise((resolve) => {
        state.pauseResolve = resolve;
      });
    },
    start() {
      if (state.isRunning) return;
      state.isRunning = true;
      state.isPaused = false;
      state.isStopping = false;
      Promise.resolve()
        .then(() => runTask({
          shouldStop: controller.shouldStop,
          waitIfPaused: controller.waitIfPaused,
        }))
        .finally(() => {
          state.isRunning = false;
          state.isPaused = false;
          state.isStopping = false;
          if (state.pauseResolve) {
            state.pauseResolve();
            state.pauseResolve = null;
          }
          onFinally?.();
        });
    },
  };

  return controller;
}
