-- COMMENT-RESEARCH-V1-REAL-CLOSURE-001
--
-- A RunItem records one attempt under a precise research input and contract. Historical V1
-- rows retain their frozen input hash until the explicit development reset removes them.

ALTER TABLE linggan_comment_research_run_item
  ADD COLUMN research_fingerprint text
    NOT NULL DEFAULT '0000000000000000000000000000000000000000000000000000000000000000';

UPDATE linggan_comment_research_run_item
SET research_fingerprint=input_hash
WHERE research_fingerprint='0000000000000000000000000000000000000000000000000000000000000000';

ALTER TABLE linggan_comment_research_run_item
  ADD CONSTRAINT linggan_comment_research_run_item_fingerprint_valid
    CHECK(research_fingerprint ~ '^[0-9a-f]{64}$');

CREATE INDEX linggan_comment_research_run_item_derivation_fingerprint_latest_idx
  ON linggan_comment_research_run_item(derivation_ref,research_fingerprint,updated_at DESC,run_ref DESC);
