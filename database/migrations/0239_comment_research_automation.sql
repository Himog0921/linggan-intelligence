-- A daily schedule creates reviewable selections. It never owns an AI batch.
CREATE TABLE linggan_comment_research_automation (
    domain_ref uuid PRIMARY KEY REFERENCES observation_domain,
    enabled boolean NOT NULL DEFAULT false,
    minute_of_day integer NOT NULL DEFAULT 540 CHECK (minute_of_day BETWEEN 0 AND 1439),
    time_zone text NOT NULL DEFAULT 'Asia/Shanghai' CHECK (char_length(time_zone) BETWEEN 1 AND 128),
    selection_limit integer NOT NULL DEFAULT 300 CHECK (selection_limit BETWEEN 1 AND 1000),
    revision bigint NOT NULL DEFAULT 0 CHECK (revision >= 0),
    enabled_from timestamptz,
    next_run_at timestamptz,
    CHECK (enabled = (next_run_at IS NOT NULL)),
    CHECK (NOT enabled OR enabled_from IS NOT NULL)
);

CREATE INDEX linggan_comment_research_automation_due
    ON linggan_comment_research_automation (next_run_at, domain_ref) WHERE enabled;

CREATE TABLE linggan_comment_research_selection (
    selection_ref uuid PRIMARY KEY,
    domain_ref uuid NOT NULL REFERENCES linggan_comment_research_automation,
    scheduled_at timestamptz NOT NULL,
    generated_at timestamptz NOT NULL CHECK (generated_at >= scheduled_at),
    UNIQUE (domain_ref, scheduled_at),
    UNIQUE (domain_ref, selection_ref)
);

CREATE TABLE linggan_comment_research_selection_target (
    domain_ref uuid NOT NULL,
    selection_ref uuid NOT NULL,
    position integer NOT NULL CHECK (position BETWEEN 1 AND 1000),
    work_ref uuid NOT NULL REFERENCES linggan_material_content(public_ref),
    comment_external_id text NOT NULL,
    source_ref uuid NOT NULL REFERENCES linggan_material_comment(material_ref),
    FOREIGN KEY (domain_ref, selection_ref) REFERENCES linggan_comment_research_selection(domain_ref, selection_ref),
    PRIMARY KEY (selection_ref, position),
    UNIQUE (domain_ref, work_ref, comment_external_id)
);

CREATE FUNCTION linggan_comment_research_next_run(p_now timestamptz, p_minute integer, p_time_zone text)
RETURNS timestamptz LANGUAGE sql STABLE AS $$
    WITH local_day AS (SELECT (p_now AT TIME ZONE p_time_zone)::date AS day),
         slot AS (SELECT day, (day + make_interval(mins => p_minute)) AT TIME ZONE p_time_zone AS today FROM local_day)
    SELECT CASE WHEN today > p_now THEN today
                ELSE ((day + 1) + make_interval(mins => p_minute)) AT TIME ZONE p_time_zone END
    FROM slot
$$;
