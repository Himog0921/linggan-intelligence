import test from 'node:test';
import assert from 'node:assert/strict';

import {
  collectComments,
  collectCommentImages,
  initializeCollectedComments,
  parseCommentTextTail,
  resolveCommentContinuationHint,
  resolveXhsCommentTargetIdentity,
  rewindCommentSurface,
  shouldContinueDomAfterApi,
  shouldFallbackToDomAfterFreshApiFailure,
  shouldNudgeCommentSurfaceFromBottom,
} from '../src/platforms/xhs/commentCollector.js';
import { COMMENT_DEPTH_MODE } from '../src/shared/constants.js';

test('initializeCollectedComments seeds API comments for later DOM continuation without duplicates', () => {
  const seeded = initializeCollectedComments([
    { commentId: 'root_1', level: 1 },
    { commentId: 'reply_1', level: 2 },
    { commentId: 'reply_1', level: 2 },
    { commentId: '', level: 2 },
  ]);

  assert.deepEqual(seeded.allComments.map((item) => item.commentId), ['root_1', 'reply_1']);
  assert.deepEqual([...seeded.seenIds], ['root_1', 'reply_1']);
});

test('comment target identity never treats an unobserved or different page as matched', () => {
  assert.equal(resolveXhsCommentTargetIdentity({
    expectedNoteId: 'expected',
    currentUrl: 'https://www.xiaohongshu.com/explore/expected',
  }), 'matched');
  assert.equal(resolveXhsCommentTargetIdentity({
    expectedNoteId: 'expected',
    currentUrl: 'https://www.xiaohongshu.com/explore/different',
  }), 'mismatched');
  assert.equal(resolveXhsCommentTargetIdentity({
    expectedNoteId: 'expected',
    currentUrl: 'https://www.xiaohongshu.com/explore?source=webshare',
  }), 'unverified');
});

test('shouldContinueDomAfterApi keeps reply continuation and visible top-up paths alive', () => {
  assert.equal(shouldContinueDomAfterApi({
    depthMode: COMMENT_DEPTH_MODE.ALL_REPLIES,
    hydrationDegraded: true,
    hasExpandableReplies: true,
  }), true);

  assert.equal(shouldContinueDomAfterApi({
    depthMode: COMMENT_DEPTH_MODE.ALL_REPLIES,
    hydrationDegraded: false,
    hasExpandableReplies: true,
  }), false);

  assert.equal(shouldContinueDomAfterApi({
    depthMode: COMMENT_DEPTH_MODE.TWO_LEVEL,
    hydrationDegraded: true,
    hasExpandableReplies: true,
  }), false);

  assert.equal(shouldContinueDomAfterApi({
    depthMode: COMMENT_DEPTH_MODE.TWO_LEVEL,
    currentTotal: 16,
    maxTotal: 20,
    commentHint: 27,
    hasDomComments: true,
  }), true);

  assert.equal(shouldContinueDomAfterApi({
    depthMode: COMMENT_DEPTH_MODE.TWO_LEVEL,
    currentTotal: 16,
    maxTotal: 20,
    commentHint: 27,
    hasDomComments: false,
  }), false);

  assert.equal(shouldContinueDomAfterApi({
    depthMode: COMMENT_DEPTH_MODE.TWO_LEVEL,
    currentTotal: 0,
    maxTotal: 20,
    commentHint: 27,
    hasDomComments: true,
  }), true);

  assert.equal(shouldContinueDomAfterApi({
    depthMode: COMMENT_DEPTH_MODE.ALL_REPLIES,
    currentTotal: 17,
    maxTotal: 0,
    commentHint: 492,
    hasDomComments: true,
  }), true);

  assert.equal(shouldContinueDomAfterApi({
    depthMode: COMMENT_DEPTH_MODE.ALL_REPLIES,
    currentTotal: 492,
    maxTotal: 0,
    commentHint: 492,
    hasDomComments: true,
  }), false);
});

test('a failed fresh API attempt falls through to DOM parsing before all-replies expansion', () => {
  assert.equal(shouldFallbackToDomAfterFreshApiFailure({
    freshAttemptStarted: true,
    freshAttemptReady: false,
    apiObserved: false,
    currentTotal: 0,
  }), true);

  assert.equal(shouldFallbackToDomAfterFreshApiFailure({
    freshAttemptStarted: true,
    freshAttemptReady: true,
    apiObserved: false,
    currentTotal: 0,
  }), false);
  assert.equal(shouldFallbackToDomAfterFreshApiFailure({
    freshAttemptStarted: true,
    freshAttemptReady: false,
    apiObserved: true,
    currentTotal: 0,
  }), false);
  assert.equal(shouldFallbackToDomAfterFreshApiFailure({
    freshAttemptStarted: true,
    freshAttemptReady: false,
    apiObserved: false,
    currentTotal: 12,
  }), false);
});

test('known public count survives when the current DOM heading cannot be parsed', () => {
  assert.equal(resolveCommentContinuationHint(0, 492), 492);
  assert.equal(resolveCommentContinuationHint(493, 492), 493);
  assert.equal(resolveCommentContinuationHint(0, null), 0);
});

test('DOM fallback rewinds the comment surface before reading a new Attempt', async (t) => {
  const previousWindow = globalThis.window;
  const events = [];
  const scrollParent = {
    parentElement: null,
    scrollTop: 640,
    scrollTo({ top }) {
      events.push(`scroll:${top}`);
      this.scrollTop = top;
    },
  };
  globalThis.window = {
    getComputedStyle() {
      return { overflow: 'auto', overflowY: 'auto' };
    },
  };
  t.after(() => { globalThis.window = previousWindow; });

  const changed = await rewindCommentSurface({ parentElement: scrollParent }, {
    actionGate: {
      async before({ kind }) {
        events.push(`before:${kind}`);
        return true;
      },
      async after({ kind, changed: didChange }) {
        events.push(`after:${kind}:${didChange}`);
      },
    },
  });

  assert.equal(changed, true);
  assert.equal(scrollParent.scrollTop, 0);
  assert.deepEqual(events, ['before:dom_rewind', 'scroll:0', 'after:dom_rewind:true']);
});

test('an unfinished comment surface may perform only bounded bottom nudges', () => {
  const unfinished = {
    scrollTop: 50709,
    clientHeight: 647,
    scrollHeight: 51356,
    currentTotal: 396,
    pageCommentCount: 713,
    hasEndMarker: false,
    nudgeCount: 0,
  };
  assert.equal(shouldNudgeCommentSurfaceFromBottom(unfinished), true);
  assert.equal(shouldNudgeCommentSurfaceFromBottom({ ...unfinished, currentTotal: 713 }), false);
  assert.equal(shouldNudgeCommentSurfaceFromBottom({ ...unfinished, hasEndMarker: true }), false);
  assert.equal(shouldNudgeCommentSurfaceFromBottom({ ...unfinished, scrollTop: 49000 }), false);
  assert.equal(shouldNudgeCommentSurfaceFromBottom({ ...unfinished, nudgeCount: 3 }), false);
});

test('parseCommentTextTail splits visible XHS comment text without mixing metrics into content', () => {
  assert.deepEqual(parseCommentTextTail(
    '热死人了😭😭 在美国也算正常吧毕竟美国人的思想开放多了 4小时前 广东 309 回复',
    '热死人了😭😭',
  ), {
    contentText: '在美国也算正常吧毕竟美国人的思想开放多了',
    replyToNickname: '',
    timeText: '4小时前',
    ipLocation: '广东',
    likeText: '309',
    replyCount: 0,
    replyCountText: '',
  });

  assert.deepEqual(parseCommentTextTail(
    '西瓜葉 回复 热死人了😭😭 : 跟国家没关系，美国也分人 59分钟前 安徽 7 回复',
    '西瓜葉',
  ), {
    contentText: '跟国家没关系，美国也分人',
    replyToNickname: '热死人了😭😭',
    timeText: '59分钟前',
    ipLocation: '安徽',
    likeText: '7',
    replyCount: 0,
    replyCountText: '',
  });
});

test('collectComments can use API snapshot before comments container renders', async (t) => {
  const previousWindow = globalThis.window;
  const previousDocument = globalThis.document;
  const previousChrome = globalThis.chrome;
  const listeners = new Set();

  globalThis.window = {
    location: {
      href: 'https://www.xiaohongshu.com/explore/note_api_only',
      pathname: '/explore/note_api_only',
    },
    innerHeight: 800,
    addEventListener(type, listener) {
      if (type === 'message') listeners.add(listener);
    },
    removeEventListener(type, listener) {
      if (type === 'message') listeners.delete(listener);
    },
    setTimeout,
    clearTimeout,
    getComputedStyle() {
      return {
        display: 'block',
        visibility: 'visible',
        opacity: '1',
        overflow: 'visible',
        overflowY: 'visible',
      };
    },
    postMessage(message) {
      const responseByRequestType = {
        __lgboom_xhs_comment_api_reset_request__: {
          type: '__lgboom_xhs_comment_api_reset_response__',
          payload: {
            ok: true,
            noteId: 'note_api_only',
            reset: true,
          },
        },
        __lgboom_xhs_page_fetch_request__: {
          type: '__lgboom_xhs_page_fetch_response__',
          payload: {
            ok: true,
            json: {
              data: {
                comments: [{
                  id: 'comment_1',
                  content: '这是一条接口评论',
                  user_info: {
                    nickname: '评论用户',
                    user_id: 'user_1',
                  },
                }],
                has_more: false,
                cursor: '',
              },
            },
          },
        },
      };
      const response = responseByRequestType[message?.type];
      if (!response) return;
      setTimeout(() => {
        listeners.forEach((listener) => listener({
          source: globalThis.window,
          data: {
            source: 'lgboom-xhs-api-capture',
            type: response.type,
            payload: {
              requestId: message.payload.requestId,
              ...response.payload,
            },
          },
        }));
      }, 0);
    },
  };
  globalThis.document = {
    readyState: 'complete',
    body: {
      innerText: '',
      querySelectorAll() {
        return [];
      },
    },
    documentElement: {
      scrollBy() {},
      getBoundingClientRect() {
        return { top: 0, bottom: 800 };
      },
    },
    querySelector() {
      return null;
    },
    querySelectorAll() {
      return [];
    },
  };
  globalThis.chrome = {
    runtime: {
      getURL() {
        return '';
      },
    },
  };

  t.after(() => {
    globalThis.window = previousWindow;
    globalThis.document = previousDocument;
    globalThis.chrome = previousChrome;
  });

  const result = await collectComments({
    noteId: 'note_api_only',
    noteUrl: 'https://www.xiaohongshu.com/explore/note_api_only',
    maxTotal: 1,
    persist: false,
  });

  assert.equal(result.total, 1);
  assert.equal(result.comments[0].text, '这是一条接口评论');
});

test('collectComments stops immediately when the page reports an access risk', async (t) => {
  const previousWindow = globalThis.window;
  const previousDocument = globalThis.document;

  globalThis.window = {
    location: {
      href: 'https://www.xiaohongshu.com/explore/note_risk',
      pathname: '/explore/note_risk',
    },
  };
  globalThis.document = {
    body: { innerText: '操作频繁，请稍后再试' },
    querySelector() {
      return null;
    },
    querySelectorAll() {
      return [];
    },
  };

  t.after(() => {
    globalThis.window = previousWindow;
    globalThis.document = previousDocument;
  });

  const result = await collectComments({
    noteId: 'note_risk',
    noteUrl: 'https://www.xiaohongshu.com/explore/note_risk',
    persist: false,
    emitReceipt: false,
  });

  assert.equal(result.total, 0);
  assert.equal(result.stopReason, 'risk_control');
});

test('incremental budget counts stable root, parent and reply identities together, skips known and duplicate pages', async () => {
  const { boundedNewCommentSelection, normalizeIncrementalCommentBudget } = await import('../src/platforms/xhs/commentCollector.js');
  const budget = { knownCommentIds: ['known'], newUniqueLimit: 30, maxScrollRounds: 20, maxDurationSeconds: 180 };
  const rows = [{ commentId: 'known' }, { commentId: 'new-parent', level: 1 }, { commentId: 'new-reply', parentCommentId: 'new-parent', level: 2 }, ...Array.from({ length: 40 }, (_, i) => ({ commentId: `n-${i}` })), { commentId: 'new-reply', level: 2 }];
  const selected = boundedNewCommentSelection(rows, budget);
  assert.equal(selected.length, 30);
  assert.equal(selected[0].commentId, 'new-parent');
  assert.equal(selected[1].commentId, 'new-reply');
  assert.equal(new Set(selected.map((r) => r.commentId)).size, 30);
  assert.throws(() => normalizeIncrementalCommentBudget({ ...budget, newUniqueLimit: 31 }));
  assert.throws(() => normalizeIncrementalCommentBudget({ ...budget, maxScrollRounds: 0 }));
  assert.throws(() => normalizeIncrementalCommentBudget({ ...budget, knownCommentIds: ['same', 'same'] }));
});

// Execute the real image collector; page-controller mocks cannot catch lexical failures.
test('collectCommentImages enters a visible comment container without an incremental budget', async (t) => {
  const previousWindow = globalThis.window; const previousDocument = globalThis.document;
  let collecting = 0; let stop = false;
  const container = { innerText: '暂无评论', getBoundingClientRect: () => ({ width: 300, height: 300 }), querySelectorAll: () => [] };
  globalThis.window = { getComputedStyle: () => ({ display: 'block', visibility: 'visible', opacity: '1' }) };
  globalThis.document = { body: { innerText: '' }, querySelector: () => null, querySelectorAll: (selector) => selector === '.comments-container' ? [container] : [] };
  t.after(() => { globalThis.window = previousWindow; globalThis.document = previousDocument; });
  const result = await collectCommentImages({ shouldStop: () => stop, onProgress: () => { collecting += 1; stop = true; } });
  assert.ok(collecting > 0); assert.equal(result.total, 0); assert.equal(result.stopReason, 'manual_stop');
});

// The actual API-to-DOM fallback must not manufacture platform IDs for the new-ID allowance.
test('incremental actual DOM fallback excludes synthetic root and reply IDs', async (t) => {
 const saved = { window:globalThis.window,document:globalThis.document,chrome:globalThis.chrome };
 const node = (id, text) => ({ dataset: id ? { commentId:id } : {}, innerText:text, querySelector: () => null, querySelectorAll: () => [], getAttribute: () => null });
 const root = node('', '孩子每天做练习很困难'); const reply = node('', '每天都需要提醒'); const stable = node('stable-root', '怎样开始合成练习');
 const parents = [{ querySelector: () => root, querySelectorAll: () => [reply] }, { querySelector: () => stable, querySelectorAll: () => [] }];
 const container = { innerText:'共 3 条评论 - THE END -', scrollTop:0, scrollHeight:300, clientHeight:300, getBoundingClientRect: () => ({ width:300,height:300 }), querySelectorAll: (selector) => selector === '.parent-comment' || selector === '.parent-comment, .comment-item' ? parents : [], querySelector: () => null };
 globalThis.window = { location:{href:'https://www.xiaohongshu.com/explore/dom-bounded',pathname:'/explore/dom-bounded'}, getComputedStyle:()=>({display:'block',visibility:'visible',opacity:'1'}) };
 globalThis.document = { body:{innerText:''}, scrollingElement:container, documentElement:container, querySelector:()=>null, querySelectorAll:(selector)=>selector === '.comments-container' ? [container] : [] };
 globalThis.chrome = { runtime:{ getURL:()=>'' } };
 t.after(()=>Object.assign(globalThis,saved));
 const result = await collectComments({ noteId:'dom-bounded',persist:false,emitReceipt:false,taskSpec:{incrementalCommentBudget:{knownCommentIds:[],newUniqueLimit:30,maxScrollRounds:3,maxDurationSeconds:180}},executionPolicy:{minimumCooldownMs:0} });
 assert.deepEqual(result.comments.map((c)=>c.commentId),['stable-root']); assert.equal(result.total,1);
});

test('DOM pagination arriving during a scan is consumed before THE END can close a segment', async (t) => {
  const saved = { window: globalThis.window, document: globalThis.document, chrome: globalThis.chrome };
  const node = (id) => ({
    id, innerText: `合成评论 ${id}`, querySelector: () => null,
    querySelectorAll: () => [], getAttribute: () => null,
  });
  const parents = Array.from({ length: 18 }, (_, i) => {
    const main = node(`root-${i}`);
    const replies = i === 0 ? Array.from({ length: 19 }, (_, n) => node(`reply-${n}`)) : [];
    return { id: `parent-${i}`, querySelector: () => main,
      querySelectorAll: (selector) => selector === '.comment-item.comment-item-sub' ? replies : [] };
  });
  let visibleRoots = 10;
  let scans = 0;
  const container = {
    get innerText() { return `共 37 条评论${visibleRoots === 18 ? ' - THE END -' : ''}`; },
    getBoundingClientRect: () => ({ width: 300, height: 300 }),
    querySelector: () => null,
    querySelectorAll(selector) {
      if (selector === '.parent-comment') {
        const snapshot = parents.slice(0, visibleRoots);
        // Each pass queries roots for parsing and again for reply controls. The third
        // query is the second parsing pass; its static NodeList predates the final page.
        if (++scans === 3) queueMicrotask(() => { visibleRoots = 18; });
        return snapshot;
      }
      return selector === '.parent-comment, .comment-item' ? parents.slice(0, visibleRoots) : [];
    },
  };
  globalThis.window = { location: { href: 'https://www.xiaohongshu.com/explore/late-page', pathname: '/explore/late-page' },
    getComputedStyle: () => ({ display: 'block', visibility: 'visible', opacity: '1' }) };
  globalThis.document = { body: { innerText: '' }, documentElement: container,
    querySelector: () => null, querySelectorAll: (selector) => selector === '.comments-container' ? [container] : [] };
  globalThis.chrome = { runtime: { getURL: () => '' } };
  t.after(() => Object.assign(globalThis, saved));
  const collect = (knownCommentIds, maxScrollRounds = 10) => collectComments({ noteId: 'late-page', observedNoteId: 'late-page',
    persist: false, emitReceipt: false, executionPolicy: { minimumCooldownMs: 0 },
    taskSpec: { commentMode: 'all_public', incrementalCommentBudget: {
      knownCommentIds, newUniqueLimit: 30, maxScrollRounds, maxDurationSeconds: 180,
    } },
  });
  const first = await collect([]);
  assert.equal(first.total, 30, 'must read the late roots instead of claiming 29 is complete');
  assert.equal(first.collectionReceipt.state, 'partial');
  assert.equal(first.traversal.mainExhausted, false);
  const second = await collect(first.comments.map((c) => c.commentId));
  assert.equal(second.total, 7);
  assert.equal(second.collectionReceipt.state, 'complete');
  assert.equal(new Set([...first.comments, ...second.comments].map((c) => c.commentId)).size, 37);

  visibleRoots = 10;
  scans = 0;
  const exhausted = await collect([], 3);
  assert.equal(exhausted.total, 29);
  assert.equal(exhausted.collectionReceipt.state, 'partial');
  assert.equal(exhausted.traversal.mainExhausted, false);
  assert.equal(exhausted.traversal.repliesExhausted, false);
  assert.equal(exhausted.stopReason, 'no_progress');
  const remaining = await collect(exhausted.comments.map((c) => c.commentId));
  assert.equal(remaining.total, 8);
  assert.equal(remaining.collectionReceipt.state, 'complete');
});

// A real API collector traverses one root with 64 replies over bounded segments.
test('all_public progresses beyond 30 and two replies with traversal evidence', async (t) => {
  const saved={window:globalThis.window,document:globalThis.document,chrome:globalThis.chrome};
  const listeners=new Set();let resets=0;
  const root={id:'deep-root',content:'合成有意义主评论',sub_comment_has_more:false,sub_comment_count:64,sub_comments:Array.from({length:64},(_,n)=>({id:`deep-reply-${n}`,content:`合成有意义回复 ${n}`}))};
  globalThis.window={location:{href:'https://www.xiaohongshu.com/explore/deep-api-fixture',pathname:'/explore/deep-api-fixture'},innerHeight:800,setTimeout,clearTimeout,
    addEventListener(type,l){if(type==='message')listeners.add(l);},removeEventListener(type,l){if(type==='message')listeners.delete(l);},
    getComputedStyle:()=>({display:'block',visibility:'visible',opacity:'1',overflow:'visible',overflowY:'visible'}),
    postMessage(message){
      if(message.type==='__lgboom_xhs_comment_api_reset_request__')resets++;
      const type=message.type==='__lgboom_xhs_comment_api_reset_request__'?'__lgboom_xhs_comment_api_reset_response__':message.type==='__lgboom_xhs_page_fetch_request__'?'__lgboom_xhs_page_fetch_response__':message.type==='__lgboom_xhs_comment_api_request__'?'__lgboom_xhs_comment_api_response__':null;
      if(!type)return;
      setTimeout(()=>listeners.forEach(l=>l({source:globalThis.window,data:{source:'lgboom-xhs-api-capture',type,payload:{requestId:message.payload.requestId,ok:true,noteId:'deep-api-fixture',reset:true,pages:[{noteId:'deep-api-fixture',comments:[root],hasMore:false,cursor:''}],subPages:[],json:{data:{comments:[root],has_more:false,cursor:''}}}}})),0);
    }};
  const empty={innerText:'',querySelectorAll:()=>[],querySelector:()=>null};
  globalThis.document={readyState:'complete',body:empty,documentElement:{scrollBy(){},getBoundingClientRect:()=>({top:0,bottom:800})},querySelector:()=>null,querySelectorAll:()=>[]};
  globalThis.chrome={runtime:{getURL:()=>''}};t.after(()=>Object.assign(globalThis,saved));
  const known=[];
  for(const expected of [30,30,5]) {
    let snapshots=0;
    const result=await collectComments({noteId:'deep-api-fixture',observedNoteId:'deep-api-fixture',commentSessionKey:'run:account',resumeCommentSessionKey:known.length?'run:account':'',persist:false,emitReceipt:false,executionPolicy:{minimumCooldownMs:0},onSnapshot:async(snapshot)=>{snapshots++;assert.equal(snapshot.collectionReceipt.scope,'all_public_comments');},taskSpec:{commentMode:'all_public',incrementalCommentBudget:{knownCommentIds:[...known],newUniqueLimit:30,maxScrollRounds:50,maxDurationSeconds:180}}});
    assert.equal(result.total,expected);assert(snapshots>0);known.push(...result.comments.map(c=>c.commentId));
    if(known.length<65)assert.equal(result.collectionReceipt.state,'partial');
    else {assert.equal(result.collectionReceipt.state,'complete');assert.equal(result.traversal.mainExhausted,true);assert.equal(result.traversal.repliesExhausted,true);}
  }
  assert.equal(new Set(known).size,65);assert.equal(resets,1,'valid same-run/account cursor resumes without resetting the API');
  const sample=await collectComments({noteId:'deep-api-fixture',observedNoteId:'deep-api-fixture',persist:false,emitReceipt:false,maxTotal:30,maxSubComments:2,executionPolicy:{minimumCooldownMs:0},taskSpec:{commentMode:'sample_30'}});
  assert.equal(sample.total,3,'sample keeps one root and at most two replies');
  const resumed=await collectComments({noteId:'deep-api-fixture',observedNoteId:'deep-api-fixture',persist:false,emitReceipt:false,maxTotal:20,maxSubComments:2,executionPolicy:{minimumCooldownMs:0},taskSpec:{commentMode:'sample_30',incrementalCommentBudget:{knownCommentIds:['deep-root','deep-reply-0'],knownReplyCounts:{'deep-root':1},newUniqueLimit:20,maxScrollRounds:50,maxDurationSeconds:180}}});
  assert.deepEqual(resumed.comments.map(c=>c.commentId),['deep-reply-1'],'a resumed sample preserves the total per-root two reply cap');
  const exhausted=await collectComments({noteId:'deep-api-fixture',observedNoteId:'deep-api-fixture',persist:false,emitReceipt:false,maxTotal:20,maxSubComments:2,executionPolicy:{minimumCooldownMs:0},taskSpec:{commentMode:'sample_30',incrementalCommentBudget:{knownCommentIds:['deep-root','deep-reply-0','deep-reply-1'],knownReplyCounts:{'deep-root':2},newUniqueLimit:20,maxScrollRounds:50,maxDurationSeconds:180}}});
  assert.equal(exhausted.total,0,'no third reply enters a standard sample after restart');
});


test('reply exhaustion needs per-root evidence and missing identities prevent a complete proof', async()=>{
 const {apiReplyTraversalExhausted}=await import('../src/platforms/xhs/commentCollector.js');
 assert.equal(apiReplyTraversalExhausted({pages:[{comments:[{id:'r',sub_comment_has_more:true}]}]}),false);
 assert.equal(apiReplyTraversalExhausted({pages:[{comments:[{id:'r'}]}]}),false);
 assert.equal(apiReplyTraversalExhausted({pages:[{comments:[{sub_comment_has_more:false}]}]}),false);
 assert.equal(apiReplyTraversalExhausted({pages:[{comments:[{id:'r',sub_comment_has_more:true}]}],subPages:[{rootCommentId:'r',hasMore:false}]}),true);
});


test('repeated API pages leave budget for actual DOM pagination of missing roots', async (t) => {
  const saved = { window: globalThis.window, document: globalThis.document, chrome: globalThis.chrome };
  const listeners = new Set();
  let fetches = 0;
  let domScans = 0;
  let loaded = false;
  const root = { id: 'known-root', content: '合成已知评论', sub_comment_has_more: false, sub_comment_count: 0 };
  const parents = ['known-root', 'missing-root'].map(id => ({
    id: `parent-${id}`, querySelector: () => ({ id, innerText: `合成评论 ${id}`,
      querySelector: () => null, querySelectorAll: () => [], getAttribute: () => null }),
    querySelectorAll: () => [],
  }));
  const scrollParent = { scrollTop: 0, clientHeight: 300, scrollHeight: 600, parentElement: null,
    scrollBy({ top }) { this.scrollTop += top; loaded = true; },
  };
  const container = {
    parentElement: scrollParent,
    get innerText() { return `共 2 条评论${loaded ? ' - THE END -' : ''}`; },
    getBoundingClientRect: () => ({ width: 300, height: 300 }),
    querySelector: () => parents[0],
    querySelectorAll(selector) {
      if (selector === '.parent-comment') { domScans++; return parents.slice(0, loaded ? 2 : 1); }
      return selector === '.parent-comment, .comment-item' ? parents.slice(0, loaded ? 2 : 1) : [];
    },
  };
  globalThis.window = {
    location: { href: 'https://www.xiaohongshu.com/explore/repeated-api', pathname: '/explore/repeated-api' },
    innerHeight: 800, setTimeout, clearTimeout,
    addEventListener(type, listener) { if (type === 'message') listeners.add(listener); },
    removeEventListener(type, listener) { listeners.delete(listener); },
    getComputedStyle: element => ({ display: 'block', visibility: 'visible', opacity: '1',
      overflow: 'visible', overflowY: element === scrollParent ? 'scroll' : 'visible' }),
    postMessage(message) {
      const type = { __lgboom_xhs_comment_api_reset_request__: '__lgboom_xhs_comment_api_reset_response__',
        __lgboom_xhs_page_fetch_request__: '__lgboom_xhs_page_fetch_response__',
        __lgboom_xhs_comment_api_request__: '__lgboom_xhs_comment_api_response__' }[message.type];
      if (!type) return;
      if (message.type === '__lgboom_xhs_page_fetch_request__') fetches++;
      queueMicrotask(() => listeners.forEach(listener => listener({ source: window, data: {
        source: 'lgboom-xhs-api-capture', type, payload: { requestId: message.payload.requestId,
          ok: true, reset: true, noteId: 'repeated-api',
          pages: [{ noteId: 'repeated-api', comments: [root], hasMore: true, cursor: 'same-cursor' }], subPages: [],
          json: { data: { comments: [root], has_more: true, cursor: 'same-cursor' } },
        },
      } })));
    },
  };
  globalThis.document = { readyState: 'complete', body: { innerText: '' }, documentElement: container,
    querySelector: () => null, querySelectorAll: selector => selector === '.comments-container' ? [container] : [] };
  globalThis.chrome = { runtime: { getURL: () => '' } };
  t.after(() => Object.assign(globalThis, saved));
  const result = await collectComments({ noteId: 'repeated-api', observedNoteId: 'repeated-api',
    persist: false, emitReceipt: false, executionPolicy: { minimumCooldownMs: 0 },
    taskSpec: { commentMode: 'all_public', incrementalCommentBudget: { knownCommentIds: ['known-root'],
      newUniqueLimit: 11, maxScrollRounds: 8, maxDurationSeconds: 180 } },
  });
  assert.deepEqual(result.comments.map(c => c.commentId), ['missing-root']);
  assert.ok(domScans > 0);
  assert.ok(fetches < 8, 'repeated API pages must hand over before the shared budget is exhausted');
  assert.equal(result.collectionReceipt.state, 'complete');
});
