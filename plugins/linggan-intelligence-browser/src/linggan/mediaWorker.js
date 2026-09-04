import { localMediaOutbox, localProducerOutbox } from './localProducerOutbox.js';
import { flushMediaOutboxInExecutionContext } from './mediaTransferRuntime.js';
import { registerMediaWorkerPort } from './mediaWorkerChannel.js';
const mediaOutbox = {
  dueById: (...args) => localMediaOutbox.dueById(...args),
  due: (...args) => localMediaOutbox.due(...args),
  markInFlight: (...args) => localMediaOutbox.markInFlight(...args),
  acknowledge: (...args) => localMediaOutbox.acknowledge(...args),
  retry: (...args) => localMediaOutbox.retry(...args),
  terminal: (...args) => localMediaOutbox.terminal(...args),
  producerDependency: (submissionId) => localProducerOutbox.get(submissionId),
};

registerMediaWorkerPort({
  runtime: chrome.runtime,
  processMedia: ({ preferredUploadId, installationCredential }) => flushMediaOutboxInExecutionContext({
    outbox: mediaOutbox,
    preferredUploadId,
    installationCredential,
  }),
});
