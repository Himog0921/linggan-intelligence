// OpenAI-compatible embeddings transport; local endpoints use the same bounded protocol.
// https://developers.openai.com/api/reference/resources/embeddings/methods/create
export class EmbeddingError extends Error { constructor(code){super(code);this.code=code;} }
export async function embed(r, base, transport, usage) {
  const input=JSON.parse(r.prompt);
  if(!Array.isArray(input)||input.length<1||input.length>2||input.some(s=>typeof s!=='string'||!s.trim()||Buffer.byteLength(s)>8000)||input.reduce((n,s)=>n+Buffer.byteLength(s),0)>16000)throw new EmbeddingError('invalid_embedding_input');
  const response=await transport(base.href.replace(/\/$/,'')+'/embeddings',{method:'POST',headers:{Authorization:`Bearer ${r.apiKey}`,'Content-Type':'application/json'},body:JSON.stringify({model:r.modelId,input,encoding_format:'float'})});
  const payload=await response.json();
  usage.inputTokens=Number.isSafeInteger(payload.usage?.prompt_tokens)&&payload.usage.prompt_tokens>=0?payload.usage.prompt_tokens:null;usage.outputTokens=0;
  if(!Array.isArray(payload.data)||payload.data.length!==input.length)throw new EmbeddingError('invalid_embedding_output');
  const rows=[...payload.data].sort((a,b)=>a.index-b.index), dimensions=rows[0]?.embedding?.length;
  if(!Number.isInteger(dimensions)||dimensions<1||dimensions>8192||rows.some((row,i)=>row.index!==i||!Array.isArray(row.embedding)||row.embedding.length!==dimensions||row.embedding.some(v=>typeof v!=='number'||!Number.isFinite(v))||!Number.isFinite(row.embedding.reduce((n,v)=>n+v*v,0))||row.embedding.reduce((n,v)=>n+v*v,0)<=0))throw new EmbeddingError('invalid_embedding_output');
  return {text:JSON.stringify({vectors:rows.map(row=>row.embedding),dimensions}),inputTokens:Number.isSafeInteger(payload.usage?.prompt_tokens)&&payload.usage.prompt_tokens>=0?payload.usage.prompt_tokens:null};
}
