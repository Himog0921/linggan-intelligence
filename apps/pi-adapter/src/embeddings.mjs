// OpenAI-compatible embeddings transport. Positional identities are validated before any
// vector is accepted; null retains a missing/invalid member without shifting later inputs.
export class EmbeddingError extends Error { constructor(code){super(code);this.code=code;} }
export async function embed(r, base, transport, usage) {
  const input=JSON.parse(r.prompt);
  if(!Array.isArray(input)||input.length<1||input.length>20||input.some(s=>typeof s!=='string'||!s.trim()||Buffer.byteLength(s)>8000)||input.reduce((n,s)=>n+Buffer.byteLength(s),0)>16000)throw new EmbeddingError('invalid_embedding_input');
  const response=await transport(base.href.replace(/\/$/,'')+'/embeddings',{method:'POST',headers:{Authorization:`Bearer ${r.apiKey}`,'Content-Type':'application/json'},body:JSON.stringify({model:r.modelId,input,encoding_format:'float'})});
  const payload=await response.json();
  usage.inputTokens=Number.isSafeInteger(payload.usage?.prompt_tokens)&&payload.usage.prompt_tokens>=0?payload.usage.prompt_tokens:null;usage.outputTokens=0;
  if(!Array.isArray(payload.data)||payload.data.length>input.length)throw new EmbeddingError('invalid_embedding_output');
  const vectors=Array(input.length).fill(null), seen=new Set(), rejected=[];
  for(const row of payload.data) {
    if(!Number.isSafeInteger(row?.index)||row.index<0||row.index>=input.length||seen.has(row.index))throw new EmbeddingError('invalid_embedding_identity');
    seen.add(row.index);
    const v=row.embedding;
    if(!Array.isArray(v)||v.length<1||v.length>8192||v.some(x=>typeof x!=='number'||!Number.isFinite(x))||!Number.isFinite(v.reduce((n,x)=>n+x*x,0))||v.reduce((n,x)=>n+x*x,0)<=0) {
      rejected.push({index:row.index,code:'invalid_vector'});continue;
    }
    vectors[row.index]=v;
  }
  for(let index=0;index<input.length;index++)if(!seen.has(index))rejected.push({index,code:'missing_vector'});
  const dimensions=vectors.find(Array.isArray)?.length??null;
  return {text:JSON.stringify({vectors,dimensions,rejected}),inputTokens:usage.inputTokens};
}
