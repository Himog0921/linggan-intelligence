-- One-time creator preview: a discovery operation, not long-term monitoring.
-- Number 0180 follows the in-flight unified research migrations 0169–0179.
ALTER TABLE collection_run DROP CONSTRAINT collection_run_source_kind_check;
ALTER TABLE collection_run ADD CONSTRAINT collection_run_source_kind_check CHECK(source_kind IN('temporary_keyword','observation_target','selected_works','archive','patrol','supplement','creator_preview','creator_selected_works'));
ALTER TABLE collection_run DROP CONSTRAINT collection_run_comment_mode_check;
ALTER TABLE collection_run ADD CONSTRAINT collection_run_comment_mode_check CHECK(comment_mode IN('sample_30','all_public','up_to','none'));

ALTER TABLE linggan_material_domain_usage ADD COLUMN use_kind text NOT NULL DEFAULT 'domain_research' CHECK(use_kind IN('domain_research','creator_context'));
CREATE INDEX creator_context_domain_usage_read ON linggan_material_domain_usage(domain_ref,content_public_ref,role) WHERE use_kind='creator_context';

CREATE FUNCTION linggan_creator_preview_usage_kind() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NEW.work_order_ref IS NOT NULL AND EXISTS(
   SELECT 1 FROM collection_run_order relation JOIN collection_run run USING(run_ref)
   WHERE relation.work_order_ref=NEW.work_order_ref AND run.source_kind IN ('creator_preview','creator_selected_works')
 ) THEN NEW.use_kind := 'creator_context'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER creator_preview_material_usage_kind BEFORE INSERT ON linggan_material_domain_usage FOR EACH ROW EXECUTE FUNCTION linggan_creator_preview_usage_kind();

-- A selected study of homepage context must not silently promote all its works
-- to formal domain-research membership. Explicit adoption is a separate action.
COMMENT ON COLUMN linggan_material_domain_usage.use_kind IS 'domain_research: selected material for this domain; creator_context: homepage author context, not a domain topic assertion';
