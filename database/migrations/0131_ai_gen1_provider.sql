-- AI-GEN1 / PR-1B. Configuration facts only: no grant, switch or model call is enabled.
CREATE TABLE linggan_ai_task (
 task_key text PRIMARY KEY,
 label text NOT NULL,
 lifecycle text NOT NULL CHECK(lifecycle IN ('pending','active','frozen','retired','legacy')),
 automatic_brake boolean NOT NULL
);
INSERT INTO linggan_ai_task(task_key,label,lifecycle,automatic_brake) VALUES
 ('note_interpretation','每篇解读','pending',true),
 ('direction_list_draft','方向清单起草','pending',true),
 ('direction_proposal','新方向提议','pending',true),
 ('author_org_identity','作者公开简介判断','pending',true),
 ('writing_assistant','写作助手','pending',false),
 ('writing_legacy','既有写作建议','active',false),
 ('model_probe','连接与能力测试','active',false),
 ('comment_study','评论研究','frozen',false),
 ('comment_embedding','评论向量','frozen',false),
 ('legacy_creator_discovery','旧创作者分析','retired',false),
 ('legacy_topic_map','旧主题研究','retired',false),
 ('legacy_unclassified','历史未分类调用','legacy',false);

-- A route epoch changes whenever the normalized API/address/local-mode tuple changes.
-- Returning to an older address does not return to its previous approval epoch.
CREATE TABLE linggan_ai_endpoint (
 endpoint_ref uuid PRIMARY KEY,
 connection_ref uuid NOT NULL REFERENCES linggan_model_connection,
 first_revision integer NOT NULL CHECK(first_revision>0),
 api text NOT NULL CHECK(api IN ('openai-completions','openai-responses','anthropic-messages','local-wemm-runtime')),
 base_url text NOT NULL,
 local_endpoint boolean NOT NULL,
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 UNIQUE(connection_ref,first_revision),
 UNIQUE(endpoint_ref,connection_ref)
);
CREATE TABLE linggan_ai_connection_endpoint (
 version_ref uuid PRIMARY KEY REFERENCES linggan_model_connection_version,
 connection_ref uuid NOT NULL REFERENCES linggan_model_connection,
 endpoint_ref uuid NOT NULL,
 FOREIGN KEY(endpoint_ref,connection_ref) REFERENCES linggan_ai_endpoint(endpoint_ref,connection_ref)
);
WITH ordered AS (
 SELECT v.*,CASE WHEN lag(jsonb_build_array(api,base_url,local_endpoint)) OVER w
 IS NOT DISTINCT FROM jsonb_build_array(api,base_url,local_endpoint) THEN 0 ELSE 1 END AS changed
 FROM linggan_model_connection_version v WINDOW w AS(PARTITION BY connection_ref ORDER BY revision)
), grouped AS (
 SELECT *,sum(changed) OVER(PARTITION BY connection_ref ORDER BY revision) AS route_group FROM ordered
), firsts AS (
 SELECT DISTINCT ON(connection_ref,route_group) version_ref,connection_ref,revision,api,base_url,local_endpoint,created_at
 FROM grouped ORDER BY connection_ref,route_group,revision
)
INSERT INTO linggan_ai_endpoint SELECT version_ref,connection_ref,revision,api,base_url,local_endpoint,created_at FROM firsts;
INSERT INTO linggan_ai_connection_endpoint(version_ref,connection_ref,endpoint_ref)
 SELECT v.version_ref,v.connection_ref,e.endpoint_ref
 FROM linggan_model_connection_version v
 JOIN LATERAL(SELECT endpoint_ref FROM linggan_ai_endpoint e WHERE e.connection_ref=v.connection_ref
 AND e.first_revision<=v.revision ORDER BY e.first_revision DESC LIMIT 1)e ON true;

CREATE FUNCTION linggan_project_ai_endpoint() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE previous linggan_model_connection_version%ROWTYPE; route uuid;
BEGIN
 PERFORM 1 FROM linggan_model_connection WHERE connection_ref=NEW.connection_ref FOR UPDATE;
 SELECT * INTO previous FROM linggan_model_connection_version
 WHERE connection_ref=NEW.connection_ref AND version_ref<>NEW.version_ref ORDER BY revision DESC LIMIT 1;
 IF FOUND THEN
  IF previous.revision>=NEW.revision THEN RAISE EXCEPTION 'ai_connection_version_not_monotonic'; END IF;
  IF (previous.api,previous.base_url,previous.local_endpoint)=(NEW.api,NEW.base_url,NEW.local_endpoint) THEN
   SELECT endpoint_ref INTO STRICT route FROM linggan_ai_connection_endpoint WHERE version_ref=previous.version_ref;
  END IF;
 END IF;
 IF route IS NULL THEN
  route:=NEW.version_ref;
  INSERT INTO linggan_ai_endpoint(endpoint_ref,connection_ref,first_revision,api,base_url,local_endpoint,created_at)
  VALUES(route,NEW.connection_ref,NEW.revision,NEW.api,NEW.base_url,NEW.local_endpoint,NEW.created_at);
 END IF;
 INSERT INTO linggan_ai_connection_endpoint VALUES(NEW.version_ref,NEW.connection_ref,route);
 RETURN NULL;
END $$;
CREATE TRIGGER ai_connection_endpoint AFTER INSERT ON linggan_model_connection_version
 FOR EACH ROW EXECUTE FUNCTION linggan_project_ai_endpoint();

CREATE TABLE linggan_ai_model_identity (
 identity_ref uuid PRIMARY KEY,
 connection_ref uuid NOT NULL REFERENCES linggan_model_connection,
 model_id text NOT NULL CHECK(char_length(model_id) BETWEEN 1 AND 200),
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 UNIQUE(connection_ref,model_id)
);
INSERT INTO linggan_ai_model_identity
 SELECT DISTINCT ON(v.connection_ref,m.model_id) m.model_ref,v.connection_ref,m.model_id,m.created_at
 FROM linggan_model_entry m JOIN linggan_model_connection_version v ON v.version_ref=m.connection_version_ref
 ORDER BY v.connection_ref,m.model_id,m.created_at,m.model_ref;
CREATE FUNCTION linggan_project_ai_model_identity() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 INSERT INTO linggan_ai_model_identity(identity_ref,connection_ref,model_id,created_at)
 SELECT NEW.model_ref,connection_ref,NEW.model_id,NEW.created_at FROM linggan_model_connection_version WHERE version_ref=NEW.connection_version_ref
 ON CONFLICT(connection_ref,model_id) DO NOTHING;
 RETURN NULL;
END $$;
CREATE TRIGGER ai_model_identity AFTER INSERT ON linggan_model_entry
 FOR EACH ROW EXECUTE FUNCTION linggan_project_ai_model_identity();

CREATE FUNCTION linggan_ai_thinking_levels_valid(value jsonb) RETURNS boolean
 LANGUAGE sql IMMUTABLE AS $$
 SELECT CASE WHEN jsonb_typeof(value)<>'array' THEN false ELSE
 jsonb_array_length(value)>0 AND NOT EXISTS(SELECT 1 FROM jsonb_array_elements(value)x
 WHERE jsonb_typeof(x)<>'string' OR (x#>>'{}') NOT IN ('off','minimal','low','medium','high','xhigh','max'))
 AND jsonb_array_length(value)=(SELECT count(DISTINCT x) FROM jsonb_array_elements(value)x) END
 $$;
-- Frozen resource limits mirror ai-resource-limits.v1. Tests compare the JSON source.
CREATE TABLE linggan_model_capability (
 capability_ref uuid PRIMARY KEY,
 revision_sequence bigint GENERATED ALWAYS AS IDENTITY UNIQUE,
 identity_ref uuid NOT NULL REFERENCES linggan_ai_model_identity,
 endpoint_ref uuid NOT NULL REFERENCES linggan_ai_endpoint,
 source_kind text NOT NULL CHECK(source_kind IN ('pi_catalog','manual','probe')),
 source_version text NOT NULL CHECK(length(source_version)>0),
 probe_invocation_ref uuid REFERENCES linggan_model_invocation,
 context_tokens integer NOT NULL CHECK(context_tokens BETWEEN 1024 AND 2097152),
 max_output_tokens integer NOT NULL CHECK(max_output_tokens BETWEEN 1 AND 2097152 AND max_output_tokens<=context_tokens),
 max_timeout_seconds integer NOT NULL CHECK(max_timeout_seconds BETWEEN 1 AND 3600),
 thinking_levels jsonb NOT NULL CHECK(linggan_ai_thinking_levels_valid(thinking_levels)),
 compat jsonb NOT NULL CHECK(jsonb_typeof(compat)='object' AND octet_length(compat::text)<=16384),
 thinking_level_map jsonb NOT NULL CHECK(jsonb_typeof(thinking_level_map)='object' AND octet_length(thinking_level_map::text)<=4096),
 output_mode text NOT NULL CHECK(output_mode IN ('prompt_only','json_object','json_schema')),
 strict_schema boolean NOT NULL DEFAULT false CHECK(NOT strict_schema OR output_mode='json_schema'),
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 CHECK((source_kind='probe')=(probe_invocation_ref IS NOT NULL))
);
CREATE INDEX ai_model_capability_current ON linggan_model_capability(identity_ref,endpoint_ref,revision_sequence DESC);
CREATE FUNCTION linggan_validate_ai_capability() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NOT EXISTS(SELECT 1 FROM linggan_ai_model_identity i JOIN linggan_ai_endpoint e USING(connection_ref)
 WHERE i.identity_ref=NEW.identity_ref AND e.endpoint_ref=NEW.endpoint_ref) THEN RAISE EXCEPTION 'ai_capability_endpoint_mismatch'; END IF;
 IF NEW.source_kind='probe' AND NOT EXISTS(
  SELECT 1 FROM linggan_model_invocation i JOIN linggan_model_connection_version v ON v.version_ref=i.connection_version_ref
  JOIN linggan_ai_connection_endpoint p ON p.version_ref=v.version_ref
  JOIN linggan_model_entry m ON m.model_ref=i.model_ref
  JOIN linggan_model_connection_version mv ON mv.version_ref=m.connection_version_ref AND mv.connection_ref=v.connection_ref
  JOIN linggan_ai_model_identity stable ON stable.connection_ref=v.connection_ref AND stable.model_id=m.model_id
  WHERE i.invocation_ref=NEW.probe_invocation_ref AND i.state='succeeded' AND i.operation='probe'
  AND stable.identity_ref=NEW.identity_ref AND p.endpoint_ref=NEW.endpoint_ref
 ) THEN RAISE EXCEPTION 'ai_capability_probe_not_proven'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER ai_capability_qualification BEFORE INSERT ON linggan_model_capability
 FOR EACH ROW EXECUTE FUNCTION linggan_validate_ai_capability();

CREATE TABLE linggan_model_price (
 price_ref uuid PRIMARY KEY,
 revision_sequence bigint GENERATED ALWAYS AS IDENTITY UNIQUE,
 identity_ref uuid NOT NULL REFERENCES linggan_ai_model_identity,
 endpoint_ref uuid NOT NULL REFERENCES linggan_ai_endpoint,
 currency text NOT NULL DEFAULT 'USD' CHECK(currency='USD'),
 input_micro_usd_per_million bigint CHECK(input_micro_usd_per_million>=0),
 output_micro_usd_per_million bigint CHECK(output_micro_usd_per_million>=0),
 cached_read_micro_usd_per_million bigint CHECK(cached_read_micro_usd_per_million>=0),
 cached_write_micro_usd_per_million bigint CHECK(cached_write_micro_usd_per_million>=0),
 source_kind text NOT NULL CHECK(source_kind IN ('manual','pi_catalog')),
 source_version text NOT NULL CHECK(length(source_version)>0),
 created_at timestamptz NOT NULL DEFAULT scope_001_now()
);
CREATE INDEX ai_model_price_current ON linggan_model_price(identity_ref,endpoint_ref,revision_sequence DESC);
CREATE FUNCTION linggan_validate_ai_price() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NOT EXISTS(SELECT 1 FROM linggan_ai_model_identity i JOIN linggan_ai_endpoint e USING(connection_ref)
 WHERE i.identity_ref=NEW.identity_ref AND e.endpoint_ref=NEW.endpoint_ref) THEN RAISE EXCEPTION 'ai_price_endpoint_mismatch'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER ai_price_qualification BEFORE INSERT ON linggan_model_price
 FOR EACH ROW EXECUTE FUNCTION linggan_validate_ai_price();

CREATE TABLE linggan_model_egress_event (
 event_ref uuid PRIMARY KEY,
 event_sequence bigint GENERATED ALWAYS AS IDENTITY UNIQUE,
 endpoint_ref uuid NOT NULL REFERENCES linggan_ai_endpoint,
 task_key text NOT NULL REFERENCES linggan_ai_task,
 material_class text NOT NULL CHECK(material_class IN ('synthetic_probe','model_catalog','note_material','public_author_bio','selected_writing_material')),
 allowed boolean NOT NULL,
 actor_kind text NOT NULL CHECK(actor_kind IN ('human','system')),
 reason text NOT NULL CHECK(char_length(btrim(reason)) BETWEEN 1 AND 300),
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 CHECK(actor_kind='human' OR NOT allowed),
 CHECK((task_key='model_probe' AND material_class IN ('synthetic_probe','model_catalog'))
 OR (task_key IN ('note_interpretation','direction_list_draft','direction_proposal') AND material_class='note_material')
 OR (task_key='author_org_identity' AND material_class='public_author_bio')
 OR (task_key IN ('writing_assistant','writing_legacy') AND material_class='selected_writing_material'))
);
CREATE INDEX ai_model_egress_current ON linggan_model_egress_event(endpoint_ref,task_key,material_class,event_sequence DESC);
CREATE VIEW linggan_ai_current_egress AS
 SELECT DISTINCT ON(endpoint_ref,task_key,material_class) * FROM linggan_model_egress_event
 WHERE created_at<=scope_001_now() ORDER BY endpoint_ref,task_key,material_class,event_sequence DESC;
CREATE FUNCTION linggan_ai_budget_day(instant timestamptz DEFAULT scope_001_now()) RETURNS date
 LANGUAGE sql IMMUTABLE AS $$ SELECT (instant AT TIME ZONE 'Asia/Shanghai')::date $$;

CREATE TRIGGER ai_endpoint_immutable BEFORE UPDATE OR DELETE ON linggan_ai_endpoint FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TRIGGER ai_connection_endpoint_immutable BEFORE UPDATE OR DELETE ON linggan_ai_connection_endpoint FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TRIGGER ai_model_identity_immutable BEFORE UPDATE OR DELETE ON linggan_ai_model_identity FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TRIGGER ai_capability_immutable BEFORE UPDATE OR DELETE ON linggan_model_capability FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TRIGGER ai_price_immutable BEFORE UPDATE OR DELETE ON linggan_model_price FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TRIGGER ai_egress_immutable BEFORE UPDATE OR DELETE ON linggan_model_egress_event FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
