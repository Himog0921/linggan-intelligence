-- Free placement of work frames on the materials desk; a view preference, not authorship.
CREATE TABLE linggan_creation_desk_position (
    domain_ref uuid NOT NULL REFERENCES observation_domain(domain_ref),
    work_ref uuid NOT NULL REFERENCES linggan_creation_work(work_ref) ON DELETE CASCADE,
    x double precision NOT NULL,
    y double precision NOT NULL,
    updated_at timestamptz NOT NULL DEFAULT scope_001_now(),
    PRIMARY KEY (domain_ref, work_ref)
);
