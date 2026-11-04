const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict');
const {makeValidator}=require('./validate.cjs');
const [ajvDir,outputFile]=process.argv.slice(2);
if(!ajvDir||!outputFile)throw Error('usage: node check.cjs AJV_MODULE_DIR PRIVATE_RECEIPT');
const Ajv=require(path.resolve(ajvDir)),validate=makeValidator(Ajv);
const packet={sampleId:'SYN',inputFingerprint:'a'.repeat(64),sources:[{ref:'B1',text:'作者说昨天尝试了一次。'}],comments:[
 {ref:'C1',text:'我才数到18',isNoteAuthor:false},{ref:'C2',text:'建议多练习',isNoteAuthor:true}]};
const base={contract:'note-understanding.candidate.v0.1',sampleId:'SYN',inputFingerprint:packet.inputFingerprint,status:'partial',
 claims:[{id:'k1',kind:'author_claim',text:'作者自述一次尝试。',evidence:[{sourceRef:'B1',quote:'昨天尝试了一次'}],conditions:[],alternatives:[]}],
 reading:{overviewClaimIds:['k1'],distinctiveClaimIds:[]},comments:[
 {commentId:'C1',responseType:'reported_result',summary:'报告进度到18。',evidence:[{sourceRef:'C1',quote:'我才数到18'}],measurement:{kind:'progress',value:18}},
 {commentId:'C2',responseType:'author_response',summary:'作者鼓励练习。',evidence:[{sourceRef:'C2',quote:'建议多练习'}],measurement:null}],
 tags:[{dimension:'interaction',label:'进度反馈',claimIds:['k1'],retrievalUse:'查找有过程反馈的材料；本例仅测结构'}],uses:[],limitations:[{reason:'合成材料',affects:'不证明业务质量'}]};
const results=[];
function test(name,mutate,expected,code){const o=structuredClone(base),p=structuredClone(packet);mutate(o,p);const r=validate(o,p);assert.equal(r.ok,expected,name+JSON.stringify(r));if(code)assert(r.errors.includes(code),name);results.push({name,...r,expected,matchedExpectation:true});}
test('valid_partial',()=>{},true);
test('foreign_source',o=>o.claims[0].evidence[0].sourceRef='OTHER:B1',false,'unknown_source');
test('fabricated_quote',o=>o.claims[0].evidence[0].quote='有效率100%',false,'quote_mismatch');
test('wrong_snapshot',o=>o.inputFingerprint='b'.repeat(64),false,'scope_mismatch');
test('author_as_user',o=>o.comments[1].responseType='reported_result',false,'comment_role_mismatch');
test('parent_as_target_evidence',o=>o.comments[0].evidence=[{sourceRef:'C2',quote:'建议多练习'}],false,'comment_evidence_not_target');
test('unknown_as_zero',o=>o.comments[0].measurement={kind:'unknown',value:0},false,'measurement_unknown_mismatch');
test('unknown_as_null',o=>o.comments[0].measurement={kind:'unknown',value:null},true);
test('missing_comment',o=>o.comments.pop(),false,'comment_coverage_mismatch');
test('duplicate_comment',o=>o.comments.push(structuredClone(o.comments[0])),false,'duplicate_comment');
test('tag_without_claim',o=>o.tags[0].claimIds=['missing'],false,'claim_reference_missing');
test('hypothesis_without_alternative',o=>o.claims[0].kind='hypothesis',false,'hypothesis_missing_alternative');
test('inspiration_without_condition',o=>o.claims[0].kind='inspiration',false,'inspiration_missing_conditions');
test('no_material_false_claim',o=>o.status='no_readable_material',false,'false_no_material');
test('unexpected_formal_topic_id',o=>o.topicRef='unapproved',false);
test('zero_is_known_zero',o=>o.comments[0].measurement={kind:'progress',value:0},true);
test('empty_comments_are_not_failure',(o,p)=>{o.comments=[];p.comments=[];},true);
test('genuinely_no_readable_input',(o,p)=>{p.sources=[];p.comments=[];o.comments=[];o.claims=[];o.tags=[];o.reading={overviewClaimIds:[],distinctiveClaimIds:[]};o.status='no_readable_material';},true);
// Deliberate limitation probes: structural validity cannot determine semantic entailment.
test('semantic_trap_progress_as_seconds',o=>{o.comments[0].measurement.kind='elapsed_seconds';o.comments[0].summary='18秒完成';},true);
test('semantic_trap_claim_promoted_to_fact',o=>{o.claims[0].kind='observation';o.claims[0].text='已经证明该方法有效';},true);
test('semantic_trap_irrelevant_tag',o=>o.tags[0].label='预约价格',true);
const receipt={kind:'offline_contract_checks',ajvVersion:require(path.join(path.resolve(ajvDir),'package.json')).version,providerCalls:0,results,
 semanticTraps:'最后三项结构通过是预期的能力边界，不是语义通过；须由单独盲评/来源判读拒绝。'};
fs.mkdirSync(path.dirname(outputFile),{recursive:true});fs.writeFileSync(outputFile,JSON.stringify(receipt,null,2)+'\n');
console.log(JSON.stringify({checks:results.length,matched:results.filter(x=>x.matchedExpectation).length,semanticTraps:3}));
