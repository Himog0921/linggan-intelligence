import { FactGrid, InlineState, Section, SourceNote, Tech } from './Primitives';
import type { PanelContext } from './ReadingPanel';
import { limitationCopy, receiptRows } from './evidencePresent';
import styles from './Inspector.module.css';

const GROUPS = [
  ['Package', 'packageRefs'],
  ['Task', 'taskRefs'],
  ['Attempt', 'attemptRefs'],
  ['Receipt', 'receiptRefs'],
  ['Target', 'targetRefs'],
  ['Work Order', 'workOrderRefs'],
  ['Producer', 'producers'],
] as const;

/**
 * 来源轨迹：来源与执行血缘（缺哪项写「来源信息不完整」）、限制与访问边界。这一页本来就是查证层，保留机器标识；
 * 「工位与账号镜头」只显示资格状态，不展示平台账号身份。
 */
export function TracePanel({ context }: { context: PanelContext }) {
  const { inspector, channels } = context;
  const provenance = inspector.provenance;
  const receipt = channels.provenanceReceipt ?? provenance?.receipt ?? null;
  return (
    <div>
      <details className={styles.technical}>
        <summary>来源与执行编号（技术核查）</summary>
        <Section title="来源轨迹通道">
        <FactGrid rows={receiptRows(receipt)} />
      </Section>
      <Section title="来源与执行血缘">
        {provenance === null ? (
          <SourceNote>当前详情没有返回来源血缘。</SourceNote>
        ) : (
          <>
            {GROUPS.map(([label, key]) => (
              <div key={label} className={styles.provenance}>
                <strong>{label}</strong>
                {provenance[key].length > 0 ? provenance[key].map(value => <code key={value}>{value}</code>) : <span className={styles.inlineNote}>来源信息不完整</span>}
              </div>
            ))}
            <InlineState title="工位与账号镜头" code={provenance.stationAccountLensState}>
              当前详情只返回资格状态，不展示平台账号身份。
            </InlineState>
          </>
        )}
      </Section>
      <Section title="限制与访问边界">
        {inspector.limitations.length === 0 ? (
          <SourceNote>当前详情没有返回限制列表；页面不据此推断没有限制。</SourceNote>
        ) : (
          <ul className={styles.limits}>
            {inspector.limitations.map(code => (
              <li key={code}>
                {`当前限制：${limitationCopy(code)}`} <Tech>{code}</Tech>
              </li>
            ))}
          </ul>
        )}
        </Section>
      </details>
    </div>
  );
}
