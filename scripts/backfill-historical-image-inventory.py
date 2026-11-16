"""Check or apply a reviewed XHS image inventory plan to an explicitly selected PostgreSQL database."""

import argparse
import json
import os
import re
import subprocess
from pathlib import Path
from uuid import UUID


decoder = json.JSONDecoder()

allowed = {
    'xhs.ssr_initial_state': 'document.ssr.__INITIAL_STATE__.note.noteDetailMap',
    'xhs.ssr_initial_state_partial': 'document.ssr.__INITIAL_STATE__.note.noteDetailMap',
    '__INITIAL_STATE__': '__INITIAL_STATE__.noteMap',
    'xhs.initial_state_partial': '__INITIAL_STATE__.noteMap',
}


def parse_prefix(raw):
    if not isinstance(raw, str):
        return {}, 'invalid'
    raw = raw.lstrip()
    if not raw.startswith('{'):
        return {}, 'invalid'
    try:
        out, end = decoder.raw_decode(raw)
        if isinstance(out, dict) and not raw[end:].strip():
            return out, 'whole'
        return {}, 'invalid'
    except json.JSONDecodeError:
        if not raw.endswith('...') or len(raw.encode('utf-16-le', errors='surrogatepass')) != 12006:
            return {}, 'invalid'
    raw = raw[:-3]
    out = {}
    pos = 1
    try:
        while pos < len(raw):
            while pos < len(raw) and raw[pos].isspace():
                pos += 1
            key, pos = decoder.raw_decode(raw, pos)
            if not isinstance(key, str) or key in out:
                return {}, 'invalid'
            while pos < len(raw) and raw[pos].isspace():
                pos += 1
            if raw[pos] != ':':
                return {}, 'invalid'
            pos += 1
            while pos < len(raw) and raw[pos].isspace():
                pos += 1
            value, pos = decoder.raw_decode(raw, pos)
            out[key] = value
            while pos < len(raw) and raw[pos].isspace():
                pos += 1
            if pos == len(raw):
                break
            if raw[pos] != ',':
                return {}, 'invalid'
            pos += 1
    except (IndexError, json.JSONDecodeError):
        pass
    return out, 'truncated'


def url_values(record):
    urls = set()
    fields = ('url', 'urlDefault', 'urlPre', 'url_default', 'url_pre', 'originUrl', 'urlOriginal', 'url_original')

    def add(item):
        for key in fields:
            value = item.get(key)
            if isinstance(value, str) and value.startswith(('http://', 'https://')):
                urls.add(value)

    add(record)
    for field in ('infoList', 'info_list'):
        for image in record.get(field) or []:
            if isinstance(image, dict):
                add(image)
    return urls


def platform_shape(row):
    if not isinstance(row, dict):
        return False
    dimensions = all(isinstance(row.get(key), int) and row[key] > 0 for key in ('width', 'height'))
    file_id = any(isinstance(row.get(key), str) and row[key] for key in ('fileId', 'file_id'))
    variants = any(
        isinstance(row.get(key), list)
        and any(isinstance(image, dict) and url_values(image) for image in row[key])
        for key in ('infoList', 'info_list')
    )
    return (file_id or dimensions) and variants


def detail_list(entry, external_id):
    payload = entry.get('payload') or {}
    identity = entry.get('sourceObject') or {}
    if identity.get('externalId') != external_id or identity.get('type') != 'content' or identity.get('platform') != 'xhs':
        return None
    if (
        payload.get('noteId') != external_id
        or payload.get('dataSource') not in allowed
        or payload.get('rawSource') != allowed[payload['dataSource']]
        or payload.get('type') == 'video'
    ):
        return None
    parsed, status = parse_prefix(payload.get('rawPayload'))
    if any(isinstance(parsed.get(key), str) and parsed.get(key) != external_id for key in ('noteId', 'id')):
        return None
    sequence = parsed.get('imageList')
    if not isinstance(sequence, list) or not sequence or any(not isinstance(image, dict) for image in sequence):
        return None
    contains_platform_id = parsed.get('noteId') == external_id or parsed.get('id') == external_id
    return sequence, status, contains_platform_id


def proof_row(candidate):
    count = candidate["expectedPages"]
    if type(count) is not int or not 1 <= count <= 100:
        raise ValueError("invalid page count")
    row = {
        "work": str(UUID(candidate["workRef"])),
        "detail": str(UUID(candidate["detailMaterialRef"])),
        "package": str(UUID(candidate["detailPackageRef"])),
        "excerpt_hash": candidate["sourceExcerptSha256"],
    }
    for key, value in (
        ("origins", candidate["observationRefs"]),
        ("files", candidate["materializationRefs"]),
    ):
        row[key] = [str(UUID(ref)) for ref in value]
        if len(row[key]) != count or len(set(row[key])) != count:
            raise ValueError("non-unique or incomplete page proof")
    if not isinstance(candidate["imageHashes"], list):
        raise ValueError("image hashes must be an array")
    row["hashes"] = candidate["imageHashes"]
    if len(row["hashes"]) != count or not all(
        re.fullmatch(r"[0-9a-f]{64}", value)
        for value in [row["excerpt_hash"], *row["hashes"]]
    ):
        raise ValueError("invalid source hash")
    return row


VALIDATE = """
CREATE TEMP TABLE checked_inventory ON COMMIT DROP AS
WITH plan AS (
    SELECT * FROM jsonb_to_recordset(PLAN_JSON) AS r(
        work uuid, detail uuid, package uuid, excerpt_hash text,
        origins uuid[], files uuid[], hashes text[])
), source AS (
    SELECT plan.*, d.observed_at, d.created_at,
        p.payload->'records'->d.record_ordinal->'payload' AS payload
    FROM plan
    JOIN linggan_material_content_detail d ON d.material_ref=plan.detail
      AND d.content_public_ref=plan.work AND d.package_ref=plan.package
    JOIN linggan_runtime_capture_package p ON p.package_ref=d.package_ref
    JOIN linggan_runtime_record_disposition r ON r.package_ref=d.package_ref
      AND r.record_ordinal=d.record_ordinal
    WHERE p.platform='xhs' AND p.package_kind='content_detail'
      AND r.disposition='accepted_for_library_content'
), current_origin AS (
    SELECT DISTINCT ON(o.work,o.slot_key) o.* FROM (
        SELECT plan.work, m.* FROM plan
        JOIN linggan_material_media_origin m ON m.content_public_ref=plan.work
        WHERE m.purpose IN ('body_image','live_photo','video')
          AND linggan_dossier_slot_active(plan.work,m.slot_key,scope_001_now())
    ) o ORDER BY o.work,o.slot_key,o.source_generation DESC
)
SELECT s.work,s.detail,s.excerpt_hash,s.origins,s.files
FROM source s
WHERE linggan_opportunity_observed_at(s.observed_at) IS NOT NULL
  AND s.excerpt_hash=encode(sha256(convert_to(s.payload->>'rawPayload','UTF8')),'hex')
  AND NOT EXISTS (
    SELECT 1 FROM linggan_material_content_detail d
    JOIN linggan_runtime_capture_package p USING(package_ref)
    JOIN linggan_runtime_record_disposition r USING(package_ref,record_ordinal)
    WHERE d.content_public_ref=s.work AND r.disposition='accepted_for_library_content'
      AND (linggan_opportunity_observed_at(d.observed_at),d.created_at,d.package_ref,d.material_ref)
        > (linggan_opportunity_observed_at(s.observed_at),s.created_at,s.package,s.detail))
  AND NOT EXISTS (
    SELECT 1 FROM linggan_material_lane_observation lane
    JOIN linggan_runtime_capture_package p USING(package_ref)
    WHERE lane.content_public_ref=s.work AND lane.lane='media_slots'
      AND p.payload#>>'{coverage,target,imageInventory,state}'='complete'
      AND p.payload#>>'{coverage,target,imageInventory,source}'='platform_image_list'
      AND jsonb_typeof(p.payload#>'{coverage,target,imageInventory,expectedPages}')='number')
  AND NOT EXISTS (
    SELECT 1 FROM linggan_content_understanding_request req
    JOIN linggan_ai_batch_item_control item USING(item_ref)
    WHERE req.work_ref=s.work AND item.state IN ('pending','running'))
  AND (SELECT count(*) FROM current_origin o WHERE o.work=s.work)=cardinality(s.origins)
  AND cardinality(s.origins)=(
    SELECT count(*) FROM unnest(s.origins,s.files,s.hashes) WITH ORDINALITY page(origin,file,hash,number)
    JOIN current_origin o ON o.work=s.work AND o.observation_ref=page.origin
      AND o.purpose='body_image' AND o.producer_ordinal=page.number
      AND (o.display_ordinal IS NULL OR o.display_ordinal=page.number)
    JOIN LATERAL (
        SELECT m.materialization_ref,m.blob_sha256 FROM linggan_media_materialization m
        JOIN linggan_media_download_attempt a USING(download_attempt_ref)
        JOIN linggan_media_blob b ON b.sha256=m.blob_sha256
        WHERE a.media_observation_ref=o.observation_ref AND b.mime_type LIKE 'image/%'
        ORDER BY m.verified_at DESC,m.materialization_ref DESC LIMIT 1
    ) latest ON latest.materialization_ref=page.file AND latest.blob_sha256=page.hash
    WHERE NOT EXISTS (
      SELECT 1 FROM linggan_current_material_media_disposition d
      WHERE d.materialization_ref=page.file OR
        (d.state='WITHDRAWN_OR_RESTRICTED' AND (d.slot_key=o.slot_key OR d.blob_sha256=page.hash))));
"""

READ_PROOF = """
WITH plan AS (
    SELECT * FROM jsonb_to_recordset(PLAN_JSON) AS r(
        work uuid, detail uuid, package uuid, origins uuid[])
)
SELECT jsonb_build_object(
    'work', plan.work, 'externalId', work.content_external_id,
    'entry', p.payload->'records'->d.record_ordinal,
    'urls', (
        SELECT jsonb_agg(to_jsonb(page_urls.urls) ORDER BY page.number)
        FROM unnest(plan.origins) WITH ORDINALITY page(origin,number)
        CROSS JOIN LATERAL (
            SELECT ARRAY(
                SELECT external_uri FROM linggan_material_media_candidate
                WHERE observation_ref=page.origin
                UNION
                SELECT observed_external_uri FROM linggan_media_observation
                WHERE observation_ref=page.origin
            ) AS urls
        ) page_urls
    ))
FROM plan
JOIN linggan_material_content_detail d ON d.material_ref=plan.detail
  AND d.content_public_ref=plan.work AND d.package_ref=plan.package
JOIN linggan_runtime_capture_package p ON p.package_ref=d.package_ref
JOIN linggan_material_content work ON work.public_ref=d.content_public_ref;
"""

INSERT = """
DO $$ BEGIN
    IF EXISTS (
        SELECT 1 FROM checked_inventory proposed
        JOIN linggan_historical_image_inventory saved ON saved.detail_ref=proposed.detail
        WHERE (saved.source_excerpt_sha256,saved.observation_refs,saved.materialization_refs)
          IS DISTINCT FROM (proposed.excerpt_hash,proposed.origins,proposed.files)
    ) THEN RAISE EXCEPTION 'historical inventory proof differs from saved proof'; END IF;
END $$;
WITH accepted AS (
    INSERT INTO linggan_historical_image_inventory
        (detail_ref,source_excerpt_sha256,observation_refs,materialization_refs)
    SELECT detail,excerpt_hash,origins,files FROM checked_inventory
    ON CONFLICT (detail_ref) DO NOTHING
    RETURNING detail_ref
)
SELECT jsonb_build_object('inserted',count(*)) FROM accepted;
"""


def verify_proofs(psql, encoded_plan, rows):
    query = "BEGIN READ ONLY;\nSET LOCAL statement_timeout='60s';\n"
    query += READ_PROOF.replace("PLAN_JSON", encoded_plan) + "\nROLLBACK;\n"
    result = subprocess.run(psql, input=query, text=True, stdout=subprocess.PIPE, check=True)
    counts = {row["work"]: len(row["origins"]) for row in rows}
    verified = set()
    # The proof uses the actual accepted payload and original slot URLs. Only counts leave this function.
    for line in filter(None, result.stdout.split("\n")):
        proof = json.loads(line)
        parsed = detail_list(proof["entry"], proof["externalId"])
        if not parsed:
            continue
        images, _, _ = parsed
        if len(images) == len(proof["urls"]) == counts[proof["work"]] and all(
            platform_shape(image) and url_values(image).intersection(candidates)
            for image, candidates in zip(images, proof["urls"], strict=True)
        ):
            verified.add(proof["work"])
    if len(verified) != len(rows):
        raise ValueError(f"platform list proof validated {len(verified)} of {len(rows)} works")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("plan", type=Path)
    scope = parser.add_mutually_exclusive_group(required=True)
    scope.add_argument("--cohort", action="store_true")
    scope.add_argument("--work", action="append", type=UUID)
    scope.add_argument("--all-vetted", action="store_true")
    parser.add_argument("--apply", action="store_true")
    parser.add_argument("--psql", default="psql")
    args = parser.parse_args()
    if not os.environ.get("PGDATABASE"):
        parser.error("PGDATABASE must select the intended local database")
    candidates = json.loads(args.plan.read_text())["candidates"]
    rows = [
        proof_row(candidate)
        for candidate in candidates
        if args.all_vetted
        or (args.cohort and candidate["originalCohort"] is True)
        or (args.work and UUID(candidate["workRef"]) in args.work)
    ]
    if not rows or len({row["work"] for row in rows}) != len(rows):
        parser.error("empty scope or duplicate works")
    if args.work and len(rows) != len(set(args.work)):
        parser.error("one or more requested works have no verified plan")
    encoded = json.dumps(rows, separators=(",", ":")).encode().hex()
    plan_sql = f"convert_from(decode('{encoded}','hex'),'UTF8')::jsonb"
    psql = [args.psql, "-XqAt", "-v", "ON_ERROR_STOP=1"]
    verify_proofs(psql, plan_sql, rows)
    sql = (
        "BEGIN ISOLATION LEVEL REPEATABLE READ;\n"
        "SET LOCAL statement_timeout='60s';\n"
        + VALIDATE.replace("PLAN_JSON", plan_sql)
        + f"SELECT jsonb_build_object('selected',{len(rows)},'validated',count(*)) FROM checked_inventory;\n"
    )
    if args.apply:
        # Write only if the entire reviewed selection still passes, or roll back.
        sql += (
            f"DO $$ BEGIN IF (SELECT count(*) FROM checked_inventory)<>{len(rows)} "
            "THEN RAISE EXCEPTION 'historical inventory plan no longer valid'; END IF; END $$;\n"
            + INSERT
            + "COMMIT;\n"
        )
    else:
        sql += "ROLLBACK;\n"
    subprocess.run(psql, input=sql, text=True, check=True)


if __name__ == "__main__":
    main()
