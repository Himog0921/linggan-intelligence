// Synthetic local child only. No network, credentials, model or platform access.
let source = '';
for await (const part of process.stdin) source += part;
const request = JSON.parse(source);
const envelope = JSON.parse(request.prompt);
const f = envelope.input.fragments[0];
if (request.prompt.includes('SYNTHETIC SLOW')) await new Promise((resolve) => setTimeout(resolve, 700));
const attack = request.prompt.includes('SYNTHETIC ATTACK');
const multi = request.prompt.includes('SYNTHETIC COMPARE');
const cross = multi ? envelope.input.fragments.filter((v) => v.field.includes('comment') || !v.fragmentId.startsWith(envelope.input.workRef)).slice(0, 3).map((v) => ({ fragmentId: v.fragmentId, start: v.start, end: Math.min(v.end, v.start + 4) })) : [];
const evidence = [{ fragmentId: attack ? 'foreign.fragment' : f.fragmentId, start: f.start, end: Math.min(f.end, f.start + 4) }];
const output = {
 contract: 'topic-map.research.v1', outcome: 'analyzed', discussions: [], scenes: [{ label: '合成家庭练习场景', evidence }],
 journey: { mainStage: 'begin_practice', involvedStages: ['begin_practice'], overlays: [], path: 'unknown', rationale: '合成来源有练习描述，身份未知', evidence },
 responseMatches: cross.some((c) => c.fragmentId.includes('.comment.')) ? [{status: 'partial', unanswered:['合成实践问题'], evidence:cross}] : [], angles: [{ label: '合成实践角度', title: '怎样开始练习', answerTask: '解释合成片段中的练习步骤', evidence: cross.length ? cross : evidence }], productOpportunities: multi ? [{need:'合成实践中的启动困难',hypothesis:'分步支持可能帮助启动',verificationQuestion:'分步是否改变启动次数',alternativeExplanation:'也可能只是观察效应',evidence:cross.length ? cross : evidence}] : [], limitations: ['SYNTHETIC / NOT EVIDENCE'],
};
process.stdout.write(JSON.stringify({ version: request.version, ok: true, text: JSON.stringify(output), failureCode: null, modelIds: null, modelListOrigin: null, usage: { inputTokens: 299, outputTokens: 100, costUsd: null }, elapsedMs: 1, diagnostic: { schemaVersion: 1, stage: 'terminal', httpStatus: 200, responseStarted: true, terminalReceived: true, receivedBytes: 100, limitKind: null, finishReason: 'stop', elapsedMs: 1, usageKnown: true, sdkErrorType: null, retryClass: 'never' } }));
