#!/bin/sh
# Synthetic, local-only Pi child for the isolated worker-settlement proof. It receives one
# serialized Pi request on stdin and emits protocol-valid transport JSON; it never contacts or
# loads a model.
request=$(cat)

case "$request" in
  *SETTLEMENT_JSON_UNPARSEABLE*)
    printf '%s' '{"version":"linggan.pi.v1/0.85.1","ok":true,"text":"not semantic json","failureCode":null,"modelIds":null,"modelListOrigin":null,"usage":{"inputTokens":null,"outputTokens":null,"costUsd":null},"elapsedMs":1}'
    ;;
  *SETTLEMENT_CONTRACT_REJECTED*)
    printf '%s' '{"version":"linggan.pi.v1/0.85.1","ok":true,"text":"{\"outcome\":\"atoms\",\"atoms\":[]}","failureCode":null,"modelIds":null,"modelListOrigin":null,"usage":{"inputTokens":null,"outputTokens":null,"costUsd":null},"elapsedMs":1}'
    ;;
  *SETTLEMENT_OFFSET_UNMAPPABLE*)
    printf '%s' '{"version":"linggan.pi.v1/0.85.1","ok":true,"text":"{\"outcome\":\"atoms\",\"atoms\":[{\"kind\":\"problem\",\"proposition\":\"synthetic offset proof\",\"basis\":\"explicit\",\"evidenceStart\":0,\"evidenceEnd\":1}]}","failureCode":null,"modelIds":null,"modelListOrigin":null,"usage":{"inputTokens":null,"outputTokens":null,"costUsd":null},"elapsedMs":1}'
    ;;
  *)
    exit 64
    ;;
esac
