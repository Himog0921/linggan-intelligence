import { IntelligenceIcon } from '../../../shared/IntelligenceIcon';
import styles from './DossierDocument.module.css';

type Name = 'clock' | 'chevron' | 'refresh' | 'download' | 'check' | 'info' | 'expand' | 'close' | 'copy';

export function DossierIcon({ name, expanded = false }: { name: Name; expanded?: boolean }) {
  return <IntelligenceIcon name={name} expanded={expanded} className={styles.icon}/>;
}
