export const BRAND_ASSETS = {
  logo: 'lgboom-logo.svg',
  banner: 'lgboom-banner.svg',
  injectLogo: 'lgboom-inject-logo.svg',
};

export function getBrandAssetUrl(asset) {
  try {
    return chrome.runtime?.getURL?.(asset) || asset;
  } catch {
    return asset;
  }
}
