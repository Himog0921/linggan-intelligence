//! Owned loopback SSE endpoint. It never forwards traffic or reads credentials/files.
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};
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
        Self::start_inner(gated, None, None).await
    }
    /// Blocks precisely the second image response until the owned browser proof writes its
    /// release token.  It never opens a network path beyond the loopback mock.
    pub async fn start_browser_second_gate(release_path: PathBuf) -> Self {
        Self::start_inner(false, Some(release_path), None).await
    }
    /// A valid transport with an invalid visual contract simulates a terminal second-image
    /// failure while retaining the exact raw receipt for inspection.
    pub async fn start_second_image_schema_failure() -> Self {
        Self::start_inner(false, None, Some(2)).await
    }
    async fn start_inner(
        gated: bool,
        second_gate_path: Option<PathBuf>,
        schema_failure_at: Option<usize>,
    ) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/v1", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(vec![]));
        let recorded = requests.clone();
        let (stop, mut stopping) = watch::channel(false);
        let (seen_tx, seen) = oneshot::channel();
        let (release, release_rx) = oneshot::channel();
        let gate = Arc::new(Mutex::new(gated.then_some((seen_tx, release_rx))));
        let visual_requests = Arc::new(AtomicUsize::new(0));
        let task = tokio::spawn(async move {
            let mut tasks = JoinSet::new();
            loop {
                tokio::select! {
                    _=stopping.changed()=>break,
                    socket=listener.accept()=>{
                        let(mut socket,_)=socket.unwrap();let recorded=recorded.clone();let gate=gate.clone();let visual_requests=visual_requests.clone();let second_gate_path=second_gate_path.clone();
                        tasks.spawn(async move {
                            let request=read(&mut socket).await;let prompt=text(&request,"user");
                            let probe=prompt.contains("ai-probe.synthetic.v1");
                            recorded.lock().unwrap().push(request);
                            let waiting=if probe {None} else {gate.lock().unwrap().take()};
                            if let Some((seen,release))=waiting {seen.send(()).unwrap();release.await.unwrap();}
                            let image_number=(!probe).then(||visual_requests.fetch_add(1,Ordering::SeqCst)+1);
                            if image_number==Some(2) {
                                if let Some(path)=second_gate_path {
                                    while !path.exists() { tokio::time::sleep(std::time::Duration::from_millis(20)).await; }
                                }
                            }
                            let output=if schema_failure_at.is_some() && image_number==schema_failure_at {json!({"malformed":"synthetic second-image schema failure"})}else if probe {json!({"lingganProbe":"ok"})}else if prompt.contains("\"imageRole\":\"explicit_cover\"") {
                                json!({"titlePresence":"present","titleText":"合成封面标题","regions":[
                                    {"regionRef":"cover-title","text":"合成封面标题","role":"main_title","legibility":"clear","bbox":[0.1,0.1,0.9,0.25]},
                                    {"regionRef":"cover-body","text":"封面正文","role":"body","legibility":"clear","bbox":[0.1,0.3,0.9,0.5]},
                                    {"regionRef":"cover-brand","text":"SYNTHETIC BRAND","role":"brand","legibility":"clear","bbox":null}],"unreadable":false})
                            } else if prompt.contains("\"imageRole\":\"interior\"") {
                                json!({"titlePresence":"absent","titleText":null,"regions":[
                                    {"regionRef":"interior-heading","text":"内页章节标题","role":"chapter_heading","legibility":"clear","bbox":[0.1,0.1,0.9,0.25]},
                                    {"regionRef":"interior-body","text":"内页正文","role":"body","legibility":"clear","bbox":[0.1,0.3,0.9,0.5]},
                                    {"regionRef":"interior-partial","text":"模糊字","role":"body","legibility":"partial","bbox":null}],"unreadable":false})
                            } else {json!({"titlePresence":"absent","titleText":null,"regions":[{"regionRef":"r1","text":"42","role":"body","legibility":"clear","bbox":null},{ "regionRef":"r2","text":"SYNTHETIC BRAND","role":"brand","legibility":"clear","bbox":null},{"regionRef":"r3","text":"SYNTHETIC ACTION","role":"call_to_action","legibility":"clear","bbox":null}],"unreadable":false})};
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
