// Explicit local-only P3 fixture. It serves both frozen v4 and v5 packet contracts through the
// real Pi adapter, never receives a production credential, and stores no request material.
import http from 'node:http';

function sse(response, payload, usage = true) {
  response.writeHead(200, { 'Content-Type': 'text/event-stream' });
  const chunk = (delta, finish_reason = null) => ({
    id: 'comment-replay-fixture', object: 'chat.completion.chunk', created: 1,
    model: 'comment-replay-fixture', choices: [{ index: 0, delta, finish_reason }],
  });
  response.write(`data: ${JSON.stringify(chunk({ role: 'assistant', content: JSON.stringify(payload) }))}\n\n`);
  response.write(`data: ${JSON.stringify(chunk({}, 'stop'))}\n\n`);
  if (usage) {
    response.write(`data: ${JSON.stringify({ id: 'comment-replay-fixture', choices: [], usage: { prompt_tokens: 800, completion_tokens: 100, total_tokens: 900 } })}\n\n`);
  }
  response.end('data: [DONE]\n\n');
}

function v4(comments) {
  return { comments: comments.map((comment) => ({
    commentRef: comment.commentRef,
    outcome: 'interpretable',
    labels: [],
    problems: [{ name: '合成执行困难', meaning: '合成评论中的执行困难', basis: 'explicit', contextEvidence: [], evidence: [{ quote: comment.text }] }],
    stances: [], contextMissing: [], uncertaintyReason: null, limitations: ['SYNTHETIC / NOT EVIDENCE'],
  })) };
}

function v5(comments, invalidEvidence = false) {
  return { comments: comments.map((comment) => ({
    commentRef: comment.commentRef,
    outcome: 'interpretable',
    atoms: [{ ordinal: 1, kind: 'problem', meaning: '合成评论中的执行困难', context: null, basis: 'explicit', evidence: [{ quote: invalidEvidence ? '未在输入中出现的证据' : comment.text }], contextEvidence: [] }],
    contextMissing: [], uncertaintyReason: null, limitations: ['SYNTHETIC / NOT EVIDENCE'],
  })) };
}

const server = http.createServer(async (request, response) => {
  if (request.headers.authorization !== 'Bearer SYNTHETIC-NOT-A-CREDENTIAL') {
    response.writeHead(401); response.end('synthetic unauthorized'); return;
  }
  const chunks = [];
  for await (const chunk of request) chunks.push(chunk);
  let wire;
  try { wire = JSON.parse(Buffer.concat(chunks)); } catch { response.writeHead(400); response.end('invalid fixture request'); return; }
  const message = wire.messages?.findLast((item) => item.role === 'user')?.content;
  let prompt;
  try { prompt = JSON.parse(typeof message === 'string' ? message : message?.find((item) => item.type === 'text')?.text); }
  catch { response.writeHead(400); response.end('missing replay prompt'); return; }
  if (prompt?.pairedSanitizedResult) {
    const paired = prompt.pairedSanitizedResult;
    sse(response, {
      kind: 'shadow_difference_explanation',
      stabilityNotAccuracy: true,
      summary: '两个已脱敏影子回执的工程字段不同；这只表示稳定性差异。',
      citations: [
        { side: 'baseline', itemRef: paired.baseline?.itemRef },
        { side: 'candidate', itemRef: paired.candidate?.itemRef },
      ],
      uncertainty: '没有评论原文或人工标注，不能判断准确性。',
    });
    return;
  }
  const comments = prompt?.untrustedMaterial?.comments;
  if (!Array.isArray(comments)) { response.writeHead(400); response.end('missing comments'); return; }
  const isV5 = prompt.contract === 'comment-research.v5';
  const marker = comments.some((comment) => String(comment.text).includes('[REPLAY_PARTIAL]'));
  const healthRegression = comments.some((comment) => String(comment.text).includes('[REPLAY_HEALTH_REGRESSION]'));
  const healthSupplierFailure = comments.some((comment) => String(comment.text).includes('[REPLAY_HEALTH_SUPPLIER_FAILURE]'));
  const partialCandidateScenario = String(prompt?.purpose?.title ?? '').includes('部分失败演练');
  if (isV5 && partialCandidateScenario && marker) { response.writeHead(503); response.end('synthetic candidate-only partial failure'); return; }
  if (isV5 && healthSupplierFailure) { response.writeHead(503); response.end('synthetic active-health supplier failure'); return; }
  const unknownUsage = wire.model === 'replay-unknown';
  sse(response, isV5 ? v5(comments, healthRegression) : v4(comments), !unknownUsage);
});

server.listen(0, '127.0.0.1', () => {
  process.stdout.write(`http://127.0.0.1:${server.address().port}/v1\n`);
});
