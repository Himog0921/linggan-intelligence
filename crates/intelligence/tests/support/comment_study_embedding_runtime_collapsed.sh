#!/bin/sh
# A local runtime that answers every text with the same vector. It is protocol-valid and
# deterministic, so it passes every structural check the probe makes — dimension, finiteness, unit
# norm, repeat consistency — and is still useless: everything it encodes lands on one point.
# The probe has to refuse it, which is what this script exists to prove.
revision=""
while [ $# -gt 0 ]; do
  case "$1" in
    --model-revision) revision="$2"; shift 2 ;;
    *) shift ;;
  esac
done

vector="1.0"
index=1
while [ "$index" -lt 512 ]; do
  vector="$vector,0.0"
  index=$((index + 1))
done

printf '{"version":"linggan.wemm.v1","ok":true,"type":"ready","modelId":"Tencent/WeMM-Embedding-2B","modelRevision":"%s","encodingMode":"document","dimension":512,"backend":"cpu","dtype":"torch.bfloat16","torchVersion":"test","sentenceTransformersVersion":"test","coldStartMs":1,"peakRssBytes":%s}\n' "$revision" "$$"

while IFS= read -r request; do
  [ -n "$request" ] || continue
  rest=${request#*\"id\":\"}
  id=${rest%%\"*}
  printf '{"version":"linggan.wemm.v1","id":"%s","ok":true,"dimension":512,"backend":"cpu","values":[[%s]],"elapsedMs":1,"peakRssBytes":%s}\n' "$id" "$vector" "$$"
done
