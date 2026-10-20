//! Delay one successful BEGIN reply on an owned plaintext PG connection.
//! No SQL, packets or credentials are logged. Never accepts a shared DSN.
use std::{io, sync::Arc};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::Notify,
    task::{JoinHandle, JoinSet},
};

pub struct BeginProxy {
    pub url: String,
    pub observed: Arc<Notify>,
    pub release: Arc<Notify>,
    armed: Arc<std::sync::atomic::AtomicBool>,
    task: JoinHandle<()>,
}

impl BeginProxy {
    pub async fn start(owned_url: &str) -> Self {
        let (credentials, address) = owned_url.rsplit_once('@').expect("owned runner URL");
        let (upstream, database) = address.split_once('/').expect("owned runner database");
        assert!(upstream.starts_with("127.0.0.1:"));
        assert!(database.starts_with("linggan_intelligence_rd_") && !database.contains('?'));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!(
            "{credentials}@{}/{database}?sslmode=disable",
            listener.local_addr().unwrap()
        );
        let upstream = upstream.to_owned();
        let observed = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let signals = (observed.clone(), release.clone());
        let armed = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let first = armed.clone();
        let task = tokio::spawn(async move {
            let mut connections = JoinSet::new();
            loop {
                tokio::select! {
                    socket=listener.accept()=>{
                        let (client, _)=socket.unwrap();
                        let (observed, release)=signals.clone();
                        let server=upstream.clone();let first=first.clone();
                        connections.spawn(async move { let _=forward(client,&server,first,observed,release).await; });
                    }
                    result=connections.join_next(),if !connections.is_empty()=>{ result.unwrap().unwrap(); }
                }
            }
        });
        Self {
            url,
            observed,
            release,
            armed,
            task,
        }
    }

    pub fn arm(&self) {
        self.armed.store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

impl Drop for BeginProxy {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn forward(
    mut client: TcpStream,
    upstream: &str,
    first: Arc<std::sync::atomic::AtomicBool>,
    observed: Arc<Notify>,
    release: Arc<Notify>,
) -> io::Result<()> {
    client.set_nodelay(true)?;
    let mut server = TcpStream::connect(upstream).await?;
    server.set_nodelay(true)?;
    // The owned URL disables SSL. Forward the startup packet before parsing backend frames.
    let length = client.read_u32().await? as usize;
    if !(8..=16_384).contains(&length) {
        return Err(io::ErrorKind::InvalidData.into());
    }
    let mut startup = vec![0; length - 4];
    client.read_exact(&mut startup).await?;
    if startup[..4] != 196608_u32.to_be_bytes() {
        return Err(io::ErrorKind::InvalidData.into());
    }
    server.write_u32(length as u32).await?;
    server.write_all(&startup).await?;
    let (mut input, mut output) = client.into_split();
    let (mut replies, mut commands) = server.into_split();
    let frontend = tokio::io::copy(&mut input, &mut commands);
    let backend = async {
        loop {
            let tag = replies.read_u8().await?;
            let length = replies.read_u32().await? as usize;
            if !(4..=16_777_216).contains(&length) {
                return Err(io::ErrorKind::InvalidData.into());
            }
            let mut payload = vec![0; length - 4];
            replies.read_exact(&mut payload).await?;
            if tag == b'C'
                && payload == b"BEGIN\0"
                && first.swap(false, std::sync::atomic::Ordering::SeqCst)
            {
                observed.notify_one();
                release.notified().await;
            }
            output.write_u8(tag).await?;
            output.write_u32(length as u32).await?;
            output.write_all(&payload).await?;
        }
        #[allow(unreachable_code)]
        Ok::<(), io::Error>(())
    };
    tokio::select! { result=frontend=>{result?;}, result=backend=>{result?;} }
    Ok(())
}
