// Structural/reference checks only. Meaning/support requires a separate review.
const schema = require('./contract.cjs');
function makeValidator(Ajv) {
  const structural = new Ajv({allErrors:true,strictDefaults:true}).compile(schema);
  return (output, packet) => {
    if (!structural(output)) return {ok:false,errors:structural.errors.map(x=>'schema:'+x.keyword)};
    const errors=[];
    if(output.sampleId!==packet.sampleId || output.inputFingerprint!==packet.inputFingerprint) errors.push('scope_mismatch');
    const sources=new Map(packet.sources.map(s=>[s.ref,s]));
    const comments=new Map(packet.comments.map(c=>[c.ref,c]));
    const claims=new Map();
    function evidence(items, target=null){
      for(const e of items){
        const s=sources.get(e.sourceRef)||comments.get(e.sourceRef);
        if(!s) errors.push('unknown_source');
        else if(!s.text.includes(e.quote)) errors.push('quote_mismatch');
        if(target && e.sourceRef!==target) errors.push('comment_evidence_not_target');
      }
    }
    for(const c of output.claims){
      if(claims.has(c.id)) errors.push('duplicate_claim');
      claims.set(c.id,c);evidence(c.evidence);
      if(c.kind==='hypothesis' && c.alternatives.length===0) errors.push('hypothesis_missing_alternative');
      if(c.kind==='inspiration' && c.conditions.length===0) errors.push('inspiration_missing_conditions');
    }
    const seen=new Set();
    for(const c of output.comments){
      const source=comments.get(c.commentId);
      if(!source) errors.push('comment_not_in_input');
      if(seen.has(c.commentId)) errors.push('duplicate_comment');
      seen.add(c.commentId);evidence(c.evidence,c.commentId);
      if(source && source.isNoteAuthor !== (c.responseType==='author_response')) errors.push('comment_role_mismatch');
      if(c.measurement && ((c.measurement.kind==='unknown') !== (c.measurement.value===null))) errors.push('measurement_unknown_mismatch');
    }
    if(seen.size!==comments.size || [...comments.keys()].some(k=>!seen.has(k))) errors.push('comment_coverage_mismatch');
    const allIds=[...output.reading.overviewClaimIds,...output.reading.distinctiveClaimIds,...output.tags.flatMap(t=>t.claimIds),...output.uses.flatMap(u=>u.claimIds)];
    if(allIds.some(id=>!claims.has(id))) errors.push('claim_reference_missing');
    if(output.status==='no_readable_material'){
      if(packet.sources.length || packet.comments.length || output.claims.length || allIds.length) errors.push('false_no_material');
    }else if(!output.claims.length || !output.reading.overviewClaimIds.length)errors.push('analyzed_without_claims');
    return {ok:errors.length===0,errors:[...new Set(errors)]};
  };
}
module.exports={makeValidator};
