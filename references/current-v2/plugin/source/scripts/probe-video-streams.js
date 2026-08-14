/**
 * 视频流质量探查脚本（在视频笔记详情页 DevTools Console 运行）
 *
 * 目的：
 * 1. 输出 __INITIAL_STATE__ 中的视频流列表
 * 2. 标记可用的 bitrate / 分辨率信息
 * 3. 便于确认“最高码率优先”策略是否正确
 */
(function probeVideoStreams() {
  const noteMap = window.__INITIAL_STATE__?.note?.noteDetailMap || {};
  const keys = Object.keys(noteMap);
  if (keys.length === 0) {
    console.warn('[probe-video-streams] noteDetailMap 为空');
    return null;
  }

  const key = keys.find((k) => k && k !== 'undefined') || keys[0];
  const raw = noteMap[key];
  const note = raw?.note || raw || {};
  const stream = note?.video?.media?.stream || {};
  // 外层分组名不是协议（实测可为 EF4/EF5），只把即时数组当作流集合。
  const streamGroups = Object.entries(stream || {})
    .filter(([, items]) => Array.isArray(items));
  const groupCounts = Object.fromEntries(
    streamGroups.map(([group, items]) => [group, items.length]),
  );
  const rows = streamGroups.flatMap(([group, items]) => items.map((item) => ({
    group,
    codec: item.videoCodec || item.format || item.codecType || '',
    bitrate: item.avgBitrate || item.bitrate || item.avg_bitrate || 0,
    width: item.width || item.vwidth || 0,
    height: item.height || item.vheight || 0,
    qualityType: item.qualityType || item.quality_type || '',
    url: item.masterUrl || item.master_url || item.url || item.backupUrl || item.backup_url || '',
    backupUrls: item.backupUrls || item.backup_urls || [],
  })));

  const output = {
    url: location.href,
    time: new Date().toISOString(),
    noteId: note.noteId || note.id || key,
    groupCounts,
    streamCount: rows.length,
    rows,
  };

  console.table(rows);
  console.log('[probe-video-streams] result:', output);
  return output;
})();
