import { IntelligenceIcon } from '../../../shared/IntelligenceIcon';
import type { PanelWidth } from './evidenceQuery';

export function InspectorToolIcon({name}:{name:PanelWidth|'close'}) {
  return <IntelligenceIcon name={name} className="lgi-inspector-icon"/>;
}
