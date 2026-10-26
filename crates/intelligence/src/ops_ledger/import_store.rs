//! Writes one import batch. Every write happens in the caller's transaction; a failure anywhere
//! leaves no rows behind. Rows reference the batch, which is checked at commit (deferred keys).

use sqlx::PgConnection;
use uuid::Uuid;

use super::OpsImportError;
use super::import_diff::ChangeSet;
use super::import_plan::{PlannedAccount, PlannedDaily, PlannedPublication};

/// Writes every change of the set under `batch_ref`. Rows referencing the batch are checked
/// against the batch row at commit (deferred foreign keys).
pub(super) async fn write_changes(
    connection: &mut PgConnection,
    changes: &ChangeSet,
    batch_ref: Uuid,
    domain_ref: Uuid,
    as_of: &str,
) -> Result<(), OpsImportError> {
    // D1-only schemas remain useful for historic-ledger proofs and rolling upgrades. The
    // registration write must keep working until 0159 is present; once it is, field facts are
    // appended in the same transaction as the import.
    let field_adoption_ready: bool = sqlx::query_scalar(
        "SELECT to_regclass('linggan_ops_publication_field_adoption') IS NOT NULL",
    )
    .fetch_one(&mut *connection)
    .await?;
    for (account_ref, account) in &changes.accounts {
        upsert_account(connection, *account_ref, account, batch_ref, domain_ref).await?;
    }
    for (publication_ref, publication) in &changes.publications {
        let account_ref = publication
            .account_source_id
            .as_ref()
            .map(|id| changes.account_refs[id]);
        let merged_into = publication
            .merged_into_source_id
            .as_ref()
            .map(|id| changes.publication_refs[id]);
        upsert_publication(
            connection,
            *publication_ref,
            publication,
            (account_ref, merged_into),
            batch_ref,
            field_adoption_ready,
        )
        .await?;
    }
    for (publication_ref, view_count) in &changes.view_reports {
        sqlx::query(
            "INSERT INTO linggan_ops_view_report \
               (report_ref, publication_ref, report_kind, view_count, source_kind, batch_ref, as_of) \
             VALUES ($1, $2, CASE WHEN $3::bigint IS NULL THEN 'cleared' ELSE 'value' END, $3, \
                     'workbench_import', $4, $5::timestamptz)",
        )
        .bind(Uuid::new_v4())
        .bind(publication_ref)
        .bind(view_count)
        .bind(batch_ref)
        .bind(as_of)
        .execute(&mut *connection)
        .await?;
    }
    // Retire rows first: a live operator-day must be free before its replacement is written.
    let (retired, live): (Vec<_>, Vec<_>) =
        changes.daily.iter().partition(|(_, daily)| daily.missing);
    for (daily_ref, daily) in retired.into_iter().chain(live) {
        upsert_daily(connection, *daily_ref, daily, batch_ref).await?;
    }
    Ok(())
}

async fn upsert_account(
    connection: &mut PgConnection,
    account_ref: Uuid,
    account: &PlannedAccount,
    batch_ref: Uuid,
    domain_ref: Uuid,
) -> Result<(), OpsImportError> {
    // The business domain is Linggan's own decision: set on first import, never overwritten.
    sqlx::query(
        "INSERT INTO linggan_ops_account AS a ( \
            account_ref, source_system, source_record_id, domain_ref, ledger_state, platform, \
            account_name, handle, status, is_retained, operator_name, responsible_user_source_id, \
            follower_count, profile_url, profile_link_state, platform_user_id, device_code, \
            ip_location, notes, review_status, created_by_user_source_id, source_created_at, \
            source_updated_at, first_batch_ref, last_changed_batch_ref) \
         VALUES ($1,'content_workbench',$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18, \
                 $19,$20,$21::timestamptz,$22::timestamptz,$23,$23) \
         ON CONFLICT (source_system, source_record_id) DO UPDATE SET \
            ledger_state = EXCLUDED.ledger_state, platform = EXCLUDED.platform, \
            account_name = EXCLUDED.account_name, handle = EXCLUDED.handle, status = EXCLUDED.status, \
            is_retained = EXCLUDED.is_retained, operator_name = EXCLUDED.operator_name, \
            responsible_user_source_id = EXCLUDED.responsible_user_source_id, \
            follower_count = EXCLUDED.follower_count, profile_url = EXCLUDED.profile_url, \
            profile_link_state = EXCLUDED.profile_link_state, platform_user_id = EXCLUDED.platform_user_id, \
            device_code = EXCLUDED.device_code, ip_location = EXCLUDED.ip_location, notes = EXCLUDED.notes, \
            review_status = EXCLUDED.review_status, \
            created_by_user_source_id = EXCLUDED.created_by_user_source_id, \
            source_created_at = EXCLUDED.source_created_at, source_updated_at = EXCLUDED.source_updated_at, \
            last_changed_batch_ref = EXCLUDED.last_changed_batch_ref",
    )
    .bind(account_ref)
    .bind(&account.source_record_id)
    .bind(domain_ref)
    .bind(account.ledger_state.as_str())
    .bind(account.platform.as_str())
    .bind(&account.account_name)
    .bind(&account.handle)
    .bind(account.status.map(|status| status.as_str()))
    .bind(account.is_retained)
    .bind(&account.operator_name)
    .bind(&account.responsible_user_source_id)
    .bind(account.follower_count)
    .bind(&account.profile_url)
    .bind(account.profile_link_state.as_str())
    .bind(&account.platform_user_id)
    .bind(&account.device_code)
    .bind(&account.ip_location)
    .bind(&account.notes)
    .bind(account.review_status.map(|status| status.as_str()))
    .bind(&account.created_by_user_source_id)
    .bind(&account.source_created_at)
    .bind(&account.source_updated_at)
    .bind(batch_ref)
    .execute(&mut *connection)
    .await?;
    Ok(())
}

async fn upsert_publication(
    connection: &mut PgConnection,
    publication_ref: Uuid,
    publication: &PlannedPublication,
    (account_ref, merged_into_ref): (Option<Uuid>, Option<Uuid>),
    batch_ref: Uuid,
    field_adoption_ready: bool,
) -> Result<(), OpsImportError> {
    let missing_since = publication.missing.then_some(batch_ref);
    sqlx::query(
        "INSERT INTO linggan_ops_publication AS p ( \
            publication_ref, source_system, source_record_id, account_ref, platform, publication_url, \
            canonical_url, content_id_state, platform_content_id, published_at, published_date_state, \
            title, body, publisher_name, responsible_user_source_id, created_by_user_source_id, \
            like_count, collect_count, comment_count, cover_click_rate, entry_source, review_status, \
            legacy_effective_publication, legacy_data_synced_at, legacy_data_sync_status, \
            merged_into_ref, missing_since_batch_ref, source_created_at, source_updated_at, \
            first_batch_ref, last_changed_batch_ref) \
         VALUES ($1,'content_workbench',$2,$3,$4,$5,$6,$7,$8,$9::timestamptz,$10,$11,$12,$13,$14,$15, \
                 $16,$17,$18,$19,$20,$21,$22,$23::timestamptz,$24,$25,$26,$27::timestamptz, \
                 $28::timestamptz,$29,$29) \
         ON CONFLICT (source_system, source_record_id) DO UPDATE SET \
            account_ref = EXCLUDED.account_ref, platform = EXCLUDED.platform, \
            publication_url = EXCLUDED.publication_url, canonical_url = EXCLUDED.canonical_url, \
            content_id_state = EXCLUDED.content_id_state, platform_content_id = EXCLUDED.platform_content_id, \
            published_at = EXCLUDED.published_at, published_date_state = EXCLUDED.published_date_state, \
            title = EXCLUDED.title, body = EXCLUDED.body, publisher_name = EXCLUDED.publisher_name, \
            responsible_user_source_id = EXCLUDED.responsible_user_source_id, \
            created_by_user_source_id = EXCLUDED.created_by_user_source_id, \
            like_count = EXCLUDED.like_count, collect_count = EXCLUDED.collect_count, \
            comment_count = EXCLUDED.comment_count, cover_click_rate = EXCLUDED.cover_click_rate, \
            entry_source = EXCLUDED.entry_source, review_status = EXCLUDED.review_status, \
            legacy_effective_publication = EXCLUDED.legacy_effective_publication, \
            legacy_data_synced_at = EXCLUDED.legacy_data_synced_at, \
            legacy_data_sync_status = EXCLUDED.legacy_data_sync_status, \
            merged_into_ref = EXCLUDED.merged_into_ref, missing_since_batch_ref = EXCLUDED.missing_since_batch_ref, \
            source_created_at = EXCLUDED.source_created_at, source_updated_at = EXCLUDED.source_updated_at, \
            last_changed_batch_ref = EXCLUDED.last_changed_batch_ref",
    )
    .bind(publication_ref)
    .bind(&publication.source_record_id)
    .bind(account_ref)
    .bind(publication.platform.as_str())
    .bind(&publication.publication_url)
    .bind(&publication.canonical_url)
    .bind(publication.content_id_state.as_str())
    .bind(&publication.platform_content_id)
    .bind(&publication.published_at)
    .bind(publication.published_date_state.as_str())
    .bind(&publication.title)
    .bind(&publication.body)
    .bind(&publication.publisher_name)
    .bind(&publication.responsible_user_source_id)
    .bind(&publication.created_by_user_source_id)
    .bind(publication.like_count)
    .bind(publication.collect_count)
    .bind(publication.comment_count)
    .bind(publication.cover_click_rate)
    .bind(publication.entry_source.as_str())
    .bind(publication.review_status.as_str())
    .bind(publication.legacy_effective_publication)
    .bind(&publication.legacy_data_synced_at)
    .bind(publication.legacy_data_sync_status.as_str())
    .bind(merged_into_ref)
    .bind(missing_since)
    .bind(&publication.source_created_at)
    .bind(&publication.source_updated_at)
    .bind(batch_ref)
    .execute(&mut *connection)
    .await?;
    if field_adoption_ready {
        append_imported_publication_field_adoptions(connection, publication_ref, publication, batch_ref)
            .await?;
    }
    Ok(())
}

/// Imports retain their own source timestamp and batch identity in the field-adoption stream.
/// This is deliberately separate from the mutable registration projection: a blank in a later
/// export is an explicit `not_provided` current decision, never a hidden fallback to old text.
async fn append_imported_publication_field_adoptions(
    connection: &mut PgConnection,
    publication_ref: Uuid,
    publication: &PlannedPublication,
    batch_ref: Uuid,
) -> Result<(), OpsImportError> {
    sqlx::query(
        "INSERT INTO linggan_ops_publication_field_adoption \
         (adoption_ref,publication_ref,field_name,disposition,source_kind,source_batch_ref, \
          value_text,value_count,value_instant,observed_at) \
         SELECT gen_random_uuid(),$1,candidate.field_name,candidate.disposition, \
                'workbench_import',$2,candidate.value_text,candidate.value_count, \
                candidate.value_instant,$3 \
           FROM (VALUES \
             ('title'::text,CASE WHEN $4::text IS NULL THEN 'not_provided' ELSE 'adopted' END, \
                $4::text,NULL::bigint,NULL::timestamptz), \
             ('body'::text,CASE WHEN $5::text IS NULL THEN 'not_provided' ELSE 'adopted' END, \
                $5::text,NULL::bigint,NULL::timestamptz), \
             ('like_count'::text,CASE WHEN $6::bigint IS NULL THEN 'not_provided' ELSE 'adopted' END, \
                NULL::text,$6::bigint,NULL::timestamptz), \
             ('collect_count'::text,CASE WHEN $7::bigint IS NULL THEN 'not_provided' ELSE 'adopted' END, \
                NULL::text,$7::bigint,NULL::timestamptz), \
             ('comment_count'::text,CASE WHEN $8::bigint IS NULL THEN 'not_provided' ELSE 'adopted' END, \
                NULL::text,$8::bigint,NULL::timestamptz), \
             ('published_at'::text,'adopted',NULL::text,NULL::bigint,$9::timestamptz) \
           ) AS candidate(field_name,disposition,value_text,value_count,value_instant) \
         ON CONFLICT (source_batch_ref,publication_ref,field_name) DO NOTHING",
    )
    .bind(publication_ref)
    .bind(batch_ref)
    .bind(&publication.source_updated_at)
    .bind(&publication.title)
    .bind(&publication.body)
    .bind(publication.like_count)
    .bind(publication.collect_count)
    .bind(publication.comment_count)
    .bind(&publication.published_at)
    .execute(&mut *connection)
    .await?;
    Ok(())
}

async fn upsert_daily(
    connection: &mut PgConnection,
    daily_ref: Uuid,
    daily: &PlannedDaily,
    batch_ref: Uuid,
) -> Result<(), OpsImportError> {
    sqlx::query(
        "INSERT INTO linggan_ops_daily_report AS d ( \
            daily_ref, source_system, source_record_id, operator_name, business_date, \
            links_opened_count, followers_gained, responsible_user_source_id, created_by_user_source_id, \
            review_status, notes, missing_since_batch_ref, source_created_at, source_updated_at, \
            first_batch_ref, last_changed_batch_ref) \
         VALUES ($1,'content_workbench',$2,$3,$4::date,$5,$6,$7,$8,$9,$10,$11,$12::timestamptz, \
                 $13::timestamptz,$14,$14) \
         ON CONFLICT (source_system, source_record_id) DO UPDATE SET \
            operator_name = EXCLUDED.operator_name, business_date = EXCLUDED.business_date, \
            links_opened_count = EXCLUDED.links_opened_count, followers_gained = EXCLUDED.followers_gained, \
            responsible_user_source_id = EXCLUDED.responsible_user_source_id, \
            created_by_user_source_id = EXCLUDED.created_by_user_source_id, \
            review_status = EXCLUDED.review_status, notes = EXCLUDED.notes, \
            missing_since_batch_ref = EXCLUDED.missing_since_batch_ref, \
            source_created_at = EXCLUDED.source_created_at, source_updated_at = EXCLUDED.source_updated_at, \
            last_changed_batch_ref = EXCLUDED.last_changed_batch_ref",
    )
    .bind(daily_ref)
    .bind(&daily.source_record_id)
    .bind(&daily.operator_name)
    .bind(daily.business_date.to_string())
    .bind(daily.links_opened_count)
    .bind(daily.followers_gained)
    .bind(&daily.responsible_user_source_id)
    .bind(&daily.created_by_user_source_id)
    .bind(daily.review_status.as_str())
    .bind(&daily.notes)
    .bind(daily.missing.then_some(batch_ref))
    .bind(&daily.source_created_at)
    .bind(&daily.source_updated_at)
    .bind(batch_ref)
    .execute(&mut *connection)
    .await?;
    Ok(())
}
