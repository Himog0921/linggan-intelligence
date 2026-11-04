//! Deterministic transport fixture, not an analysis model or a quality oracle.
use serde_json::{Value, json};
pub fn response(packet: &Value) -> Value {
    let sources = packet["material"]["sources"].as_array().unwrap();
    let material = sources
        .iter()
        .find(|s| s["kind"] == "material" && s["text"].as_str().is_some_and(|s| !s.is_empty()))
        .unwrap();
    let quote = material["text"].as_str().unwrap();
    let statement = |id: &str, text: &str| json!({"id":id,"text":text,"kind":"observation","evidenceRefs":["e1"]});
    let comments: Vec<_> = sources.iter().filter(|s| s["kind"] == "comment").collect();
    let mut evidence =
        vec![json!({"id":"e1","inputCitationId":material["citationId"],"quote":quote})];
    let feedback = if let Some(c) = comments.first() {
        evidence.push(json!({"id":"e2","inputCitationId":c["citationId"],"quote":c["text"]}));
        vec![
            json!({"id":"r1","text":"SYNTHETIC：已采评论原声可定位，未判断效果。","kind":"observation","evidenceRefs":["e2"]}),
        ]
    } else {
        vec![]
    };
    json!({"contract":"note-analysis.result.v1","summary":"SYNTHETIC 流程回执：验证当前笔记的两层结果与依据可保存读取，不代表模型分析质量。","summaryBasisIds":["u1","u4"],"understanding":{"discussion":statement("u1","SYNTHETIC：记录当前作品讨论。"),"audience":null,"situation":null,"answer":statement("u4","SYNTHETIC：原文回答已保留。"),"offeredValue":null},"features":[{"id":"f1","text":"SYNTHETIC：保留一项原文特征供流程验证。","kind":"observation","evidenceRefs":["e1"],"label":"原文特征","category":"expression"}],"feedback":feedback,"explanations":[{"id":"x1","text":"SYNTHETIC：解释字段与材料建立引用关系。","kind":"interpretation","featureIds":["f1"],"feedbackIds":[],"evidenceRefs":["e1"],"support":"仅验证数据链路。","uncertainty":"尚未由真实模型分析。"}],"tags":[{"label":"流程样本","dimension":"expression","judgmentIds":["f1"],"retrievalUse":"识别模拟输出"}],"methodCandidates":[{"id":"m1","text":"SYNTHETIC 方法候选。","kind":"reuse_idea","basisIds":["x1"],"conditions":"仅用于测试。","openQuestion":null}],"limitations":["受控模拟供应商；不得视为质量验收。"],"evidence":evidence})
}
