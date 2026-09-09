// Deterministic loopback-only review responder for the P4 organization proof.
// It accepts no credential other than the fixture sentinel and derives every
// returned atom reference from the bounded review prompt it just received.
import http from 'node:http';

const server = http.createServer(async (request, response) => {
  if (request.headers.authorization !== 'Bearer SYNTHETIC-NOT-A-CREDENTIAL'
      && request.headers['x-api-key'] !== 'SYNTHETIC-NOT-A-CREDENTIAL') {
    response.writeHead(401); response.end('synthetic unauthorized'); return;
  }
  if (request.method === 'GET' && request.url === '/v1/models') {
    response.writeHead(200, {'Content-Type': 'application/json'});
    response.end(JSON.stringify({data: [{id: 'synthetic-semantic-review'}]}));
    return;
  }
  const chunks = [];
  for await (const chunk of request) chunks.push(chunk);
  const requestBody = JSON.parse(Buffer.concat(chunks));
  const content = requestBody.messages?.findLast(item => item.role === 'user')?.content;
  const prompt = JSON.parse(typeof content === 'string' ? content : content?.find(item => item.type === 'text')?.text);
  const probeComment = prompt.untrustedMaterial?.comments?.find(comment => comment.commentRef === 'C001');
  if (!probeComment && prompt.task !== 'bounded_semantic_cluster_review.v1') {
    response.writeHead(400); response.end('unexpected synthetic review task'); return;
  }
  const decision = probeComment
    ? JSON.stringify({
        comments: [{
          commentRef: 'C001', outcome: 'interpretable',
          labels: [{
            label: 'need', basis: 'explicit', contextEvidence: [],
            evidence: [{quote: probeComment.text}]
          }],
          problems: [], stances: [], contextMissing: [], uncertaintyReason: null, limitations: []
        }]
      })
    : JSON.stringify({
        decision: 'same', sameScope: 'cluster',
        name: '合成本地语义组', definition: '合成受控 review 回执只验证持久化与谱系，不构成研究结论。',
        coreAtomRefs: prompt.atoms.filter(atom => atom.sampleRole === 'core').slice(0, 5).map(atom => atom.atomRef),
        relatedAtomRefs: prompt.atoms.filter(atom => atom.sampleRole === 'boundary').slice(0, 1).map(atom => atom.atomRef)
      });
  const chunk = (delta, finishReason = null) => ({
    id: 'synthetic-semantic-review', object: 'chat.completion.chunk', created: 1,
    model: requestBody.model, choices: [{index: 0, delta, finish_reason: finishReason}]
  });
  response.writeHead(200, {'Content-Type': 'text/event-stream'});
  response.write(`data: ${JSON.stringify(chunk({role: 'assistant', content: decision}))}\n\n`);
  response.write(`data: ${JSON.stringify(chunk({}, 'stop'))}\n\n`);
  response.write(`data: ${JSON.stringify({id: 'synthetic-semantic-review', choices: [], usage: {prompt_tokens: 400, completion_tokens: 120, total_tokens: 520}})}\n\n`);
  response.end('data: [DONE]\n\n');
});

server.listen(0, '127.0.0.1', () => {
  process.stdout.write(`http://127.0.0.1:${server.address().port}/v1\n`);
});
