//! Synthetic responses through the real SDK wire; not a model-quality or provider-capability proof.
#[path = "note_analysis_response.rs"]
mod note_response;
#[path = "ai_writing_endpoint.rs"]
mod writing_wire;

use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use tokio::{
    net::TcpListener,
    sync::watch,
    task::{JoinHandle, JoinSet},
};
pub fn text(request: &Value, role: &str) -> String {
    let message = request["messages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|message| message["role"] == role)
        .unwrap();
    let content = &message["content"];
    content
        .as_str()
        .or_else(|| content[0]["text"].as_str())
        .unwrap()
        .into()
}

pub struct Endpoint {
    pub url: String,
    pub requests: Arc<Mutex<Vec<Value>>>,
    stop: watch::Sender<bool>,
    task: JoinHandle<()>,
}
impl Endpoint {
    pub async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/v1", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let recorded = requests.clone();
        let (stop, mut stopping) = watch::channel(false);
        let task = tokio::spawn(async move {
            let mut clients = JoinSet::new();
            loop {
                tokio::select! {
                    _ = stopping.changed() => break,
                    socket = listener.accept() => {
                        let (mut socket, _) = socket.unwrap();
                        let recorded = recorded.clone();
                        clients.spawn(async move {
                            // Reuse the bounded reader, synthetic-credential check and SSE
                            // response (including its actual wire model: synthetic-writing).
                            let request = writing_wire::read(&mut socket).await;
                            let prompt = text(&request, "user");
                            let output = if prompt.contains("ai-probe.synthetic.v1") {
                                json!({"lingganProbe":"ok"})
                            } else {
                                let packet: Value = serde_json::from_str(&prompt).unwrap();
                                if let Some(question) = packet["question"].as_str() {
                                    assert!(!request.to_string().contains("image_url"));
                                    let evidence = packet["evidence"].as_array().unwrap();
                                    let quote = evidence.iter().find(|e| e["kind"] == "material_quote").unwrap();
                                    assert!(quote["quote"].as_str().unwrap().contains("先摆出材料"));
                                    assert!(quote.get("sourceRef").is_none());
                                    let observed = evidence.iter().find(|e| e["kind"] == "model_read").unwrap();
                                    assert!(observed["observation"].is_string());
                                    assert!(observed.get("blobSha256").is_none());
                                    assert_eq!(packet["core"]["knowledgeUnits"][2]["assertionKind"], "author_claim");
                                    let reference = if question == "合成无效引用" { json!("missing") } else { quote["id"].clone() };
                                    json!({"findings":[{
                                        "id":"f1","findingType":"structure","statement":"作者用材料示范核对步骤。",
                                        "basisUnitIds":["u3"],"questionFocus":question,
                                        "support":[{"evidenceRef":reference,"quote":"先摆出材料，再按步骤核对"}],
                                        "presentationOrder":[]
                                    }]})
                                } else if packet["material"]["texts"].is_array() {
                                    let images = packet["material"]["images"].as_array().unwrap();
                                    if !images.is_empty() {
                                        let content = &request["messages"].as_array().unwrap().iter()
                                            .find(|message| message["role"] == "user").unwrap()["content"];
                                        assert_eq!(content.as_array().unwrap().len(), 1 + images.len() * 2);
                                        for (index, image) in images.iter().enumerate() {
                                            assert!(content[1 + index * 2]["text"].as_str().unwrap().starts_with(
                                                &format!("inputCitationId={};", image["citationId"].as_str().unwrap())
                                            ));
                                            assert_eq!(content[2 + index * 2]["type"], "image_url");
                                        }
                                    }
                                    draft(&packet)
                                } else {
                                    assert!(packet["material"]["sources"].is_array());
                                    note_response::response(&packet)
                                }
                            };
                            recorded.lock().unwrap().push(request);
                            writing_wire::respond(&mut socket, &output.to_string()).await;
                        });
                    }
                }
            }
            while let Some(result) = clients.join_next().await {
                result.unwrap();
            }
        });
        Self {
            url,
            requests,
            stop,
            task,
        }
    }
    pub async fn finish(self) {
        self.stop.send_replace(true);
        self.task.await.unwrap();
    }
}

pub fn draft(packet: &Value) -> Value {
    let source = packet["material"]["texts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["kind"] == "body")
        .unwrap();
    let quote = source["text"].as_str().unwrap();
    assert!(!quote.is_empty() && quote.chars().count() <= 500);
    let images = packet["material"]["images"].as_array().unwrap();
    let vision: Vec<_> = images.iter().enumerate().map(|(i, image)| json!({
        "id": format!("v{}", i + 1), "inputCitationId": image["citationId"],
        "modelQuote": null, "observation": "合成单像素图片，无额外可读内容。", "speakerRole": "unknown"
    })).collect();
    let contributions: Vec<_> = images.iter().enumerate().map(|(i, image)| json!({
        "inputCitationId": image["citationId"], "status": "no_new_information",
        "summary": "合成空白图片没有新增信息。", "unitIds": [], "evidenceRefs": [format!("v{}", i + 1)]
    })).collect();
    json!({
        "core": {
            "summary": {"text":"合成材料说明步骤，仅验证数据链路。","unitIds":["u2","u3"]},
            "contentType":"practical_guide", "mainTopicUnitId":"u2",
            "knowledgeUnits":[
                {"id":"u1","unitType":"discussion","statement":"合成流程的背景。","assertionKind":"observation","speakerRole":"author","evidenceRefs":["t1"]},
                {"id":"u2","unitType":"discussion","statement":"合成材料的主要步骤。","assertionKind":"interpretation","speakerRole":"unknown","evidenceRefs":["t1"]},
                {"id":"u3","unitType":"provision","statement":"先摆出材料，再按步骤核对。","assertionKind":"author_claim","speakerRole":"author","evidenceRefs":["t1"]}
            ],
            "candidateLabels":[{"dimension":"offered_value","label":"合成流程","retrievalUse":"只检索合成流程测试。","unitIds":["u3"]}]
        },
        "imageContributions":contributions,
        "evidenceCandidates":{
            "text":[
                {"id":"t1","inputCitationId":source["citationId"],"quote":quote},
                {"id":"unusedText","inputCitationId":source["citationId"],"quote":format!("{quote}”")}
            ],
            "vision":vision
        }
    })
}
