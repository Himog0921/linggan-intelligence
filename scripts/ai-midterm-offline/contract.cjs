// Offline candidate only; not imported by production. JSON Schema draft-07.
const text = {type:'string',minLength:1,maxLength:1600};
const arr = (items,maxItems=30,minItems=0) => ({type:'array',items,minItems,maxItems});
const obj = properties => ({type:'object',additionalProperties:false,required:Object.keys(properties),properties});
const refs = arr(obj({sourceRef:text,quote:text}),12,1);
const ids = arr(text,12,1);
const schema = {$schema:'http://json-schema.org/draft-07/schema#', ...obj({
  contract:{const:'note-understanding.candidate.v0.1'},
  sampleId:text,
  inputFingerprint:{type:'string',pattern:'^[a-f0-9]{64}$'},
  status:{enum:['analyzed','partial','no_readable_material']},
  claims:arr(obj({id:text,kind:{enum:['observation','author_claim','hypothesis','inspiration']},
    text,evidence:refs,conditions:arr(text,8),alternatives:arr(text,5)}),24),
  reading:obj({overviewClaimIds:arr(text,6),distinctiveClaimIds:arr(text,6)}),
  comments:arr(obj({commentId:text,responseType:{enum:['author_response','intention','reported_result','question','evaluation','social','ambiguous','other']},
    summary:text,evidence:refs,measurement:{anyOf:[{type:'null'},obj({kind:{enum:['elapsed_seconds','remaining_seconds','progress','unknown']},value:{type:['number','null'],minimum:0}})]}}),100),
  tags:arr(obj({dimension:{enum:['situation','answer_form','interaction','material_form']},label:text,claimIds:ids,retrievalUse:text}),8),
  uses:arr(obj({text,claimIds:ids,conditions:arr(text,8,1)}),5),
  limitations:arr(obj({reason:text,affects:text}),12,1)
})};
module.exports = schema;
