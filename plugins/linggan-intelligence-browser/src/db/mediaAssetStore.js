import db from './index.js';
import { normalizeMediaAssetRecord } from './recordNormalization.js';

export const mediaAssetStore = {
  async upsert(asset) {
    await db.mediaAssets.put(normalizeMediaAssetRecord(asset));
  },

  async bulkUpsert(assets) {
    if (!assets || assets.length === 0) return;
    await db.transaction('rw', db.mediaAssets, async () => {
      await db.mediaAssets.bulkPut(assets.map(normalizeMediaAssetRecord));
    });
  },

  async getAll() {
    const assets = await db.mediaAssets.orderBy('createdAt').reverse().toArray();
    return assets.map(normalizeMediaAssetRecord);
  },

  async getByContentId(contentId) {
    const assets = await db.mediaAssets.where('contentId').equals(contentId).toArray();
    return assets.map(normalizeMediaAssetRecord);
  },

  async getByCollectionRunId(collectionRunId) {
    const assets = await db.mediaAssets.where('collectionRunId').equals(collectionRunId).toArray();
    return assets.map(normalizeMediaAssetRecord);
  },

  async deleteById(assetId) {
    await db.mediaAssets.delete(assetId);
  },
};
