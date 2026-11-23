import { useQuery } from '@tanstack/react-query';
import { ActionButton } from '../../../shared/ActionButton';
import type { Citation, WorkDocument } from '../workbenchModel';
import { stepNames } from '../workbenchModel';
import { cardsOf, softRole, type Card } from '../deskCards';
import * as api from './assetsApi';
import type { AccountView, BridgeState, Experience, Sentence } from './assetsModel';
import { bridgeStateText, relationLabel, usableExperiences } from './assetsText';
import { listStyles } from './stylesApi';

const PREVIEW_CHARS = 64;
const short = (text: string) => { const chars = Array.from(text); return chars.length > PREVIEW_CHARS ? chars.slice(0, PREVIEW_CHARS).join('') + '…' : text; };

/**
 * The 素材 tab of the writing sidebar: what this draft can draw on — its desk cards, the account's experiences and
 * bridge sentences, and the reference assets (persona facts, product card, style). It only reads; every insert goes
 * through the same edit (or placement dialog) the rest of the page uses, so sources and checks keep following.
 */
export function MaterialsTab({ document, domainRef, account, bridgeState, bridgeItems, bridgeFailed = false, enabled, locked, onUseBridge, onInsertExperience, onPlaceCard, onGoDesk, onOpenAssets, onOpenSource }: {
  document: WorkDocument; domainRef: string; account: AccountView | null; bridgeState: BridgeState | null; bridgeItems: readonly Sentence[] | null; bridgeFailed?: boolean;
  /** The tab is showing and the page may display personal assets (not mid-withdrawal or re-check). */
  enabled: boolean; locked: boolean;
  onUseBridge: (sentence: Sentence) => void; onInsertExperience: (experience: Experience) => void; onPlaceCard: (blockId: string) => void;
  onGoDesk: () => void; onOpenAssets: (kind: 'experience' | 'persona') => void; onOpenSource?: (source: Citation) => void;
}) {
  const accountRef = document.accountRef;
  const experiences = useQuery({ queryKey: ['creation-assets', 'experiences', accountRef], queryFn: ({ signal }) => api.listExperiences(accountRef!, signal), enabled: enabled && !!accountRef, gcTime: 0 });
  const facts = useQuery({ queryKey: ['creation-assets', 'persona-facts', accountRef], queryFn: ({ signal }) => api.listPersonaFacts(accountRef!, signal), enabled: enabled && !!accountRef, gcTime: 0 });
  const cards = useQuery({ queryKey: ['creation-assets', 'fact-cards'], queryFn: ({ signal }) => api.listFactCards(signal), enabled, gcTime: 0 });
  const styles = useQuery({ queryKey: ['creation-assets', 'styles', domainRef], queryFn: ({ signal }) => listStyles(domainRef, signal), enabled: enabled && !!accountRef, gcTime: 0 });

  if (!enabled) return <p className="aux" role="status">正在核验稿件，经历和事实暂时不显示。</p>;

  const deskCards = cardsOf(document);
  const usedCards = new Set(document.uses.map(use => use.canvasId));
  const name = account?.accountName ?? '这个账号';
  const identity = account?.identity ?? null;
  const noPerson = identity?.hasRealPerson === false;

  const cardRow = (card: Card) => {
    const used = usedCards.has(card.id);
    const canInsert = !used && !locked && card.readable && !(card.note && !card.text.trim());
    const citation = card.excerptId ? document.excerpts.find(item => item.id === card.excerptId)?.citation : undefined;
    return <div className="mat-row" key={card.id}>
      <div className="mat-top">
        <span className={`role-chip${softRole(card.group) ? ' soft' : ''}`}>{card.role}</span>
        <span className="aux">{card.kind}</span>
        <span className="grow" />
        {citation && onOpenSource ? <ActionButton variant="quiet" onClick={() => onOpenSource(citation)}>回到原文 ↗</ActionButton> : null}
        {used ? <span className="aux">本篇已用</span>
          : canInsert ? <ActionButton variant="quiet" aria-label={`插入正文：${short(card.text).slice(0, 20)}`} onClick={() => onPlaceCard(card.id)}>插入正文</ActionButton> : null}
      </div>
      <div className="mat-text">{short(card.text)}</div>
      {card.memo ? <div className="mat-memo">思路：{card.memo}</div> : null}
    </div>;
  };

  const experienceRow = (item: Experience, insertable: Set<string>) => {
    const used = document.experienceSlots.some(slot => slot.experienceRef === item.id && slot.experienceRevision === item.revision);
    return <div className="exp-mini paper" key={item.id}>
      <b><span className="data muted">{item.code}</span> {item.title ?? '未命名经历'}</b>
      {item.body}
      <div className="row">
        {item.state === 'confirmed' ? <span className="chip ok">已确认</span> : <span className="chip warn">待本人确认</span>}
        {item.usedCount ? <span className="aux">{item.usedCount} 篇已保存稿件引用</span> : null}
        <span className="grow" style={{ flex: 1 }} />
        {used ? <span className="aux">本篇已用</span>
          : insertable.has(item.id) ? <ActionButton variant="outline" className="sm" disabled={locked} aria-label={`插入经历 ${item.code}`} onClick={() => onInsertExperience(item)}>插入</ActionButton> : null}
      </div>
    </div>;
  };

  const listed = (experiences.data ?? []).filter(item => item.currentPerson && (item.state === 'confirmed' || item.state === 'awaiting_subject'));
  const insertable = new Set(usableExperiences(document, listed).map(item => item.id));
  const experienceBody = !accountRef ? <p className="aux">先为这篇选择账号。</p>
    : noPerson ? <p className="aux">这个账号背后没有真人，不用第一人称经历。</p>
      : experiences.isPending ? <p className="aux" role="status">正在读取经历…</p>
        : experiences.isError ? <p className="aux" role="alert">经历读取失败。<ActionButton variant="quiet" onClick={() => void experiences.refetch()}>重试</ActionButton></p>
          : listed.length ? listed.map(item => experienceRow(item, insertable)) : <p className="aux">还没有经历。</p>;

  const group = identity && (identity.orgRelation === 'parent' || identity.orgRelation === 'coach') ? relationLabel[identity.orgRelation] : null;
  const onBridge = document.bridge && !document.bridge.skip ? document.bridge.sentence?.sentenceCode ?? null : null;
  const bridgeBody = !bridgeState ? <p className="aux" role="status">正在核对衔接句…</p>
    : bridgeState !== 'ready' ? <p className="aux">{bridgeStateText[bridgeState]}</p>
      : bridgeFailed ? <p className="aux" role="alert">衔接句读取失败。</p>
      : bridgeItems === null ? <p className="aux" role="status">正在读取衔接句…</p>
        : bridgeItems.length ? <ul className="mini-list">{bridgeItems.map(item => <li key={item.code}>
          <span className="grow">{item.text}</span>
          {onBridge === item.code ? <span className="chip ok">在用</span>
            : <ActionButton variant="quiet" disabled={locked} aria-label={`用这句：${short(item.text).slice(0, 20)}`} onClick={() => onUseBridge(item)}>用这句</ActionButton>}
        </li>)}</ul> : <p className="aux">这个身份还没有审核通过的衔接句，先去资产库的“衔接句”里添加并审核。</p>;

  const currentFacts = (facts.data ?? []).filter(item => item.currentPerson && item.state === 'active');
  const factChip = (status: string | null) => status === 'confirmed' ? null : status === 'missing' ? <span className="chip unk">缺</span> : <span className="chip warn">待确认</span>;

  const stepsUsed = new Set(document.paragraphSteps.map(item => item.step));
  const productCard = cards.data?.find(item => item.confirmed)?.confirmed ?? null;
  const style = (styles.data ?? []).find(item => item.accountRef === accountRef) ?? null;

  return <>
    <div className="r-sec">
      <h3>选题框{document.ideaName ? <> · {document.ideaName}</> : null}<span className="grow" /><ActionButton variant="quiet" onClick={onGoDesk}>去画布整理</ActionButton></h3>
      <p className="brief paper">{document.angle ? document.angle : '还没写“这篇想说什么”。'}</p>
      {!document.ideaRef ? <p className="aux">这篇还没有关联选题框，可以在稿件上方的“补一个选题框”里建立或关联。</p> : null}
      {deskCards.length ? deskCards.map(cardRow) : <p className="aux">还没有材料。可以去画布找材料，放进这一篇。</p>}
    </div>
    <div className="r-sec">
      <h3>经历 · {name}<span className="grow" /><ActionButton variant="quiet" onClick={() => onOpenAssets('experience')}>去采集</ActionButton></h3>
      {experienceBody}
    </div>
    <div className="r-sec">
      <h3>衔接句{group ? ` · ${group}` : ''}</h3>
      {bridgeBody}
    </div>
    <details className="r-sec r-fold"><summary>人设事实{accountRef ? ` · ${currentFacts.length} 条` : ''}</summary>
      {!accountRef ? <p className="aux">先为这篇选择账号。</p>
        : facts.isPending ? <p className="aux" role="status">正在读取事实…</p>
          : facts.isError ? <p className="aux" role="alert">事实读取失败。</p>
            : currentFacts.length ? <ul className="mini-list">{currentFacts.map(item => <li key={item.id}><span className="grow">{item.status === 'missing' ? item.key ?? '未命名事实' : item.text}</span>{factChip(item.status)}</li>)}</ul> : <p className="aux">还没有人设事实。</p>}
      {accountRef ? <ActionButton variant="quiet" onClick={() => onOpenAssets('persona')}>去补充</ActionButton> : null}
    </details>
    <details className="r-sec r-fold"><summary>写法步骤 · 已用 {stepsUsed.size}/{Object.keys(stepNames).length}</summary>
      <div className="arc">{Object.entries(stepNames).map(([key, label]) => <span key={key} className={stepsUsed.has(key as keyof typeof stepNames) ? 'on' : ''}>{label}</span>)}</div>
      <p className="aux">按正文各段标注的步骤统计。用样板开稿还没有接入。</p>
    </details>
    <details className="r-sec r-fold"><summary>产品事实卡</summary>
      {cards.isPending ? <p className="aux" role="status">正在读取…</p>
        : cards.isError ? <p className="aux" role="alert">读取失败。</p>
          : productCard ? <ul className="mini-list">
            <li><span className="grow">正文可用：{productCard.allowedNames.join('、')}</span></li>
            <li><span className="grow">不能讲：{productCard.cannotSay}</span></li>
          </ul> : <p className="aux">还没有已确认的产品事实卡，叫法检查不会运行。</p>}
    </details>
    {style ? <details className="r-sec r-fold"><summary>风格 · {style.name}</summary>
      <ul className="trait">{style.rhythm.concat(style.wording).map((item, index) => <li key={`${index}:${item}`}>{item}</li>)}</ul>
    </details> : null}
  </>;
}
