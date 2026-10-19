"""Read-only PostgreSQL snapshot text. Relation presence is explicitly queried first."""


def query(presence):
    if presence not in ("0,0", "0,1", "1,1"):
        raise ValueError("ai_snapshot_schema_incompatible")
    invocation_count = "(SELECT count(*) FROM linggan_model_invocation WHERE state='running')" if presence.endswith(",1") else "0"
    items = "0"
    lock = "false"
    executor = "NULL"
    if presence.startswith("1,"):
        items = "(SELECT count(*) FROM linggan_ai_batch_item_control WHERE state='running')"
        lock = """EXISTS(SELECT 1 FROM pg_locks l WHERE l.locktype='advisory' AND l.granted
            AND l.objsubid=1 AND l.database=(SELECT oid FROM pg_database WHERE datname=current_database())
            AND l.classid=((linggan_ai_executor_lock_key()>>32)&4294967295)::oid
            AND l.objid=(linggan_ai_executor_lock_key()&4294967295)::oid)"""
        executor = """(SELECT jsonb_build_object('instanceRef',i.instance_ref,'generation',i.generation::text,
            'processId',i.process_id,'sourceRevision',i.source_revision,'state',i.state)
            FROM linggan_ai_executor_current c JOIN linggan_ai_executor_instance i USING(instance_ref,generation))"""
    return f"""SELECT jsonb_build_object('contract','ai-drain-snapshot.v1',
        'databaseKey',(SELECT system_identifier::text FROM pg_control_system())||':'||
            current_database()||':'||(SELECT oid::text FROM pg_database WHERE datname=current_database())||':'||current_schema(),
        'runningInvocations',{invocation_count},'runningItems',{items},'sessionLockHeld',{lock},
        'executor',{executor})"""
