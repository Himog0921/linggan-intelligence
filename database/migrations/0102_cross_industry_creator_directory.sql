-- COLLECTION-CROSS-INDUSTRY-CREATOR-ARCHIVE-001 · 跨行业创作者作品目录
--
-- `cross_industry_sample_observation` 表达「某个关键词的某个榜看到了这篇」，因此强制要求
-- keyword + sort_order。创作者主页没有这套口径；把主页发现塞进那张表只能伪造关键词或排序。
-- 本表只表达另一件真实发生的事：某个已接纳 profile_discovery Package 的第 N 条记录，
-- 在某个创作者目标的主页上发现了这篇跨行业样本。目标仍由 Package → Task → Lease →
-- WorkOrder 血缘确定，不复制第二个可漂移的 target_ref。

CREATE TABLE cross_industry_creator_sample_observation (
    observation_ref uuid PRIMARY KEY,
    sample_ref uuid NOT NULL,
    domain_ref uuid NOT NULL,
    package_ref uuid NOT NULL,
    record_ordinal integer NOT NULL,
    -- 插件原样报告的主页结果流位置，从 0 开始；缺失保持 NULL。
    discovery_order integer,
    observed_at text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    UNIQUE (package_ref, record_ordinal),
    UNIQUE (sample_ref, package_ref),
    FOREIGN KEY (sample_ref, domain_ref)
        REFERENCES cross_industry_sample(sample_ref, domain_ref),
    -- 只有被接纳的那条 record 才能形成目录关系。整包或单条被隔离时没有可引用行。
    FOREIGN KEY (package_ref, record_ordinal)
        REFERENCES linggan_runtime_record_disposition(package_ref, record_ordinal),
    CHECK (record_ordinal >= 0),
    CHECK (discovery_order IS NULL OR discovery_order >= 0)
);

CREATE INDEX cross_industry_creator_sample_observation_sample_idx
    ON cross_industry_creator_sample_observation (sample_ref, created_at DESC);

CREATE TRIGGER cross_industry_creator_sample_observation_is_append_only
    BEFORE UPDATE OR DELETE ON cross_industry_creator_sample_observation
    FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();

COMMENT ON TABLE cross_industry_creator_sample_observation IS
  '跨行业创作者主页的追加式作品目录关系；目标由 package 的 WorkOrder 血缘确定，不伪造关键词榜单口径';

-- 从不可变 Package payload 与已接纳 record disposition 重建历史关系。这里不是从标题、作者名
-- 或 sample.target_ref 猜归属：每一行都同时钉在原 Package/record、它的 WorkOrder target 与
-- 同领域同平台同 content_external_id 的样本上。重复应用由两条 UNIQUE 收敛。
INSERT INTO cross_industry_creator_sample_observation
    (observation_ref,sample_ref,domain_ref,package_ref,record_ordinal,
     discovery_order,observed_at)
SELECT gen_random_uuid(),sample.sample_ref,sample.domain_ref,package.package_ref,
       (record.ordinality - 1)::integer,
       CASE
         WHEN jsonb_typeof(record.value #> '{payload,_discoveryOrder}')='number'
           THEN (record.value #>> '{payload,_discoveryOrder}')::integer
         ELSE NULL
       END,
       package.observed_at
FROM linggan_runtime_capture_package package
JOIN linggan_runtime_submission_receipt receipt USING(package_ref)
JOIN collection_work_order_lease_task lease_task ON lease_task.task_id=package.task_id
JOIN collection_work_order_lease lease USING(lease_ref)
JOIN collection_work_order work_order USING(work_order_ref)
JOIN collection_observation_target target USING(target_ref)
JOIN observation_domain domain USING(domain_ref)
CROSS JOIN LATERAL jsonb_array_elements(
    CASE WHEN jsonb_typeof(package.payload->'records')='array'
         THEN package.payload->'records' ELSE '[]'::jsonb END
) WITH ORDINALITY AS record(value, ordinality)
JOIN linggan_runtime_record_disposition disposition
  ON disposition.package_ref=package.package_ref
 AND disposition.record_ordinal=(record.ordinality - 1)::integer
 AND disposition.disposition='accepted_for_library_discovery'
JOIN cross_industry_sample sample
  ON sample.domain_ref=target.domain_ref
 AND sample.platform=package.platform
 AND sample.content_external_id=NULLIF(
       btrim(record.value #>> '{sourceObject,externalId}'),'')
WHERE package.package_kind='profile_discovery'
  AND target.target_kind='creator'
  AND domain.is_own_domain=false
  AND receipt.execution_effect='COMPLETED_LIVE_STEP'
  AND receipt.material_admission='ACCEPTED'
  -- `sourceObject.externalId` 是合同里的规范身份。历史版本虽已接纳 payload 身份冲突的
  -- record，迁移也不能把代表 X 的 record 重建成样本 Y 的关系。
  AND NULLIF(btrim(record.value #>> '{sourceObject,externalId}'),'') IS NOT NULL
  AND (NOT (record.value->'payload' ? 'noteId')
       OR NULLIF(btrim(record.value #>> '{payload,noteId}'),'')
          =NULLIF(btrim(record.value #>> '{sourceObject,externalId}'),''))
  AND (NOT (record.value->'payload' ? 'contentId')
       OR NULLIF(btrim(record.value #>> '{payload,contentId}'),'')
          =NULLIF(btrim(record.value #>> '{sourceObject,externalId}'),''))
  AND (NOT (record.value->'payload' ? 'id')
       OR NULLIF(btrim(record.value #>> '{payload,id}'),'')
          =NULLIF(btrim(record.value #>> '{sourceObject,externalId}'),''))
ON CONFLICT DO NOTHING;

-- 旧调度器把「跨行业目录读不到」解释成「目录没有缺口」，随后把根工单收口。只恢复满足
-- 精确事故形状且仍有真实详情缺口的根；已经补齐的根、没有主页 Package 的根和本领域根不动。
UPDATE collection_work_order root
SET stop_conditions = jsonb_set(
        jsonb_set(
          jsonb_set(root.stop_conditions #- '{progressiveArchive,completedAt}',
                    '{progressiveArchive,status}','"active"'::jsonb,true),
          '{progressiveArchive,recoveryReason}',
          to_jsonb('cross_industry_creator_directory_reconstructed'::text),true),
        '{progressiveArchive,recoveredAt}',to_jsonb(scope_001_now()),true)
FROM collection_observation_target target
JOIN observation_domain domain USING(domain_ref)
WHERE root.target_ref=target.target_ref
  AND target.target_kind='creator'
  AND domain.is_own_domain=false
  AND root.lane='deep_archive'
  AND root.stop_conditions #>> '{progressiveArchive,version}'='1'
  AND root.stop_conditions #>> '{progressiveArchive,rootWorkOrderRef}'=root.work_order_ref::text
  AND root.stop_conditions #>> '{progressiveArchive,status}'='completed'
  AND EXISTS (
      SELECT 1 FROM cross_industry_creator_sample_observation seen
      WHERE seen.package_ref=(
        SELECT package.package_ref
        FROM collection_work_order_lease lease
        JOIN collection_work_order_lease_task lease_task USING(lease_ref)
        JOIN linggan_runtime_task task ON task.task_id=lease_task.task_id
        JOIN linggan_runtime_capture_package package ON package.task_id=task.task_id
        JOIN linggan_runtime_submission_receipt receipt USING(package_ref)
        CROSS JOIN LATERAL jsonb_array_elements(
          CASE WHEN jsonb_typeof(package.coverage->'layers')='array'
               THEN package.coverage->'layers' ELSE '[]'::jsonb END
        ) layer
        WHERE lease.work_order_ref=root.work_order_ref
          AND package.package_kind='profile_discovery'
          AND receipt.execution_effect='COMPLETED_LIVE_STEP'
          AND receipt.material_admission='ACCEPTED'
          AND layer->>'capability'='profile_discovery'
          AND COALESCE((layer->>'failed')::integer,0)=0
          AND COALESCE((layer->>'notAttempted')::integer,0)=0
          AND (
            checkpoint #>> '{surfaceReceipt,stopReason}'='bottom_confirmed'
            OR (checkpoint #>> '{surfaceReceipt,stopReason}'='target_reached'
                AND COALESCE((layer->>'acquired')::integer,0)
                    >= COALESCE((task_spec->>'expectedCount')::integer,
                                (task_spec->>'maximumQuota')::integer,2147483647))
          )
          AND COALESCE((layer->>'observed')::integer,0)>0
          AND COALESCE((layer->>'attempted')::integer,0)>0
          AND COALESCE((layer->>'acquired')::integer,0)>0
          AND COALESCE((task_spec->>'maximumQuota')::integer,-1)=200
          AND NOT EXISTS (
            SELECT 1 FROM linggan_runtime_record_disposition disposition
            WHERE disposition.package_ref=package.package_ref
              AND disposition.disposition='quarantined'
          )
          AND (SELECT count(*) FROM linggan_runtime_record_disposition disposition
               WHERE disposition.package_ref=package.package_ref
                 AND disposition.disposition='accepted_for_library_discovery')
              =COALESCE((layer->>'acquired')::integer,-1)
        ORDER BY package.accepted_at DESC,package.package_ref DESC
        LIMIT 1
      )
        AND NOT EXISTS (
            SELECT 1 FROM cross_industry_sample_detail detail
            WHERE detail.sample_ref=seen.sample_ref
        )
  );
