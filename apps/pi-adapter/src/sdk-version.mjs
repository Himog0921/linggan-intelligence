import fs from 'node:fs';
import PROTOCOL from '../../../crates/contracts/src/ai_protocol.json' with { type: 'json' };
export function pinnedSdkAvailable() {
  try {
    return ['@earendil-works/pi-ai','@earendil-works/pi-agent-core'].every(name => {
      const metadata=JSON.parse(fs.readFileSync(new URL('../package.json',import.meta.resolve(name)),'utf8'));
      return metadata.name===name&&metadata.version===PROTOCOL.sdkVersion;
    });
  } catch { return false; }
}
