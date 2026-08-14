let douyinRuntimeModulePromise = null;

export async function loadDouyinRuntime() {
  if (!douyinRuntimeModulePromise) {
    douyinRuntimeModulePromise = import(/* webpackMode: "eager" */ './douyinRuntimeModule.js');
  }
  return douyinRuntimeModulePromise;
}
