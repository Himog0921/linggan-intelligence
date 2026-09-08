-- CI-RUN-002: bounded research context policy and short-lived diagnostic content.
-- Historical calls stay NOT_RECORDED; this never fabricates past prompts or responses.
CREATE TABLE linggan_comment_context_settings (
 singleton boolean PRIMARY KEY DEFAULT true CHECK(singleton),
 revision bigint NOT NULL DEFAULT 0 CHECK(revision>=0),
 policy jsonb NOT NULL DEFAULT '{}'::jsonb CHECK(jsonb_typeof(policy)='object'),
 updated_at timestamptz NOT NULL DEFAULT scope_001_now()
);
INSERT INTO linggan_comment_context_settings(singleton) VALUES(true);
ALTER TABLE linggan_ci_prepare ADD COLUMN context_policy jsonb NOT NULL DEFAULT '{}'::jsonb;
ALTER TABLE linggan_comment_daily_batch ADD COLUMN context_policy jsonb NOT NULL DEFAULT '{}'::jsonb;
ALTER TABLE linggan_comment_daily_schedule ADD COLUMN context_policy jsonb NOT NULL DEFAULT '{}'::jsonb;
CREATE TABLE linggan_comment_request_trace (
 invocation_ref uuid PRIMARY KEY REFERENCES linggan_model_invocation(invocation_ref),
 packet_ref uuid NOT NULL REFERENCES linggan_comment_daily_packet(packet_ref),
 input_content jsonb, output_content text,
 input_hash text NOT NULL, source_hashes jsonb NOT NULL,
 context_guard jsonb NOT NULL, policy jsonb NOT NULL,
 validation jsonb NOT NULL DEFAULT '[]'::jsonb,
 outcomes jsonb NOT NULL DEFAULT '{}'::jsonb,
 events jsonb NOT NULL DEFAULT '[]'::jsonb,
 expires_at timestamptz NOT NULL DEFAULT scope_001_now()+interval '24 hours',
 purged_at timestamptz,
 CHECK(input_content IS NULL OR octet_length(input_content::text)<=131072),
 CHECK(output_content IS NULL OR octet_length(output_content)<=65536)
);
CREATE INDEX linggan_comment_trace_expiry ON linggan_comment_request_trace(expires_at) WHERE purged_at IS NULL;
