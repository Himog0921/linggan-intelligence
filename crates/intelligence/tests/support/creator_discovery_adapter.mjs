// Fixed synthetic child. Validate the real canonical Pi request; never open a network connection.
let raw = '';
for await (const part of process.stdin) raw += part;
const request = JSON.parse(raw);
if (request.baseUrl !== 'http://127.0.0.1:1' || request.operation !== 'analyze') process.exit(2);
const input = JSON.parse(request.prompt);
if (input.materialContract !== 'research-dossier.v2' || typeof input.asOf !== 'string' || !Array.isArray(input.materials) || 'fragments' in input) process.exit(3);
for (const material of input.materials) {
  if (!material.content || material.content.ref !== material.blockRef || typeof material.sourceRef !== 'string' || typeof material.sourceVersion !== 'string' || !Array.isArray(material.issues) || 'text' in material) process.exit(4);
  if (material.content.speaker !== material.speaker) process.exit(5);
}
const body = input.materials.find(m => m.field === 'body');
const text = body?.content?.text;
const repetitions = /^CREATOR_EXPECT_(\d+):/.exec(text ?? '');
if (!repetitions || text !== `CREATOR_EXPECT_${repetitions[1]}:${'家庭实践过程。'.repeat(Number(repetitions[1]))}CREATOR_FULL_TAIL🙂`) process.exit(6);
if (!input.materials.some(m => m.field === 'ocr' && m.speaker === 'unconfirmed' && typeof m.readingRef === 'string' && typeof m.unitRef === 'string')) process.exit(7);
const field = value => ({value, reason:'合成子进程核对完整规范化来源', evidenceFragmentIds:[body.citation]});
const output = {relevance:field('related'), traits:{personal_experience:field('yes'),professional_output:field('unknown'),explicit_promotion:field('unknown')},topicHints:[]};
process.stdout.write(JSON.stringify({version:request.version,ok:true,text:JSON.stringify(output),failureCode:null,modelIds:null,modelListOrigin:null,usage:{inputTokens:111,outputTokens:22,costUsd:null},elapsedMs:1}));
