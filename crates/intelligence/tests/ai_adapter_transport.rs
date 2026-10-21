//! Actual Rust -> fixed Node child -> pinned pi SDK -> owned loopback HTTP proof.
//! No shared DB, real credential, model, service or external host is used.
use linggan_intelligence::{
    ai_foundation::{capability::Capability, provider_store::OutputMode, transport::PiExecution},
    model_settings::ModelError,
    pi_adapter::{PI_PROTOCOL, PiAdapter, PiRequest},
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::PathBuf};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
use uuid::Uuid;
fn request(base_url: String) -> PiRequest {
    PiRequest {
        images: vec![],
        version: PI_PROTOCOL,
        operation: "analyze".into(),
        api: "openai-completions".into(),
        base_url,
        local_endpoint: true,
        api_key: "SYNTHETIC-NOT-A-CREDENTIAL".into(),
        model_id: "unknown-synthetic-model".into(),
        timeout_ms: 3_600_000,
        max_output_tokens: 384_000,
        system: "Synthetic / not evidence".into(),
        prompt: "材".repeat(100_000),
        execution: Some(PiExecution {
            capability_ref: Uuid::new_v4(),
            limits: Capability {
                context_tokens: 1_000_000,
                max_output_tokens: 384_000,
                max_timeout_seconds: 3600,
                thinking_levels: ["off", "high", "max"].map(str::to_owned).into(),
            },
            input_token_limit: 600_000,
            thinking_level: "off".into(),
            compat: json!({"maxTokensField":"max_completion_tokens","thinkingFormat":"deepseek","supportsReasoningEffort":true}),
            thinking_level_map: BTreeMap::from([("off".into(), Some("none".into()))]),
            output_mode: OutputMode::PromptOnly,
            strict_schema: false,
            response_schema: None,
        }),
    }
}
async fn read_request(socket: &mut tokio::net::TcpStream) -> Value {
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 8192];
    let header_end = loop {
        let n = socket.read(&mut chunk).await.unwrap();
        assert!(n > 0, "owned HTTP connection closed before request");
        bytes.extend_from_slice(&chunk[..n]);
        if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
            break end + 4;
        }
        assert!(bytes.len() < 16384, "request header envelope is bounded");
    };
    let header = std::str::from_utf8(&bytes[..header_end]).unwrap();
    assert!(header.starts_with("POST /v1/chat/completions "));
    let headers: BTreeMap<_, _> = header
        .lines()
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.to_ascii_lowercase(), v.trim().to_owned()))
        .collect();
    assert!(
        headers
            .get("authorization")
            .is_some_and(|v| v == "Bearer SYNTHETIC-NOT-A-CREDENTIAL"),
        "only the supplied synthetic credential may be transmitted"
    );
    let length: usize = headers["content-length"].parse().unwrap();
    assert!(length < linggan_contracts::ai_limits::AI_LIMITS.max_request_bytes as usize);
    while bytes.len() < header_end + length {
        let n = socket.read(&mut chunk).await.unwrap();
        assert!(n > 0);
        bytes.extend_from_slice(&chunk[..n]);
    }
    serde_json::from_slice(&bytes[header_end..header_end + length]).unwrap()
}
#[tokio::test]
#[ignore = "fixed Node SDK and owned loopback only; run by the AI foundation proof"]
async fn rust_and_real_node_preserve_large_frozen_resource_bounds_and_wire_usage() {
    let node =
        std::env::var_os("AI_FOUNDATION_PROOF_NODE").expect("explicit proof Node executable");
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let url = format!(
        "http://127.0.0.1:{}/v1",
        listener.local_addr().unwrap().port()
    );
    let output = "材".repeat(100_000);
    let returned = output.clone();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let request = read_request(&mut socket).await;
        let chunk = |delta: Value, finish: Value| json!({"id":"synthetic","object":"chat.completion.chunk","choices":[{"index":0,"delta":delta,"finish_reason":finish}]});
        let body = format!(
            "data: {}\n\ndata: {}\n\ndata: {}\n\ndata: [DONE]\n\n",
            chunk(json!({"role":"assistant","content":returned}), Value::Null),
            chunk(json!({}), json!("stop")),
            json!({"choices":[],"usage":{"prompt_tokens":100,"completion_tokens":40,"prompt_tokens_details":{"cached_tokens":30,"cache_write_tokens":0},"completion_tokens_details":{"reasoning_tokens":0}}})
        );
        let head = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        socket.write_all(head.as_bytes()).await.unwrap();
        socket.write_all(body.as_bytes()).await.unwrap();
        socket.shutdown().await.unwrap();
        request
    });
    let adapter = PiAdapter::configured_with_test_command(
        PathBuf::from(node),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../apps/pi-adapter/src/adapter.mjs"),
    );
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(15),
        adapter.call(&request(url)),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(result.ok, "fixed local transport must finish");
    assert!(
        result.text.as_deref() == Some(output.as_str()),
        "the complete final text must survive the child boundary"
    );
    assert!(
        result.text.as_ref().unwrap().len() > 262144,
        "the Rust child receipt crosses the historical output cap"
    );
    assert_eq!(result.usage.input_tokens, Some(100));
    assert_eq!(result.usage.output_tokens, Some(40));
    assert_eq!(result.usage.cached_input_tokens, Some(30));
    assert_eq!(result.usage.cached_write_tokens, Some(0));
    assert_eq!(result.usage.reasoning_tokens, Some(0));
    assert_eq!(result.usage.cost_usd, None);
    let sent = server.await.unwrap();
    assert_eq!(sent["max_completion_tokens"], 384_000);
    assert!(sent.get("max_tokens").is_none());
    assert_eq!(sent["thinking"], json!({"type":"disabled"}));
    assert!(
        sent.get("tools")
            .is_none_or(|v| v.as_array().is_some_and(Vec::is_empty))
    );
    assert!(sent.to_string().len() > 131072);
}
#[tokio::test]
#[ignore = "no child or network; included in the isolated adapter proof"]
async fn inconsistent_or_unqualified_resources_are_rejected_before_starting_a_child() {
    let adapter = PiAdapter::configured_with_test_command(
        PathBuf::from("/nonexistent-proof-node"),
        PathBuf::from("/nonexistent-proof-adapter"),
    );
    let mut r = request("http://127.0.0.1:9/v1".into());
    r.execution = None;
    assert!(matches!(adapter.call(&r).await, Err(ModelError::Invalid)));
    r = request("http://127.0.0.1:9/v1".into());
    r.execution.as_mut().unwrap().input_token_limit = 900_000;
    assert!(matches!(adapter.call(&r).await, Err(ModelError::Invalid)));
    r = request("http://127.0.0.1:9/v1".into());
    r.execution.as_mut().unwrap().input_token_limit = 1024;
    assert!(matches!(
        adapter.call(&r).await,
        Err(ModelError::InputLimit)
    ));
}
