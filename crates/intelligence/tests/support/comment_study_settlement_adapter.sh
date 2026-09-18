#!/bin/sh
# Synthetic, local-only Pi child for the isolated comment-study settlement proof. It receives one
# serialized Pi request on stdin and emits protocol-valid transport JSON; it never contacts or
# loads a model. The marker it branches on travels inside the frozen batch manifest, so each case
# exercises the real adapter boundary rather than a stubbed call.
request=$(cat)

case "$request" in
  *SETTLEMENT_TRANSPORT_LIMIT*)
    # What #305 saw: the response body outgrew the transport guard, so no model text exists and
    # no usage is known.
    printf '%s' '{"version":"linggan.pi.v1/0.85.1","ok":false,"text":null,"failureCode":"response_too_large","modelIds":null,"modelListOrigin":null,"usage":{"inputTokens":null,"outputTokens":null,"costUsd":null},"elapsedMs":7}'
    ;;
  *SETTLEMENT_UNPARSEABLE*)
    # A billed call that produced text which is not the contract JSON. The usage is deliberately
    # known here: it is what the old order discarded by returning before the checkpoint.
    printf '%s' '{"version":"linggan.pi.v1/0.85.1","ok":true,"text":"这不是合同要求的 JSON","failureCode":null,"modelIds":null,"modelListOrigin":null,"usage":{"inputTokens":111,"outputTokens":222,"costUsd":null},"elapsedMs":9}'
    ;;
  *)
    printf '%s' '{"version":"linggan.pi.v1/0.85.1","ok":false,"text":null,"failureCode":"provider_failed","modelIds":null,"modelListOrigin":null,"usage":{"inputTokens":null,"outputTokens":null,"costUsd":null},"elapsedMs":1}'
    ;;
esac
