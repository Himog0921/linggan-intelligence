//! A file acknowledgement is evidence of a successful database transition, never its replacement.
use std::{
    fs,
    io::{self, Write},
    os::unix::fs::OpenOptionsExt,
    path::PathBuf,
};

pub fn write(value: &serde_json::Value) -> io::Result<()> {
    let path = std::env::var_os("LINGGAN_AI_DRAIN_ACK_PATH")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("LINGGAN_SUPPORT_DIR")
                .map(|root| PathBuf::from(root).join("runtime-drain/ai-drain-ack"))
        })
        .ok_or_else(|| io::Error::other("ai_drain_ack_not_configured"))?;
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("ai_drain_ack_invalid"))?;
    fs::create_dir_all(parent)?;
    let temp = parent.join(format!(
        ".ai-drain-{}-{}.tmp",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temp)?;
        serde_json::to_writer(&mut file, value)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        fs::rename(&temp, &path)?;
        fs::File::open(parent)?.sync_all()
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}
