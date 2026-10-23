-- A human may adopt saved cloud-manual OCR without inventing an audit sample.
-- Existing audit reviews remain immutable and keep their sample/issue foreign keys.
ALTER TABLE linggan_media_ocr_review
  ALTER COLUMN sample_ref DROP NOT NULL;
ALTER TABLE linggan_media_ocr_review
  ADD CONSTRAINT ocr_review_issue_requires_sample
  CHECK(sample_ref IS NOT NULL OR issue_ref IS NULL);
