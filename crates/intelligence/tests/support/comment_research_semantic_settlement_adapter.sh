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
  *SETTLEMENT_QUOTE_VALID*)
    printf '%s' '{"version":"linggan.pi.v1/0.85.1","ok":true,"text":"{\"outcome\":\"atoms\",\"atoms\":[{\"kind\":\"problem\",\"proposition\":\"孩子存在作业持续拖延\",\"basis\":\"explicit\",\"evidence\":\"孩子写作业总是拖延\",\"problemFrame\":{\"scopeRelation\":\"in_scope\",\"subject\":{\"value\":\"孩子\",\"basis\":\"context_resolved\",\"evidenceRefs\":[\"atom_evidence\"]},\"goal\":{\"value\":\"自主完成作业\",\"basis\":\"context_resolved\",\"evidenceRefs\":[\"atom_evidence\"]},\"barrier\":{\"value\":\"作业持续拖延\",\"basis\":\"explicit\",\"evidenceRefs\":[\"atom_evidence\"]},\"context\":{\"value\":null,\"basis\":\"unknown\",\"evidenceRefs\":[]}}}]}","failureCode":null,"modelIds":null,"modelListOrigin":null,"usage":{"inputTokens":null,"outputTokens":null,"costUsd":null},"elapsedMs":1}'
    ;;
  *SETTLEMENT_OFFSET_UNMAPPABLE*)
    printf '%s' '{"version":"linggan.pi.v1/0.85.1","ok":true,"text":"{\"outcome\":\"atoms\",\"atoms\":[{\"kind\":\"problem\",\"proposition\":\"synthetic offset proof\",\"basis\":\"explicit\",\"evidence\":\"SETTLEMENT_OFFSET_UNMAPPABLE\",\"problemFrame\":{\"scopeRelation\":\"in_scope\",\"subject\":{\"value\":\"孩子\",\"basis\":\"context_resolved\",\"evidenceRefs\":[\"atom_evidence\"]},\"goal\":{\"value\":null,\"basis\":\"unknown\",\"evidenceRefs\":[]},\"barrier\":{\"value\":\"无法完成题目\",\"basis\":\"context_resolved\",\"evidenceRefs\":[\"atom_evidence\"]},\"context\":{\"value\":null,\"basis\":\"unknown\",\"evidenceRefs\":[]}}}]}","failureCode":null,"modelIds":null,"modelListOrigin":null,"usage":{"inputTokens":null,"outputTokens":null,"costUsd":null},"elapsedMs":1}'
    ;;
  *)
    exit 64
    ;;
esac
