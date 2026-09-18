#!/bin/sh
# Synthetic, local-only WeMM document runtime for the isolated comment-study proof. It speaks the
# real stdin/stdout protocol — handshake first, then one response per request — but loads no model
# and reaches no network. The revision it reports is the one the adapter passed on argv, so the
# handshake still has to agree with the configured runtime rather than a constant copied here.
#
# Every text encodes to the same unit vector. That is enough for proofs about *whether* encoding
# happened and when; proofs about distance seed the cache directly instead.
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

printf '{"version":"linggan.wemm.v1","ok":true,"type":"ready","modelId":"Tencent/WeMM-Embedding-2B","modelRevision":"%s","encodingMode":"document","dimension":512,"backend":"cpu"}\n' "$revision"

while IFS= read -r request; do
  [ -n "$request" ] || continue
  rest=${request#*\"id\":\"}
  id=${rest%%\"*}
  printf '{"version":"linggan.wemm.v1","id":"%s","ok":true,"dimension":512,"backend":"cpu","values":[[%s]]}\n' "$id" "$vector"
done
