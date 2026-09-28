#!/bin/sh
# Synthetic local Pi child for the late-response scheduler proof. The test advances its isolated
# request deadline while this child waits; this process never contacts or loads a model.
cat >/dev/null
sleep 1
printf '%s' '{"version":"linggan.pi.v1/0.85.1","ok":true,"text":"{}","failureCode":null,"modelIds":null,"modelListOrigin":null,"usage":{"inputTokens":1,"outputTokens":1,"costUsd":null},"elapsedMs":1}'
