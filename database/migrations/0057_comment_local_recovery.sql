-- Parser compatibility recovery is additive and never changes the original request ledger.
CREATE TABLE linggan_comment_local_recovery (
 invocation_ref uuid NOT NULL REFERENCES linggan_model_invocation,
 parser_version text NOT NULL,
 receipt_ref uuid NOT NULL UNIQUE,
 packet_ref uuid NOT NULL REFERENCES linggan_comment_daily_packet,
 source_refs uuid[] NOT NULL,
 analysis_refs uuid[] NOT NULL,
 input_hash text NOT NULL,
 output_hash text NOT NULL,
 receipt jsonb NOT NULL CHECK(jsonb_typeof(receipt)='object'),
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 PRIMARY KEY(invocation_ref,parser_version)
);
CREATE TRIGGER linggan_comment_local_recovery_immutable BEFORE UPDATE OR DELETE ON linggan_comment_local_recovery
 FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
