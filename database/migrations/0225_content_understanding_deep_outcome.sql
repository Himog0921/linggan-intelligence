-- L2 outputs that were not published (withheld, invalid or insufficient): the raw
-- model output and per-finding rejections, kept for evaluation only. Not read by
-- the product API.
CREATE TABLE linggan_content_understanding_deep_outcome (
 invocation_ref uuid PRIMARY KEY REFERENCES linggan_model_invocation,
 request_ref uuid NOT NULL REFERENCES linggan_content_understanding_deep_request(request_ref),
 failure_code text NOT NULL CHECK (failure_code IN (
  'ai_output_invalid',
  'content_understanding_deep_rejected_excess',
  'content_understanding_deep_insufficient_evidence'
 )),
 raw_output text NOT NULL,
 rejected jsonb NOT NULL CHECK (jsonb_typeof(rejected)='array'),
 created_at timestamptz NOT NULL DEFAULT scope_001_now()
);
CREATE INDEX content_understanding_deep_outcome_request
 ON linggan_content_understanding_deep_outcome(request_ref);
CREATE TRIGGER content_understanding_deep_outcome_immutable
 BEFORE UPDATE OR DELETE ON linggan_content_understanding_deep_outcome
 FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
