-- COLLECTION-001 · The four-stage acquisition chain
--
-- Request → Authorization → Admission → Work Order, per capture-control-contract §2.
-- Each stage is a separate table because the contract's core rule is that no earlier stage
-- may be reported as a later one succeeding (INV-36). Collapsing any two of them into one
-- row would make that rule unenforceable.
--
-- Nothing here reaches a platform. A Work Order row is an instruction that has been written
-- down, not work that has run.

-- Stage 2 · Authorization: a person permits a class of acquisition inside stated bounds.
--
-- Deliberately covers a *class* of targets rather than one target: asking Mog to approve
-- every single creator would make the control a rubber stamp, which is the failure mode the
-- contract warns about. The bounds are what keep a class grant honest.
CREATE TABLE collection_acquisition_authorization (
    authorization_ref uuid PRIMARY KEY,

    platform text NOT NULL CHECK (platform IN ('xhs')),
    target_kind text NOT NULL CHECK (target_kind IN ('creator', 'keyword')),

    -- Which observation lane this grant covers. Lanes have different risk and cost, so one
    -- grant never covers all of them (contract §4).
    lane text NOT NULL CHECK (lane IN ('deep_archive', 'patrol')),

    -- The bounds. Null means "not bounded on this axis", which is legal but must be a
    -- deliberate choice — it is never a default filled in on the person's behalf.
    max_targets integer CHECK (max_targets IS NULL OR max_targets > 0),
    max_works_per_target integer CHECK (max_works_per_target IS NULL OR max_works_per_target > 0),

    -- Why this was granted. Required: a grant without a stated purpose cannot later be
    -- checked against what it is being used for.
    purpose text NOT NULL CHECK (length(btrim(purpose)) > 0),

    -- Only a person grants. Contract §2: "Acquisition Authorization | 人允许在何种目的、
    -- 用途、数据和资源范围内研究 | Mog 或明确授权负责人". An agent may request, never grant.
    granted_by text NOT NULL CHECK (granted_by = 'person'),
    granted_at timestamptz NOT NULL DEFAULT scope_001_now(),

    -- Expiry is required, not optional. An unbounded-in-time grant is indistinguishable from
    -- no control at all, and the contract requires re-checking before every execution.
    expires_at timestamptz NOT NULL,

    revoked_at timestamptz,
    revoke_reason text,

    CHECK (expires_at > granted_at),
    CHECK ((revoked_at IS NULL) = (revoke_reason IS NULL))
);

CREATE INDEX collection_acquisition_authorization_active_idx
    ON collection_acquisition_authorization (platform, target_kind, lane, expires_at DESC)
    WHERE revoked_at IS NULL;

-- Stage 1 · Request: why a new platform access is being asked for.
--
-- A request is not a permission, not a queue slot, and not work. An agent is allowed to
-- create one (contract CCC-01: "只能创建 Evidence Need / Proposal").
CREATE TABLE collection_acquisition_request (
    request_ref uuid PRIMARY KEY,
    target_ref uuid NOT NULL REFERENCES collection_observation_target(target_ref),

    lane text NOT NULL CHECK (lane IN ('deep_archive', 'patrol')),

    -- What this request is for. Required for the same reason a grant needs one: without it
    -- the Admission stage cannot answer "is this still inside the approved purpose".
    purpose text NOT NULL CHECK (length(btrim(purpose)) > 0),

    -- Agents may ask. Only people may grant (see the authorization table).
    requested_by text NOT NULL CHECK (requested_by IN ('person', 'agent')),
    requested_at timestamptz NOT NULL DEFAULT scope_001_now()
);

CREATE INDEX collection_acquisition_request_target_idx
    ON collection_acquisition_request (target_ref, requested_at DESC);

-- Stage 3 · Admission: the server decides whether this access is still worth doing.
--
-- The outcome set is deliberately wider than accept/reject. Contract §2.1: "控制层应拒绝、
-- 等待、合并、降级为建议或标记 DECISION_REQUIRED". An admission stage with only two
-- outcomes cannot express "already satisfied" or "we cannot answer this honestly yet",
-- and would push both of those into a false accept.
CREATE TABLE collection_admission_decision (
    decision_ref uuid PRIMARY KEY,

    -- One decision per request. A second decision would mean the first was silently revised.
    request_ref uuid NOT NULL UNIQUE REFERENCES collection_acquisition_request(request_ref),

    outcome text NOT NULL CHECK (outcome IN (
        'admitted',           -- proceed; a Work Order is created
        'reuse',              -- existing evidence already satisfies the need
        'merge',              -- an in-flight Work already covers this
        'defer',              -- not now; may be re-decided later
        'refuse',             -- will not do this
        'decision_required'   -- cannot be answered honestly without a person deciding
    )),

    -- Which of the contract's six questions could not be answered, when that is the reason.
    -- 1 复用 / 2 差额 / 3 时间 / 4 边界 / 5 资源与风险 / 6 可解释性.
    unanswered_question smallint CHECK (unanswered_question BETWEEN 1 AND 6),

    reason_code text NOT NULL CHECK (length(btrim(reason_code)) > 0),
    reason text,

    -- Set only when admitted: which grant authorised it.
    authorization_ref uuid REFERENCES collection_acquisition_authorization(authorization_ref),

    decided_at timestamptz NOT NULL DEFAULT scope_001_now(),

    -- An admitted decision must name the grant it relied on. Anything else must not.
    CHECK ((outcome = 'admitted') = (authorization_ref IS NOT NULL))
);

CREATE INDEX collection_admission_decision_outcome_idx
    ON collection_admission_decision (outcome, decided_at DESC);

-- Stage 4 · Work Order: one bounded instruction for a producer.
--
-- Contract §2: "只负责『这一小步做什么』，不拥有研究意义，也不允许插件自行扩大到下一阶段".
-- A row here is a written instruction, not execution: no attempt, no lease, no result.
CREATE TABLE collection_work_order (
    work_order_ref uuid PRIMARY KEY,

    -- Every Work Order traces to the admission that permitted it. No admission, no order.
    decision_ref uuid NOT NULL UNIQUE REFERENCES collection_admission_decision(decision_ref),
    target_ref uuid NOT NULL REFERENCES collection_observation_target(target_ref),

    lane text NOT NULL CHECK (lane IN ('deep_archive', 'patrol')),

    -- The bound this order may not exceed. Distinct from "how many exist": the contract is
    -- explicit that a quota's shortfall is not a set of real objects (§3.1).
    max_works integer NOT NULL CHECK (max_works > 0),

    -- When to stop. Required, because an order without stop conditions is unbounded work.
    stop_conditions jsonb NOT NULL,

    created_at timestamptz NOT NULL DEFAULT scope_001_now()
);

CREATE INDEX collection_work_order_target_idx
    ON collection_work_order (target_ref, created_at DESC);
