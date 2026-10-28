//! Owned loopback SSE endpoint. It never forwards traffic or reads credentials/files.
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{oneshot, watch},
    task::{JoinHandle, JoinSet},
};
pub struct Endpoint {
    pub url: String,
    pub requests: Arc<Mutex<Vec<Value>>>,
    stop: watch::Sender<bool>,
    task: JoinHandle<()>,
    pub seen: Option<oneshot::Receiver<()>>,
    pub release: Option<oneshot::Sender<()>>,
}
impl Endpoint {
    pub async fn start(gated: bool) -> Self {
        Self::variant(gated, "normal").await
    }
    pub async fn variant(gated: bool, variant: &'static str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/v1", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(vec![]));
        let recorded = requests.clone();
        let (stop, mut stopping) = watch::channel(false);
        let (seen_tx, seen) = oneshot::channel();
        let (release, release_rx) = oneshot::channel();
        let gate = Arc::new(Mutex::new(gated.then_some((seen_tx, release_rx))));
        let task = tokio::spawn(async move {
            let mut tasks = JoinSet::new();
            loop {
                tokio::select! {
                    _=stopping.changed()=>break,
                    socket=listener.accept()=>{
                        let(mut socket,_)=socket.unwrap();let recorded=recorded.clone();let gate=gate.clone();
                        tasks.spawn(async move {
                            let request=read(&mut socket).await;let prompt=text(&request,"user");
                            let probe=prompt.contains("ai-probe.synthetic.v1");
                            let system=text(&request,"system");
                            let instruction=serde_json::from_str::<Value>(&prompt).ok().and_then(|v|v["authorInstruction"].as_str().map(str::to_owned)).unwrap_or_default();
                            recorded.lock().unwrap().push(request);
                            let waiting=if probe {None} else {gate.lock().unwrap().take()};
                            if let Some((seen,release))=waiting {seen.send(()).unwrap();release.await.unwrap();}
                            let mut output=if probe {json!({"lingganProbe":"ok"})} else if serde_json::from_str::<Value>(&prompt).ok().is_some_and(|v|v["documents"].is_array()) {
                                let packet:Value=serde_json::from_str(&prompt).unwrap();
                                let citation=|index:usize| {let entries=packet["documents"][index]["fragments"].as_array().unwrap();entries.iter().find(|f|f["content"]["kind"]=="table").unwrap_or(&entries[0])["id"].clone()};
                                json!({"relation":"mostly_repeated","claims":[{"kind":"shared_point","statement":"两侧合成材料均有步骤说明。","targetCitations":[citation(0)],"referenceCitations":[citation(1)]}],"limitations":["synthetic loopback fixture only"]})
                            } else if serde_json::from_str::<Value>(&prompt).ok().is_some_and(|v|v["input"]["accountResearch"]==true) {
                                let field=json!({"value":"unknown","reason":"synthetic limited sample","evidenceFragmentIds":[]});
                                json!({"focus":field,"institution_or_brand":field})
                            } else if serde_json::from_str::<Value>(&prompt).ok().is_some_and(|v|v["input"]["materialContract"]=="research-dossier.v2") {
                                let packet:Value=serde_json::from_str(&prompt).unwrap();
                                let citation=packet["input"]["materials"][0]["citation"].clone();
                                let field=|value|json!({"value":value,"reason":"synthetic source","evidenceFragmentIds":[citation]});
                                json!({"relevance":field("related"),"traits":{"personal_experience":field("unknown"),"professional_output":field("unknown"),"explicit_promotion":field("unknown")},"topicHints":[]})
                            } else if serde_json::from_str::<Value>(&prompt).ok().is_some_and(|v|v["target"]["researchText"].is_string()) {
                                json!({"contract":"comment-study.semantic.v1","outcome":"signals","reason":null,"signals":[{"kind":"need","proposition":"合成测试需要提醒","evidence":"我的孩子需要每天有人提醒","problemFrame":{"actor":{"value":"孩子","basis":"我的孩子"},"goalOrExpectedState":{"value":"获得每日提醒","basis":"每天有人提醒"},"barrierOrUnmetNeed":{"value":"依赖外部提醒","basis":"需要每天有人提醒"},"context":{"value":null,"basis":null}}}]})
                            } else if let Some(output)=topic_response(&prompt) { output
                            } else if instruction=="invalid-output" {json!({"outcome":"no_signal"})} else {
                                json!({"title":null,"paragraphs":[{"paragraphId":null,"text":if system.contains("candidate-v2") {"合成候选建议"} else if system.contains("baseline-v1") {"合成基线建议"} else {"合成写作建议"}}],"pages":[]})
                            };
                            if !probe && matches!(variant,"real_material"|"research_ui") {
                                let root:Value=serde_json::from_str(&prompt).unwrap();
                                if let Some(target)=root["target"]["researchText"].as_str().filter(|_|variant=="real_material"){output=json!({"contract":"comment-study.semantic.v1","outcome":"signals","reason":null,"signals":[{"kind":"experience","proposition":"SYNTHETIC 流程测试：原声引用可还原，不代表业务判断","evidence":target,"problemFrame":null}]});}
                                if root["packet"]["contract"]=="topic-map.extract.quotes.v1" {
                                    let fragments=root["packet"]["input"]["fragments"].as_array().unwrap();
                                    let index=fragments.iter().position(|f|matches!(f["field"].as_str(),Some("body"|"ocr"|"transcript"|"frame_text"|"studied_comment"|"unresearched_comment"))).unwrap();
                                    let full=linggan_evidence::research_dossier::content::block_text(&fragments[index]["content"]);let quote:String=full.chars().skip(full.chars().count().saturating_sub(1000)).collect();
                                    output["d"][0]["e"]=json!([{"f":index,"q":quote}]);output["d"][0]["sp"]=json!(if matches!(fragments[index]["field"].as_str(),Some("studied_comment"|"unresearched_comment")){"commenter"}else{match fragments[index]["speaker"].as_str(){Some("author")=>"author",Some("other")=>"quoted",_=>"unknown"}});output["d"][0]["s"]=json!("SYNTHETIC 流程测试：当前来源可精确引用");
                                }
                            }
                            if !probe {
                                let root:Value=serde_json::from_str(&prompt).unwrap();let packet=&root["packet"];
                                if variant=="no_signal" && packet["contract"]=="topic-map.extract.quotes.v1" {output["o"]=json!("no_signal");output["d"]=json!([]);}
                                if (variant=="reject_extraction" && packet["input"]["phase"]=="extraction") || (variant=="reject_resolution" && packet["input"]["phase"]=="resolution") {output["checks"][0]["verdict"]=json!("fail");}
                            }
                            respond(&mut socket,&output.to_string()).await;
                        });
                    }
                }
            }
            while let Some(result) = tasks.join_next().await {
                result.unwrap();
            }
        });
        Self {
            url,
            requests,
            stop,
            task,
            seen: Some(seen),
            release: Some(release),
        }
    }
    pub async fn finish(self) {
        self.stop.send_replace(true);
        self.task.await.unwrap();
    }
}
pub fn text(request: &Value, role: &str) -> String {
    let Some(message) = request["messages"]
        .as_array()
        .and_then(|messages| messages.iter().find(|m| m["role"] == role))
    else {
        return String::new();
    };
    if let Some(text) = message["content"].as_str() {
        return text.into();
    }
    message["content"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|part| part["text"].as_str())
        .collect()
}
pub(crate) async fn read(socket: &mut TcpStream) -> Value {
    let mut bytes = vec![];
    let mut chunk = [0; 8192];
    let end = loop {
        let n = socket.read(&mut chunk).await.unwrap();
        assert!(n > 0);
        bytes.extend_from_slice(&chunk[..n]);
        if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
            break end + 4;
        }
        assert!(bytes.len() < 16384);
    };
    let head = std::str::from_utf8(&bytes[..end]).unwrap();
    assert!(head.starts_with("POST /v1/chat/completions "));
    assert!(
        head.lines()
            .any(|line| line
                .eq_ignore_ascii_case("authorization: Bearer SYNTHETIC-NOT-A-CREDENTIAL")),
        "only a supplied synthetic credential may be transmitted"
    );
    let length = head
        .lines()
        .find_map(|line| {
            let (k, v) = line.split_once(':')?;
            k.eq_ignore_ascii_case("content-length")
                .then(|| v.trim().parse::<usize>().unwrap())
        })
        .unwrap();
    assert!(length < 1_048_576, "bounded synthetic fixture payload");
    while bytes.len() < end + length {
        let n = socket.read(&mut chunk).await.unwrap();
        assert!(n > 0);
        bytes.extend_from_slice(&chunk[..n]);
    }
    let request: Value = serde_json::from_slice(&bytes[end..end + length]).unwrap();
    assert!(
        request
            .get("tools")
            .is_none_or(|v| v.as_array().is_some_and(Vec::is_empty))
    );
    request
}
pub(crate) async fn respond(socket: &mut TcpStream, text: &str) {
    let body = format!(
        "data: {}\n\ndata: [DONE]\n\n",
        json!({"id":"synthetic-writing","object":"chat.completion.chunk","created":0,"model":"synthetic-writing","choices":[{"index":0,"delta":{"role":"assistant","content":text},"finish_reason":"stop"}],"usage":{"prompt_tokens":17,"completion_tokens":9,"prompt_tokens_details":{"cached_tokens":0},"completion_tokens_details":{"reasoning_tokens":0}}})
    );
    let head = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    socket.write_all(head.as_bytes()).await.unwrap();
    socket.write_all(body.as_bytes()).await.unwrap();
    socket.shutdown().await.unwrap();
}

fn topic_response(prompt: &str) -> Option<Value> {
    let root: Value = serde_json::from_str(prompt).ok()?;
    let packet = &root["packet"];
    match packet["contract"].as_str()? {
        "comment-problem-resolution.v1" => Some(
            json!({"contract":"comment-problem-resolution.v1","decisions":packet["signals"].as_array()?.iter().map(|signal|{let matched=packet["candidates"].as_array().and_then(|c|c.first());json!({"signalOrdinal":signal["signalOrdinal"],"status":if matched.is_some(){"matched"}else{"new"},"definitionRef":matched.map(|c|c["definitionRef"].clone()),"problem":if matched.is_some(){Value::Null}else{json!({"label":"每日提醒需求","actor":"孩子","goal":"获得每日提醒","barrier":"依赖外部提醒","context":null})},"reason":"合成目标原声支持"})}).collect::<Vec<_>>()}),
        ),
        "comment-problem-review.v1" => Some(
            json!({"contract":"comment-problem-review.v1","checks":packet["proposal"]["decisions"].as_array()?.iter().map(|d|json!({"signalOrdinal":d["signalOrdinal"],"verdict":"pass","reason":"synthetic pairwise review"})).collect::<Vec<_>>()}),
        ),
        "topic-map.extract.quotes.v1" => {
            let fragments = packet["input"]["fragments"].as_array()?;
            let index = fragments
                .iter()
                .position(|f| f["field"] == "body")
                .unwrap_or(0);
            Some(
                json!({"contract":"topic-map.extract.quotes.v1","o":"analyzed","d":[{"l":"保留正文末尾","t":null,"e":[{"f":index,"q":"结尾不能丢失🙂"}],"s":"结尾不能丢失。","d":"完整保留正文末尾。","i":["正文末尾保留"],"x":["新增来源材料"],"sp":"author","er":"support","r":"合成原句支持"}],"s":[],"j":{"m":"unclear","s":[],"o":[],"p":"unknown","r":"无阶段依据","e":[]},"rm":[],"a":[],"p":[],"l":["SYNTHETIC ONLY"]}),
            )
        }
        "topic-map.verify.v1" => Some(
            json!({"contract":"topic-map.verify.v1","checks":packet["input"]["checks"].as_array()?.iter().map(|c|json!({"checkId":c["checkId"],"verdict":"pass","reason":"synthetic validator fixture"})).collect::<Vec<_>>()}),
        ),
        "topic-map.resolve.v2" => {
            let definitions = packet["input"]["definitions"].as_array()?;
            let existing = definitions
                .iter()
                .find(|d| d["definition"] == "保留正文中明确存在的末尾内容。");
            Some(
                json!({"contract":"topic-map.resolve.v2","decisions":packet["input"]["units"].as_array()?.iter().map(|u|json!({"unitId":u["unitId"],"status":if existing.is_some(){"matched"}else{"new"},"matches":existing.map(|d|vec![json!({"topicRef":d["topicRef"],"definitionRef":d["definitionRef"],"reason":"SYNTHETIC exact fixture boundary"})]).unwrap_or_default(),"proposedTopic":if existing.is_some(){Value::Null}else{json!({"label":"正文完整保留","definition":"保留正文中明确存在的末尾内容。","inclusionCriteria":["讨论正文末尾保留"],"exclusionCriteria":["新增来源材料"],"domainFit":"in_scope","domainReason":"合成领域材料","abstractionReason":"合成保留规则","parent":{"kind":"root","topicRef":null,"definitionRef":null,"proposal":null,"reason":"合成根主题"}})},"relations":[],"reason":"合成明确边界"})).collect::<Vec<_>>()}),
            )
        }
        _ => None,
    }
}
