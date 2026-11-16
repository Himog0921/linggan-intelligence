-- Preserve the existing restriction-write contract without granting callers
-- access to opportunity tables. Both triggers remain tied to the original
-- restriction table and continue to serialize withdrawals with card builds.
DO $migration$
DECLARE
    target_schema name := current_schema();
    expected_table text := format('%I.%I',target_schema,'linggan_material_comment_restriction');
    trusted_path text := format('pg_catalog,%I,pg_temp',target_schema);
    gate_owner oid;
    changed_owner oid;
BEGIN
    EXECUTE format($definition$
    CREATE OR REPLACE FUNCTION %1$I.linggan_opportunity_card_restriction_gate()
    RETURNS trigger LANGUAGE plpgsql SECURITY DEFINER
    SET search_path=pg_catalog,pg_temp SET row_security=off AS $function$
    DECLARE affected_content uuid;
    DECLARE affected_comment text;
    BEGIN
      IF TG_RELID <> %2$L::regclass THEN
        RAISE EXCEPTION 'opportunity_restriction_trigger_target_invalid' USING ERRCODE='42501';
      END IF;
      PERFORM set_config('search_path',%3$L,true);
      IF TG_OP='INSERT' THEN
        PERFORM linggan_opportunity_card_touch_source(NEW.content_public_ref,NEW.comment_external_id);
      ELSIF TG_OP='DELETE' THEN
        PERFORM linggan_opportunity_card_touch_source(OLD.content_public_ref,OLD.comment_external_id);
      ELSE
        FOR affected_content,affected_comment IN
          SELECT DISTINCT source.content_ref,source.comment_external_id FROM (VALUES
            (OLD.content_public_ref,OLD.comment_external_id),
            (NEW.content_public_ref,NEW.comment_external_id)
          ) source(content_ref,comment_external_id) ORDER BY 1,2
        LOOP
          PERFORM linggan_opportunity_card_touch_source(affected_content,affected_comment);
        END LOOP;
      END IF;
      IF TG_OP='DELETE' THEN RETURN OLD; END IF;
      RETURN NEW;
    END $function$
    $definition$,target_schema,expected_table,trusted_path);

    EXECUTE format($definition$
    CREATE OR REPLACE FUNCTION %1$I.linggan_opportunity_card_restriction_changed()
    RETURNS trigger LANGUAGE plpgsql SECURITY DEFINER
    SET search_path=pg_catalog,pg_temp SET row_security=off AS $function$
    DECLARE affected_content uuid;
    DECLARE affected_comment text;
    BEGIN
      IF TG_RELID <> %2$L::regclass THEN
        RAISE EXCEPTION 'opportunity_restriction_trigger_target_invalid' USING ERRCODE='42501';
      END IF;
      PERFORM set_config('search_path',%3$L,true);
      IF TG_OP='DELETE' THEN
        affected_content := OLD.content_public_ref;
        affected_comment := OLD.comment_external_id;
      ELSE
        affected_content := NEW.content_public_ref;
        affected_comment := NEW.comment_external_id;
      END IF;
      IF TG_OP<>'DELETE' THEN
        DELETE FROM linggan_opportunity_opportunity_card card
        USING linggan_opportunity_card_voice_ref voice
        WHERE voice.problem_ref=card.problem_ref AND voice.content_ref=affected_content
          AND voice.comment_external_id=affected_comment;
      END IF;
      PERFORM linggan_opportunity_card_source_changed('content',affected_content::text);
      IF TG_OP='UPDATE' AND OLD.content_public_ref IS DISTINCT FROM NEW.content_public_ref THEN
        PERFORM linggan_opportunity_card_source_changed('content',OLD.content_public_ref::text);
      END IF;
      IF TG_OP='DELETE' THEN RETURN OLD; END IF;
      RETURN NEW;
    END $function$
    $definition$,target_schema,expected_table,trusted_path);

    EXECUTE format('REVOKE ALL ON FUNCTION %I.linggan_opportunity_card_restriction_gate() FROM PUBLIC',target_schema);
    EXECUTE format('REVOKE ALL ON FUNCTION %I.linggan_opportunity_card_restriction_changed() FROM PUBLIC',target_schema);
    IF EXISTS (
      SELECT 1 FROM pg_catalog.pg_proc function_def
      CROSS JOIN LATERAL pg_catalog.aclexplode(COALESCE(function_def.proacl,pg_catalog.acldefault('f',function_def.proowner))) privilege
      WHERE function_def.oid IN (
        format('%I.linggan_opportunity_card_restriction_gate()',target_schema)::regprocedure,
        format('%I.linggan_opportunity_card_restriction_changed()',target_schema)::regprocedure
      ) AND privilege.grantee<>function_def.proowner
    ) THEN
      RAISE EXCEPTION 'opportunity_restriction_default_grants_require_review' USING ERRCODE='55000';
    END IF;
    SELECT proowner INTO gate_owner FROM pg_catalog.pg_proc
      WHERE oid=format('%I.linggan_opportunity_card_restriction_gate()',target_schema)::regprocedure;
    SELECT proowner INTO changed_owner FROM pg_catalog.pg_proc
      WHERE oid=format('%I.linggan_opportunity_card_restriction_changed()',target_schema)::regprocedure;
    IF NOT pg_catalog.has_schema_privilege(gate_owner,target_schema,'USAGE')
       OR NOT pg_catalog.has_schema_privilege(changed_owner,target_schema,'USAGE')
       OR NOT pg_catalog.has_function_privilege(gate_owner,format('%I.linggan_opportunity_card_touch_source(uuid,text)',target_schema)::regprocedure,'EXECUTE')
       OR NOT pg_catalog.has_function_privilege(changed_owner,format('%I.linggan_opportunity_card_source_changed(text,text)',target_schema)::regprocedure,'EXECUTE')
       OR NOT pg_catalog.has_function_privilege(changed_owner,format('%I.linggan_opportunity_card_event_gate_starting()',target_schema)::regprocedure,'EXECUTE')
       OR NOT pg_catalog.has_function_privilege(changed_owner,format('%I.linggan_opportunity_card_event_gate_intent_key()',target_schema)::regprocedure,'EXECUTE')
       OR NOT pg_catalog.has_function_privilege(changed_owner,format('%I.linggan_opportunity_card_event_gate_key()',target_schema)::regprocedure,'EXECUTE')
       OR NOT pg_catalog.has_function_privilege(changed_owner,format('%I.linggan_opportunity_card_namespace()',target_schema)::regprocedure,'EXECUTE')
       OR NOT pg_catalog.has_table_privilege(gate_owner,format('%I.linggan_opportunity_card_source_guard',target_schema)::regclass,'SELECT')
       OR NOT pg_catalog.has_table_privilege(gate_owner,format('%I.linggan_opportunity_card_source_guard',target_schema)::regclass,'INSERT')
       OR NOT pg_catalog.has_table_privilege(gate_owner,format('%I.linggan_opportunity_card_source_guard',target_schema)::regclass,'UPDATE')
       OR NOT pg_catalog.has_table_privilege(changed_owner,format('%I.linggan_opportunity_opportunity_card',target_schema)::regclass,'SELECT')
       OR NOT pg_catalog.has_table_privilege(changed_owner,format('%I.linggan_opportunity_opportunity_card',target_schema)::regclass,'DELETE')
       OR NOT pg_catalog.has_table_privilege(changed_owner,format('%I.linggan_opportunity_card_voice_ref',target_schema)::regclass,'SELECT')
       OR NOT pg_catalog.has_table_privilege(changed_owner,format('%I.linggan_opportunity_card_source_event',target_schema)::regclass,'INSERT') THEN
      RAISE EXCEPTION 'opportunity_restriction_trigger_owner_permissions_incomplete' USING ERRCODE='55000';
    END IF;
END
$migration$;
