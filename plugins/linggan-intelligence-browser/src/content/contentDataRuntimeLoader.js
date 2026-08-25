let contentDataRuntimeModulePromise = null;

export async function loadContentDataRuntimeFactory() {
  if (!contentDataRuntimeModulePromise) {
    contentDataRuntimeModulePromise = import(/* webpackMode: "eager" */ './contentDataRuntime.js');
  }

  const { createContentDataRuntime } = await contentDataRuntimeModulePromise;
  if (typeof createContentDataRuntime !== 'function') {
    throw new Error('contentDataRuntime 未导出 createContentDataRuntime');
  }
  return createContentDataRuntime;
}
