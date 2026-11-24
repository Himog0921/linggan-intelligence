import { ActionFeedback, type ReceiptState } from '../../shared/ActionFeedback';
import { IDEA_ACCOUNT_LIMIT, type IdeaBoxSummary } from './workbenchModel';

/** One draft that already writes under the idea, for the "已开稿" list and for which accounts are taken. */
export interface IdeaDraftRow { workId: string; accountRef: string; title: string }
export interface DraftOutcome { state: ReceiptState; text: string }

/**
 * The demo's "给哪些账号写 · 可多选，一题多号" and "已开稿" of the topic frame. A new draft takes the idea's own
 * materials, so the picker is offered on the idea's anchor draft; any other draft is told where to go.
 */
export function IdeaDraftsBody({ idea, ideaState, hasIdea, isAnchor, accounts, drafted, picked, reading, opening, outcome, onPick, onOpenWork, onOpenAnchor }: {
  idea: IdeaBoxSummary | null; ideaState: 'pending' | 'error' | 'success'; hasIdea: boolean; isAnchor: boolean;
  accounts: { ref: string; name: string }[]; drafted: IdeaDraftRow[]; picked: string[]; opening: boolean; outcome: DraftOutcome | null;
  /** Whether every draft of the idea has been read, so that `drafted` is the whole of who already writes under it. */
  reading: 'ready' | 'pending' | 'failed';
  onPick: (ref: string) => void; onOpenWork: (workId: string) => void; onOpenAnchor: () => void;
}) {
  const nameOf = (ref: string) => accounts.find(account => account.ref === ref)?.name ?? '账号未知';
  const taken = new Set(drafted.map(row => row.accountRef));
  const full = taken.size + picked.length >= IDEA_ACCOUNT_LIMIT;
  const label = <p className="lbl">给哪些账号写 · 可多选，一题多号</p>;
  if (!hasIdea) return <>{label}<p className="aux">这篇还没有选题框。在写稿页点“补一个选题框”，才能一题多号。</p></>;
  if (!idea) return <>{label}<p className="aux">{ideaState === 'pending' ? '正在读取选题框…' : ideaState === 'error' ? '选题框读取失败，稍后再试。' : '先保存这篇，选题框建好后才能给别的账号开稿。'}</p></>;
  if (!isAnchor) return <>{label}
    <p className="aux">这篇挂在选题「{idea.name}」下。新稿用的是选题框原稿的材料，要给别的账号开稿，请到原稿里操作。</p>
    <button type="button" className="btn sm" onClick={onOpenAnchor}>切到选题框原稿</button>
  </>;
  if (reading !== 'ready') return <>{label}<p className="aux">{reading === 'pending' ? '正在读取这个选题下的稿件，读完才能确认哪些账号已开稿。' : '这个选题下有稿件读取失败，暂时无法确认哪些账号已开稿，刷新后再试。'}</p></>;
  return <>{label}
    <div className="acc-picks" role="group" aria-label="给哪些账号写">
      {accounts.map(account => {
        const has = taken.has(account.ref);
        const on = has || picked.includes(account.ref);
        const blocked = !on && full;
        return <button key={account.ref} type="button" aria-pressed={on} disabled={has || blocked || opening}
          title={has ? '这个账号已经开过稿' : blocked ? `一个选题最多 ${IDEA_ACCOUNT_LIMIT} 个账号` : undefined}
          onClick={() => onPick(account.ref)}>{account.name}{has ? ' · 已开稿' : ''}</button>;
      })}
    </div>
    <p className="aux">新稿带这个选题的材料和“这篇想说”，不带正文、图页和经历；来源现在读不到的材料不会带。最多 {IDEA_ACCOUNT_LIMIT} 个账号。</p>
    {outcome ? <ActionFeedback state={outcome.state}>{outcome.text}</ActionFeedback> : null}
    <p className="lbl">已开稿</p>
    {drafted.length
      ? <div className="dchips">{drafted.map(row => <button key={row.workId} type="button" className="dchip" onClick={() => onOpenWork(row.workId)}>
        <b>{row.title}</b><span className="aux">{nameOf(row.accountRef)}</span>
      </button>)}</div>
      : <p className="aux">还没有</p>}
  </>;
}

/** The demo's "用这组材料开稿 · N 个账号". */
export function IdeaDraftsAction({ count, cards, opening, onOpen }: { count: number; cards: number; opening: boolean; onOpen: () => void }) {
  const blocked = !count ? '先选择写作账号' : !cards ? '先放入材料' : undefined;
  return <button type="button" className="btn sm primary" disabled={opening || !!blocked} aria-busy={opening || undefined} title={blocked} onClick={onOpen}>
    {opening ? '起稿中…' : `用这组材料开稿${count > 1 ? ` · ${count} 个账号` : ''}`}
  </button>;
}
