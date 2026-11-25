-- A structural template comes from a saved manuscript or one canonical evidence work.
ALTER TABLE linggan_creation_template
    ALTER COLUMN work_ref DROP NOT NULL,
    ALTER COLUMN revision_ref DROP NOT NULL,
    ADD COLUMN evidence_work_ref uuid REFERENCES linggan_material_content(public_ref),
    ADD COLUMN evidence_domain_ref uuid REFERENCES observation_domain(domain_ref),
    ADD COLUMN evidence_material_revision text CHECK (evidence_material_revision ~ '^[0-9a-f]{64}$'),
    ADD CONSTRAINT creation_template_evidence_work_unique UNIQUE (evidence_work_ref),
    ADD CONSTRAINT creation_template_one_source CHECK (
        (work_ref IS NOT NULL AND revision_ref IS NOT NULL
            AND evidence_work_ref IS NULL AND evidence_domain_ref IS NULL AND evidence_material_revision IS NULL)
        OR
        (work_ref IS NULL AND revision_ref IS NULL
            AND evidence_work_ref IS NOT NULL AND evidence_domain_ref IS NOT NULL AND evidence_material_revision IS NOT NULL)
    );
