//! Offline validation of an explicitly selected private trial result; no network or database.
use linggan_intelligence::note_analysis::{Packet, accept, response_schema};
use std::{env, fs, path::PathBuf};

fn main() {
    let args: Vec<_> = env::args().collect();
    assert_eq!(
        args.len(),
        3,
        "usage: note_analysis_trial_check ROOT SAMPLE"
    );
    let root = PathBuf::from(&args[1]);
    fs::write(
        root.join("response-schema.json"),
        serde_json::to_vec_pretty(&response_schema()).unwrap(),
    )
    .unwrap();
    let sample = &args[2];
    assert!(["N02", "N03", "N05", "N08"].contains(&sample.as_str()));
    let packet: Packet =
        serde_json::from_slice(&fs::read(root.join(format!("{sample}.packet.json"))).unwrap())
            .unwrap();
    let value =
        serde_json::from_slice(&fs::read(root.join(format!("{sample}.result.json"))).unwrap())
            .unwrap();
    let verdict = match accept(value, &packet) {
        Ok(output) => {
            fs::write(
                root.join(format!("{sample}.accepted.json")),
                serde_json::to_vec_pretty(&output).unwrap(),
            )
            .unwrap();
            serde_json::json!({"sample":sample,"accepted":true,"evidenceCount":output.evidence.len(),"tagCount":output.tags.len(),"semanticQualityVerified":false})
        }
        Err(error) => serde_json::json!({"sample":sample,"accepted":false,"reason":error}),
    };
    fs::write(
        root.join(format!("{sample}.validation.json")),
        serde_json::to_vec_pretty(&verdict).unwrap(),
    )
    .unwrap();
    println!("{verdict}");
}
