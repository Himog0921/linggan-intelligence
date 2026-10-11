// Synthetic local child only. No network, credentials, model or platform access.
let source = '';
for await (const part of process.stdin) source += part;
const request = JSON.parse(source);
const envelope = JSON.parse(request.prompt);
const input = envelope.input;
const fragments = input.fragments ?? [];
const sourceText = fragments.map((f) => f.text).join('\n');
const slowUnknown = sourceText.includes('SYNTHETIC SLOW_UNKNOWN');
if (sourceText.includes('SYNTHETIC SLOW')) await new Promise((resolve) => setTimeout(resolve, slowUnknown ? 2000 : 700));
if (slowUnknown) {
  process.stdout.write(JSON.stringify({
    version: request.version, ok: false, text: null, failureCode: 'provider_transport', modelIds: null, modelListOrigin: null,
    usage: { inputTokens: null, outputTokens: null, costUsd: null }, elapsedMs: 2000,
    diagnostic: { schemaVersion: 1, stage: 'failed', httpStatus: null, responseStarted: null, terminalReceived: null,
      receivedBytes: null, limitKind: null, finishReason: null, elapsedMs: 2000, usageKnown: false, sdkErrorType: 'network', retryClass: 'unknown' },
  }));
  process.exit(0);
}
const attack = sourceText.includes('SYNTHETIC ATTACK');
const compare = sourceText.includes('SYNTHETIC COMPARE');
const noSignal = sourceText.includes('SYNTHETIC NO_SIGNAL');
const insufficient = sourceText.includes('SYNTHETIC INSUFFICIENT');
const authorFields = ['title', 'body', 'ocr', 'transcript'];
const commentFields = ['studied_comment', 'unresearched_comment'];
const cite = (f) => ({
  fragmentId: attack ? 'foreign.fragment' : f.fragmentId,
  start: f.start,
  end: Math.min(f.end, f.start + 1000),
});
const boundary = {
  label: '合成实践启动',
  definition: '安排并开始一次具体练习或实践的行动、困难与支持方式',
  inclusionCriteria: ['材料涉及开始练习或实践的步骤、安排或困难'],
  exclusionCriteria: ['仅谈练习开始之后的长期效果而不涉及如何开始'],
};
const sustainedBoundary = {
  label: boundary.label,
  definition: '练习已经开始以后维持注意与持续执行的困难及支持',
  inclusionCriteria: ['已经开始练习之后的走神、持续或中断'],
  exclusionCriteria: ['尚未开始练习时的启动困难'],
};
function discussion(f, text = f.text, chosenBoundary = boundary) {
  const relative = f.text.indexOf(text);
  const start = f.start + Array.from(f.text.slice(0, Math.max(0, relative))).length;
  return {
    ...chosenBoundary,
    label: text.includes('ALIAS_START') ? '拆解第一步' : chosenBoundary.label,
    statement: Array.from(text).slice(0, 1000).join(''),
    speakerRole: authorFields.includes(f.field) ? 'author' : commentFields.includes(f.field) ? 'commenter' : 'unknown',
    evidenceRole: text.includes('CHALLENGE') ? 'challenge' : 'support',
    rationale: '合成 fixture 只复述所引用原始材料；不表示真实研究判断', topicRef: null,
    evidence: [{fragmentId: attack ? 'foreign.fragment' : f.fragmentId, start, end: Math.min(f.end, start + Array.from(text).slice(0,1000).length)}],
  };
}
function extract() {
  // Only the explicit saved-source fixture model selects the last absolute slice.
  const tailAuthor = request.modelId === 'synthetic-topic-map-cite-tail'
    ? fragments.filter((f) => authorFields.includes(f.field))
      .reduce((last, f) => !last || f.start > last.start ? f : last, null)
    : null;
  const author = tailAuthor ?? fragments.find((f) => f.field === 'body' && f.fragmentId.startsWith(input.workRef))
    ?? fragments.find((f) => authorFields.includes(f.field));
  const primaryComments = fragments.filter((f) => commentFields.includes(f.field));
  const comment = primaryComments[0];
  const linked = request.modelId === 'synthetic-topic-map-cite-linked-reply'
    ? input.commentStudy?.find(c => c.parentFragmentIds?.length && primaryComments.some(f => f.fragmentId === c.fragmentId)) : null;
  const linkedChild = linked ? primaryComments.find(f => f.fragmentId === linked.fragmentId) : null;
  const primary = linkedChild ?? author ?? comment ?? fragments.find((f) => f.field !== 'parent_comment_context' && !f.field.startsWith('work_context:'));
  const evidence = primary ? [cite(primary), ...(linked?.parentFragmentIds ?? []).map(id => fragments.find(f => f.fragmentId === id)).filter(Boolean).map(cite)] : [];
  const cross = compare && author && comment ? [cite(author), cite(comment)] : evidence;
  const outcome = noSignal ? 'no_signal' : insufficient || !primary ? 'insufficient' : 'analyzed';
  let discussions = outcome === 'analyzed' ? [primary, ...primaryComments.filter(f => f !== primary)].map((f) =>
    discussion(f, f.text, f.text.includes('SUSTAIN_ONLY') ? sustainedBoundary :
      f.text.includes('PRIVATE_RULE') ? {...boundary, definition: 'SYNTHETIC_RESTRICTED_RULE：开始练习的独特合成边界'} : boundary)) : [];
  if (primary && sourceText.includes('SYNTHETIC MULTI')) {
    discussions = primary.text.split('\n').filter((line) => line && !line.includes('SYNTHETIC MULTI'))
      .map((line) => discussion(primary,line,line.includes('开始之后') ? sustainedBoundary : boundary));
  }
  const analyzed = outcome === 'analyzed';
  return {
    contract: envelope.contract, outcome, discussions,
    scenes: analyzed ? [{ label: '合成家庭练习场景', evidence }] : [],
    journey: {
      mainStage: analyzed ? 'begin_practice' : 'unclear', involvedStages: analyzed ? ['begin_practice'] : [],
      overlays: [], path: 'unknown', rationale: analyzed ? '合成来源有练习描述，身份未知' : '合成无信号或材料不足模式',
      evidence: analyzed ? evidence : [],
    },
    responseMatches: analyzed && compare && author && comment ? [{ status: 'partial', unanswered: ['合成实践问题'], evidence: cross }] : [],
    angles: analyzed ? [{ label: '合成实践角度', title: '怎样开始练习', answerTask: '解释合成片段中的练习步骤', evidence: cross }] : [],
    productOpportunities: analyzed && compare && author && comment ? [{
      need: '合成实践中的启动困难', hypothesis: '分步支持可能帮助启动', verificationQuestion: '分步是否改变启动次数',
      alternativeExplanation: '也可能只是观察效应', evidence: cross,
    }] : [],
    limitations: ['SYNTHETIC / NOT EVIDENCE'],
  };
}
function resolve() {
  const candidates = input.definitions;
  return {
    contract: envelope.contract,
    decisions: input.units.map(({ unitId, discussion }) => {
      // Deliberately compare the complete proposed boundary, never just the name.
      const matched = candidates.find((c) => c.definition === discussion.definition
        && JSON.stringify(c.inclusionCriteria) === JSON.stringify(discussion.inclusionCriteria)
        && JSON.stringify(c.exclusionCriteria) === JSON.stringify(discussion.exclusionCriteria));
      const tree = discussion.statement.includes('TREE_');
      const badParent = discussion.statement.includes('TREE_BAD_PARENT_VERSION');
      const parentConcept = {
        label: '合成任务执行支持', definition: '围绕任务从开始到持续执行的困难与支持',
        inclusionCriteria: ['已决定任务的启动或持续执行支持'],
        exclusionCriteria: ['仅讨论选哪一个任务或者任务后的满意程度'],
        domainFit: 'in_scope', domainReason: '合成领域中的实践任务执行问题',
        abstractionReason: '启动和维持是执行任务的不同阶段，未虚构其他实际材料',
      };
      const existingParent = candidates.find(c => c.definition === parentConcept.definition);
      const forceReuse = discussion.statement.includes('TREE_REUSE_REPARENT');
      const chosenMatch = forceReuse ? null : matched;
      const parent = tree ? existingParent ? {
        kind: 'existing', topicRef: existingParent.topicRef,
        definitionRef: badParent ? '00000000-0000-4000-8000-00000000dead' : existingParent.definitionRef,
        proposal: null, reason: '启动或持续是执行任务支持范围中的具体阶段',
      } : {kind:'proposed', topicRef:null, definitionRef:null, proposal:parentConcept,
        reason:'任务执行范围包含启动或持续的具体阶段，未把共现作为包含'} :
        {kind:'root', topicRef:null, definitionRef:null, proposal:null, reason:'本次无有依据的上位主题，保留根候选'};
      return {
        unitId,
        status: chosenMatch ? 'matched' : 'new',
        matches: chosenMatch ? [{ topicRef: matched.topicRef, definitionRef: matched.definitionRef, reason: '合成完整定义及纳入排除标准一致' }] : [],
        proposedTopic: chosenMatch ? null : {
          label: discussion.label, definition: discussion.definition,
          inclusionCriteria: discussion.inclusionCriteria, exclusionCriteria: discussion.exclusionCriteria,
          ...(envelope.contract === 'topic-map.resolve.v2' ? {
            domainFit:'in_scope', domainReason:'合成领域内执行任务的具体讨论',
            abstractionReason:'定义跨人物和任务可复用；现场原声及观点留在讨论，不设次数门槛', parent,
          } : {}),
        },
        relations: candidates.map((c) => ({
          topicRef: c.topicRef, definitionRef: c.definitionRef, relation: c === chosenMatch ? 'equivalent' : tree && c === existingParent ? 'narrower' : 'distinct',
          reason: c === matched ? '合成完整边界一致' : '合成候选边界与本讨论定义标准不同',
        })),
        reason: chosenMatch ? '合成定义比较后复用已有主题' : forceReuse ? '合成故意重复提案，用于验证旧身份和绑定保护' : '合成逐个比较后提出明确新边界',
      };
    }),
  };
}
if (!['topic-map.research.v2', 'topic-map.resolve.v1', 'topic-map.research.v3', 'topic-map.resolve.v2'].includes(envelope.contract)) throw new Error('unexpected synthetic contract');
const output = envelope.contract.startsWith('topic-map.resolve.') ? resolve() : extract();
process.stdout.write(JSON.stringify({
  version: request.version, ok: true, text: JSON.stringify(output), failureCode: null, modelIds: null, modelListOrigin: null,
  usage: { inputTokens: 299, outputTokens: 100, costUsd: null }, elapsedMs: 1,
  diagnostic: { schemaVersion: 1, stage: 'terminal', httpStatus: 200, responseStarted: true, terminalReceived: true,
    receivedBytes: 100, limitKind: null, finishReason: 'stop', elapsedMs: 1, usageKnown: true, sdkErrorType: null, retryClass: 'never' },
}));
