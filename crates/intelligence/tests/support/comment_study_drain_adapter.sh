#!/bin/sh
# Local synthetic child that remains in flight briefly, then returns a provider failure. The
# PostgreSQL drain proof can request shutdown after dispatch and verify receipt settlement without
# contacting or loading a model.
cat >/dev/null
sleep 1
printf '%s' '{"version":"linggan.pi.v1/0.85.1","ok":false,"text":null,"failureCode":"synthetic_drain_failure","modelIds":null,"modelListOrigin":null,"usage":{"inputTokens":null,"outputTokens":null,"costUsd":null},"elapsedMs":1}'
