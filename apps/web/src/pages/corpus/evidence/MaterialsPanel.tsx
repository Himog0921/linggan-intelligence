import { FactGrid, InlineState, Section, Tech } from './Primitives';
import type { PanelContext } from './ReadingPanel';
import { purposeLabel, receiptRows, stateMeta } from './evidencePresent';
import { mediaSlotLedger, positionText } from './mediaObjects';
import styles from './Inspector.module.css';

/**
 * 材料处理台账：逐个槽位交代本地对象、字节状态、文字处理、采集序号与副本状态，并且包含作者头像。
 * 同一份文件落成的两个槽位（封面＝正文第 1 张）在这里仍是两行。图组本身只在「作品」标签显示一次，
 * 这里不再重复一条轨道。没有本地媒体时只写明「这不表示作品没有媒体」——页面是只读的，不触发补采。
 */
export function MaterialsPanel({ context }: { context: PanelContext }) {
  const { item, inspector, channels, listItem } = context;
  const fragment = (listItem ?? item).fragment;
  const ledger = mediaSlotLedger(inspector, fragment);
  const receipt = channels.mediaReceipt ?? inspector.mediaSlotsReceipt;
  return (
    <div>
      {ledger.length === 0 ? (
        <InlineState tone="warning" title="当前没有本地媒体对象">
          这只说明本机还没有留存这个作品的媒体副本；这不表示作品没有媒体。
        </InlineState>
      ) : null}
      <details className={styles.technical}>
        <summary>媒体处理台账与回执</summary>
        <Section title="材料处理台账">
        <FactGrid rows={receiptRows(receipt)} />
        {ledger.map(object => (
          <article key={object.slotKey ?? object.index} className={styles.derivative}>
            <div className={styles.derivativeHead}>
              <strong>{`${positionText(object.index, ledger.length).split(' / ')[0]} · ${purposeLabel(object.purpose)}`}</strong>
              {object.slotKey ? <Tech>{object.slotKey}</Tech> : null}
            </div>
            <FactGrid
              rows={[
                ['本地对象', object.objectRef ?? '当前未表达'],
                ['字节状态', stateMeta(object.bytesState)[0], object.bytesState],
                ['文字处理', object.derivedState ? stateMeta(object.derivedState)[0] : '未处理', object.derivedState],
                ['采集序号', object.slot.displayOrderState === 'KNOWN' ? String(object.ordinal) : `${object.ordinal}（平台排列顺序未经验证）`],
                ['副本状态', object.slot.replicaState ?? '当前未知', object.slot.replicaState ?? 'UNKNOWN'],
              ]}
            />
          </article>
        ))}
        </Section>
      </details>
    </div>
  );
}
