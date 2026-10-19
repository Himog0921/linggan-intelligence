//! One owned plaintext PG wire fault: let COMMIT succeed, discard its acknowledgement.
//! No packet/SQL/credential content is logged or persisted; never accepts a shared DSN.
use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task::{JoinHandle, JoinSet},
};

pub struct CommitProxy {
    pub url: String,
    pub dropped: Arc<AtomicBool>,
    task: JoinHandle<()>,
}
impl CommitProxy {
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
        let dropped = Arc::new(AtomicBool::new(false));
        let once = dropped.clone();
        let task = tokio::spawn(async move {
            let mut connections = JoinSet::new();
            loop {
                tokio::select! {
                    socket=listener.accept()=>{
                        let (client, _)=socket.unwrap();let server=upstream.clone();let dropped=once.clone();
                        connections.spawn(async move { let _=forward(client,&server,dropped).await; });
                    }
                    result=connections.join_next(),if !connections.is_empty()=>{ result.unwrap().unwrap(); }
                }
            }
        });
        Self { url, dropped, task }
    }
}
impl Drop for CommitProxy {
    fn drop(&mut self) {
        self.task.abort();
    }
}
async fn frame(reader: &mut (impl AsyncRead + Unpin)) -> io::Result<(u8, Vec<u8>)> {
    let tag = reader.read_u8().await?;
    let length = reader.read_u32().await? as usize;
    if !(4..=16_777_216).contains(&length) {
        return Err(io::ErrorKind::InvalidData.into());
    }
    let mut payload = vec![0; length - 4];
    reader.read_exact(&mut payload).await?;
    Ok((tag, payload))
}
async fn forward(
    mut client: TcpStream,
    upstream: &str,
    dropped: Arc<AtomicBool>,
) -> io::Result<()> {
    let mut server = TcpStream::connect(upstream).await?;
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
    let pending = AtomicBool::new(false);
    let frontend = async {
        let mut receipt = false;
        loop {
            let (tag, payload) = frame(&mut input).await?;
            if tag == b'B'
                && payload
                    .windows(b"executorReceipt".len())
                    .any(|w| w == b"executorReceipt")
            {
                receipt = true;
            }
            if tag == b'Q' && payload == b"COMMIT\0" && receipt {
                pending.store(true, Ordering::SeqCst);
                receipt = false;
            }
            if tag == b'Q' && payload == b"ROLLBACK\0" {
                receipt = false;
            }
            commands.write_u8(tag).await?;
            commands.write_u32(payload.len() as u32 + 4).await?;
            commands.write_all(&payload).await?;
        }
        #[allow(unreachable_code)]
        Ok::<(), io::Error>(())
    };
    let backend = async {
        loop {
            let (tag, payload) = frame(&mut replies).await?;
            if tag == b'C'
                && payload == b"COMMIT\0"
                && pending.swap(false, Ordering::SeqCst)
                && dropped
                    .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                    .is_ok()
            {
                return Err(io::ErrorKind::ConnectionAborted.into());
            }
            output.write_u8(tag).await?;
            output.write_u32(payload.len() as u32 + 4).await?;
            output.write_all(&payload).await?;
        }
        #[allow(unreachable_code)]
        Ok::<(), io::Error>(())
    };
    tokio::select! { result=frontend=>result,result=backend=>result }
}
