//! Offline JSONL: one ImageInput per line -> one Assessment per line. No file path,
//! database, model or network inputs; callers own private source/output retention.
use linggan_evidence::ocr_adoption::{ImageInput, assess};
use std::io::{self, BufRead, Write};

fn main() {
    if run().is_err() {
        // Never print untrusted JSON or OCR text in a parse/validation error.
        eprintln!("ocr_adoption_replay_failed");
        std::process::exit(2);
    }
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let stdout = io::stdout();
    let mut writer = io::BufWriter::new(stdout.lock());
    loop {
        let mut bytes = vec![];
        // Enforce a byte cap before JSON allocation. A malformed record terminates the
        // stream nonzero; already emitted image receipts are not a whole-batch success.
        let mut bounded = (&mut reader).take(8_000_001);
        use std::io::Read;
        let read = bounded.read_until(b'\n', &mut bytes)?;
        if read == 0 {
            break;
        }
        if read > 8_000_000 {
            return Err("record_too_large".into());
        }
        let input: ImageInput = serde_json::from_slice(&bytes)?;
        let result = assess(&input)?;
        serde_json::to_writer(&mut writer, &result)?;
        writer.write_all(b"\n")?;
        writer.flush()?;
    }
    Ok(())
}
