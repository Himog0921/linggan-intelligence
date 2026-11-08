-- Free placement of work frames and their materials on the materials desk;
-- view preferences, not authorship.
CREATE TABLE linggan_creation_desk_position (
    domain_ref uuid NOT NULL REFERENCES observation_domain(domain_ref),
    work_ref uuid NOT NULL REFERENCES linggan_creation_work(work_ref) ON DELETE CASCADE,
    x double precision NOT NULL,
    y double precision NOT NULL,
    width double precision,
    height double precision,
    updated_at timestamptz NOT NULL DEFAULT scope_001_now(),
    PRIMARY KEY (domain_ref, work_ref)
);

-- Card offsets are relative to their frame, so moving the frame carries the cards.
CREATE TABLE linggan_creation_desk_card_position (
    domain_ref uuid NOT NULL REFERENCES observation_domain(domain_ref),
    work_ref uuid NOT NULL REFERENCES linggan_creation_work(work_ref) ON DELETE CASCADE,
    block_ref uuid NOT NULL,
    x double precision NOT NULL,
    y double precision NOT NULL,
    updated_at timestamptz NOT NULL DEFAULT scope_001_now(),
    PRIMARY KEY (domain_ref, work_ref, block_ref)
);
