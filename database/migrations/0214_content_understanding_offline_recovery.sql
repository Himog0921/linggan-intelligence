-- A Core recovered from a complete saved reply is separate from the immutable provider receipt.
CREATE TABLE linggan_content_understanding_offline_recovery (
 request_ref uuid PRIMARY KEY REFERENCES linggan_content_understanding_result,
 work_ref uuid NOT NULL,
 material_snapshot_identity text NOT NULL,
 method_version text NOT NULL,
 result jsonb NOT NULL CHECK(jsonb_typeof(result)='object'),
 validator_revision text NOT NULL CHECK(validator_revision ~ '^[0-9a-f]{40}$'),
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 FOREIGN KEY(request_ref,work_ref,material_snapshot_identity,method_version)
   REFERENCES linggan_content_understanding_request(request_ref,work_ref,material_snapshot_identity,method_version),
 UNIQUE(work_ref,material_snapshot_identity,method_version)
);
CREATE TRIGGER content_understanding_offline_recovery_immutable
BEFORE UPDATE OR DELETE ON linggan_content_understanding_offline_recovery
FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();

CREATE VIEW linggan_content_understanding_saved_core AS
SELECT request_ref,work_ref,material_snapshot_identity,method_version,result,created_at,false AS offline_recovery
FROM linggan_content_understanding_result WHERE result IS NOT NULL
UNION ALL
SELECT request_ref,work_ref,material_snapshot_identity,method_version,result,created_at,true AS offline_recovery
FROM linggan_content_understanding_offline_recovery;
