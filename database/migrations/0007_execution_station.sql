-- COLLECTION-001 · 工位与插件安装
--
-- 分成两张表，因为它们的寿命根本不同：工位是人登记的、长期的、持有授权的对象；
-- 插件安装是一次性的、随时会被重装抹掉的运行体。
--
-- 这是从内容工作台的实测教训倒推出来的：那边只有一张 ExecutionStation，身份锚点是插件
-- 端 randomUUID() 生成、存在 chrome.storage.local 的 stationKey。该存储在卸载重装时被
-- 清空，于是每次重装都注册出一台新工位。2026-06-25 排查实测：注册 13 台，活跃 2 台，
-- 11 台僵尸，调度器仍在向鬼工位派活。让客户端决定自己是谁，身份就会蒸发。

-- 工位：一个长期的执行位置。由人登记，不由插件创建。
CREATE TABLE execution_station (
    station_ref uuid PRIMARY KEY,

    -- 人给的名字，用来在界面上认人（「MacBook Chrome」）。插件不能改它。
    display_name text NOT NULL CHECK (length(btrim(display_name)) > 0),

    -- 人登记的事实，不是插件上报的。插件重装不影响这些。
    registered_by text NOT NULL DEFAULT 'person' CHECK (registered_by = 'person'),
    registered_at timestamptz NOT NULL DEFAULT scope_001_now(),

    -- 每日笔记上限（Mog 定：200 篇/天）。属于工位而不属于安装——重装不该重置额度。
    daily_work_quota integer NOT NULL DEFAULT 200 CHECK (daily_work_quota > 0),

    -- 认领窗口：开着的时候，新安装可以自动绑到这个工位；关着就得人确认。
    --
    -- 必须能过期。开发期频繁重装时把窗口开一天，比每次重装点一次确认省事得多；
    -- 而永不过期的窗口在公网域名下就是一扇一直开着的门，跟没有认领控制没区别。
    claim_window_opens_at timestamptz,
    claim_window_expires_at timestamptz,

    retired_at timestamptz,
    retire_reason text,

    CHECK ((claim_window_opens_at IS NULL) = (claim_window_expires_at IS NULL)),
    CHECK (claim_window_expires_at IS NULL OR claim_window_expires_at > claim_window_opens_at),
    CHECK ((retired_at IS NULL) = (retire_reason IS NULL))
);

CREATE UNIQUE INDEX execution_station_name_idx
    ON execution_station (display_name) WHERE retired_at IS NULL;

-- 插件安装：一个正在跑的插件实例。自己注册，随时可被取代。
--
-- 重装产生一条新记录，而不是一台新工位。历史安装保留，因为「这台工位换过 12 次插件」
-- 是需要看得见的事实；内容工作台的问题正是这个事实以 11 台僵尸工位的形式呈现。
CREATE TABLE plugin_installation (
    installation_ref uuid PRIMARY KEY,

    -- 插件自报的身份。这里**不加唯一约束也不作为工位身份**：它随时会变，
    -- 变了只说明插件重装过，不说明这是另一台机器。
    install_key text NOT NULL CHECK (length(btrim(install_key)) > 0),

    -- 认领到哪台工位。为空表示待认领——插件在，但人还没说它是谁。
    station_ref uuid REFERENCES execution_station(station_ref),

    claim_kind text CHECK (claim_kind IN ('claim_window', 'person')),
    claimed_at timestamptz,

    -- 插件自报的兼容性事实（合同 §5：插件只报告自己的兼容性，不能改变配额或范围）。
    plugin_version text NOT NULL CHECK (length(btrim(plugin_version)) > 0),
    browser_label text,
    capabilities jsonb NOT NULL DEFAULT '[]'::jsonb,

    first_seen_at timestamptz NOT NULL DEFAULT scope_001_now(),
    last_seen_at timestamptz NOT NULL DEFAULT scope_001_now(),

    -- 被同工位的新安装取代。取代是记录下来的事实，不是删除——否则「插件重装了几次」
    -- 这个问题以后没人答得上来。
    superseded_at timestamptz,
    -- 延迟到提交时校验：取代旧安装与插入新安装发生在同一个事务里，而 UPDATE 必须先跑
    -- （否则两条在岗记录会同时落进下面那个唯一索引）。立即校验的外键会在此刻指向一行
    -- 还没插入的数据而报错。
    superseded_by uuid REFERENCES plugin_installation(installation_ref)
        DEFERRABLE INITIALLY DEFERRED,

    CHECK ((station_ref IS NULL) = (claimed_at IS NULL)),
    CHECK ((claimed_at IS NULL) = (claim_kind IS NULL)),
    CHECK ((superseded_at IS NULL) = (superseded_by IS NULL))
);

-- 一台工位同时只能有一个在岗安装。两个同时在岗就会被派两份活。
CREATE UNIQUE INDEX plugin_installation_active_station_idx
    ON plugin_installation (station_ref)
    WHERE station_ref IS NOT NULL AND superseded_at IS NULL;

CREATE INDEX plugin_installation_pending_idx
    ON plugin_installation (first_seen_at DESC) WHERE station_ref IS NULL;

CREATE INDEX plugin_installation_install_key_idx
    ON plugin_installation (install_key, first_seen_at DESC);
