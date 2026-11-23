-- Existing local-machine drafts have no authenticated member attribution.
ALTER TABLE linggan_creation_work
    ADD COLUMN created_by_member_ref uuid REFERENCES linggan_ops_member(member_ref);

CREATE INDEX linggan_creation_work_member
    ON linggan_creation_work(created_by_member_ref, domain_ref)
    WHERE created_by_member_ref IS NOT NULL;

ALTER TABLE linggan_creation_account_identity
    ADD COLUMN saved_by_member_ref uuid REFERENCES linggan_ops_member(member_ref);

ALTER TABLE linggan_creation_experience
    ADD COLUMN created_by_member_ref uuid REFERENCES linggan_ops_member(member_ref);

ALTER TABLE linggan_creation_persona_fact
    ADD COLUMN created_by_member_ref uuid REFERENCES linggan_ops_member(member_ref);

ALTER TABLE linggan_creation_fact_card
    ADD COLUMN saved_by_member_ref uuid REFERENCES linggan_ops_member(member_ref);

ALTER TABLE linggan_creation_bridge_sentence
    ADD COLUMN saved_by_member_ref uuid REFERENCES linggan_ops_member(member_ref);

ALTER TABLE linggan_creation_style
    ADD COLUMN saved_by_member_ref uuid REFERENCES linggan_ops_member(member_ref);

ALTER TABLE linggan_creation_template
    ADD COLUMN created_by_member_ref uuid REFERENCES linggan_ops_member(member_ref);
