//! Holds every actual loopback request so concurrency and drain can be observed, not inferred.
use super::wire;
use std::sync::{Arc, Mutex};
use tokio::{
    net::TcpListener,
    sync::watch,
    task::{JoinHandle, JoinSet},
};
pub struct Endpoint {
    pub url: String,
    pub requests: Arc<Mutex<Vec<serde_json::Value>>>,
    release: watch::Sender<bool>,
    stop: watch::Sender<bool>,
    task: JoinHandle<()>,
}
impl Endpoint {
    pub async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/v1", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(vec![]));
        let recorded = requests.clone();
        let (release, gate) = watch::channel(false);
        let (stop, mut stopping) = watch::channel(false);
        let task = tokio::spawn(async move {
            let mut tasks = JoinSet::new();
            loop {
                tokio::select! {
                    _=stopping.changed()=>break,
                    socket=listener.accept()=>{
                        let (mut socket,_)=socket.unwrap();let mut gate=gate.clone();let recorded=recorded.clone();
                        tasks.spawn(async move {
                            let request=wire::read(&mut socket).await;
                            recorded.lock().unwrap().push(request);
                            while !*gate.borrow_and_update() {gate.changed().await.unwrap();}
                            wire::respond(&mut socket,r#"{"lingganProbe":"ok"}"#).await;
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
            release,
            stop,
            task,
        }
    }
    pub fn release(&self) {
        self.release.send_replace(true);
    }
    pub async fn finish(self) {
        self.release();
        self.stop.send_replace(true);
        self.task.await.unwrap();
    }
}
