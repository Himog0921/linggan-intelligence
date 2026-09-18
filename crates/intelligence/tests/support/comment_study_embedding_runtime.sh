#!/bin/sh
# Synthetic, local-only WeMM document runtime for the isolated comment-study proofs. It speaks the
# real stdin/stdout protocol — handshake first, then one response per request — but loads no model
# and reaches no network. The revision it reports is the one the adapter passed on argv, so the
# handshake still has to agree with the configured runtime rather than a constant copied here.
#
# The vector depends on the text: a checksum of the request's texts picks which component is 1.0.
# That makes it deterministic for a repeated input and distinct for different ones, which is what
# the qualification probe checks. It carries no semantics — proofs about distance seed the cache
# directly instead of going through this.
revision=""
while [ $# -gt 0 ]; do
  case "$1" in
    --model-revision) revision="$2"; shift 2 ;;
    *) shift ;;
  esac
done

printf '{"version":"linggan.wemm.v1","ok":true,"type":"ready","modelId":"Tencent/WeMM-Embedding-2B","modelRevision":"%s","encodingMode":"document","dimension":512,"backend":"cpu","dtype":"torch.bfloat16","torchVersion":"test","sentenceTransformersVersion":"test","coldStartMs":1,"peakRssBytes":%s}\n' "$revision" "$$"

while IFS= read -r request; do
  [ -n "$request" ] || continue
  rest=${request#*\"id\":\"}
  id=${rest%%\"*}
  texts=${request#*\"texts\":}
  checksum=$(printf '%s' "$texts" | /usr/bin/cksum | while read -r sum _rest; do printf '%s' "$sum"; done)
  hot=$((checksum % 512))

  vector=""
  index=0
  while [ "$index" -lt 512 ]; do
    if [ "$index" -eq "$hot" ]; then component="1.0"; else component="0.0"; fi
    if [ -z "$vector" ]; then vector="$component"; else vector="$vector,$component"; fi
    index=$((index + 1))
  done

  printf '{"version":"linggan.wemm.v1","id":"%s","ok":true,"dimension":512,"backend":"cpu","values":[[%s]],"elapsedMs":1,"peakRssBytes":%s}\n' "$id" "$vector" "$$"
done
