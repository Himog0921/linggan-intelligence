-- Preserve local-machine records and attribute new native draft registrations to a member.
ALTER TABLE linggan_creation_publication_link
    ADD COLUMN linked_by_member_ref uuid REFERENCES linggan_ops_member(member_ref),
    DROP CONSTRAINT linggan_creation_publication_link_linked_by_check,
    ADD CONSTRAINT linggan_creation_publication_link_actor_check CHECK (
        (linked_by = 'local_machine' AND linked_by_member_ref IS NULL)
        OR (linked_by = 'member' AND linked_by_member_ref IS NOT NULL)
    );
