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
                            recorded.lock().unwrap().push(request);
                            let waiting=if probe {None} else {gate.lock().unwrap().take()};
                            if let Some((seen,release))=waiting {seen.send(()).unwrap();release.await.unwrap();}
                            let output=if probe {json!({"lingganProbe":"ok"})}else{json!({"titlePresence":"absent","titleText":null,"regions":[{"regionRef":"r1","text":"42","role":"body","legibility":"clear","bbox":null},{ "regionRef":"r2","text":"SYNTHETIC BRAND","role":"brand","legibility":"clear","bbox":null},{"regionRef":"r3","text":"SYNTHETIC ACTION","role":"call_to_action","legibility":"clear","bbox":null}],"unreadable":false})};
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
