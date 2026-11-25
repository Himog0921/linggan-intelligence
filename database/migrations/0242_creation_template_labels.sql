-- Optional catalog labels belong to an immutable template revision.
ALTER TABLE linggan_creation_template_revision
    ADD COLUMN labels jsonb,
    ADD CONSTRAINT creation_template_labels_object
        CHECK (labels IS NULL OR (
            jsonb_typeof(labels) = 'object'
            AND jsonb_typeof(labels->'name') = 'string'
            AND jsonb_typeof(labels->'tags') = 'array'
        ) IS TRUE);
