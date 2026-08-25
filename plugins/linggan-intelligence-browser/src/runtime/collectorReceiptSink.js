// Collector-to-runtime hand-off. Page collectors keep their proven parsing and recovery logic;
// this small module is the single optional delivery seam. It never imports Linggan, so collectors
// remain usable in fixtures and no failed local delivery can be mistaken for failed page reading.

let activeSink = null;

export function registerCollectorReceiptSink(sink) {
  activeSink = sink && typeof sink === 'object' ? sink : null;
}

export async function emitCollectorReceipt(kind, payload, context = {}) {
  const callback = activeSink?.[kind];
  if (typeof callback !== 'function') return { delivered: false, reason: 'runtime_sink_not_registered' };
  return callback(payload, context);
}
