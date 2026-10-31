-- A creation idea is a content-work object, separate from the research Topic graph.
-- Its name and intent live in the anchor work's revision and follow that work's source rules.
CREATE TABLE linggan_creation_idea_box (
    idea_ref uuid PRIMARY KEY,
    domain_ref uuid NOT NULL REFERENCES observation_domain(domain_ref),
    anchor_work_ref uuid NOT NULL UNIQUE REFERENCES linggan_creation_work(work_ref),
    created_at timestamptz NOT NULL DEFAULT scope_001_now()
);
CREATE INDEX linggan_creation_idea_box_domain ON linggan_creation_idea_box(domain_ref, created_at DESC);
