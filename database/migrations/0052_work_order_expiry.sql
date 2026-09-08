-- WORK-ORDER-EXPIRY-001 · 工单保质期与派发回答
--
-- 两件事，同一个教训。
--
-- 2026-09-08 的停摆是这样的：一张 09-05 建的建档工单，它的授权当天就被人撤了换成新的。
-- 这张工单没有任何终点——它不会过期，重试冷却一过又变回「可领」，而且因为最老，永远
-- 排在队首。派发扫描每一轮都停在它身上，当天两张人工观察工单从建单到停摆一次都没有被
-- 看过。队列不空、工位在线、闸门全开，却什么都不发生。
--
-- 派发扫描的中断已经单独修掉了（一张发不出去就看下一张）。但那只是不让一张坏工单挡住
-- 别人；**它本身不该还在队列里**才是根本。一张昨天的工单回答的是昨天的问题，人可以再点
-- 一次，巡检下一轮会再来，建档可以重新发起——每条通道都有自己的再生方式，没有哪一张
-- 值得无限期等下去。
--
-- 第二件事：那一整天里，界面从头到尾没有一个地方说得出「为什么没动静」。工位每 5 分钟
-- 来问一次活，服务端每一次都给了明确的回答，而这个回答只发给插件，没有留下任何痕迹。
-- 人能看到的只有「等待 5 单」。**给出了回答却不留痕，等于没有回答。**

-- 一、工单保质期
--
-- 允许为空，空表示不设保质期：`legacy` 与已终结的历史工单本来就不在队列里，给它们编一个
-- 过期时刻只会让历史多出一个从未成立过的事实。
ALTER TABLE collection_work_order
    ADD COLUMN expires_at timestamptz;

-- 24 小时从「它可以开跑的那一刻」算起。新工单插入时 `scheduled_for` 就是 `scope_001_now()`，
-- 所以默认值与 `scheduled_for + 24 小时` 等价——写成默认值就不必去改每一处插入点，也就
-- 不会有哪一处插入点日后忘了带上它。
ALTER TABLE collection_work_order
    ALTER COLUMN expires_at SET DEFAULT scope_001_now() + interval '24 hours';

-- 既有队列补上保质期，按它们本来可以开跑的时刻算。
--
-- 这不是改写历史：保质期是这一版才有的规则，补的是「按这条规则，它们各自的保质期是
-- 什么时候」，而不是声称当时就有过一个。`leased` 也补上——它现在正在跑，租约结束回到
-- 队列后才会被这条规则看见，那时它已经该走了。
UPDATE collection_work_order
SET expires_at = COALESCE(scheduled_for, created_at) + interval '24 hours'
WHERE queue_state IN ('queued', 'leased');

-- 二、上一次派发回答
--
-- 只留最后一次，不做流水账：工位每 5 分钟问一次，一天 288 条，其中 287 条是同一句话。
-- 人要看的是「现在为什么不动」，不是「过去三天每五分钟分别为什么不动」。
--
-- 落在工位上而不是安装上：换一次插件安装不该让这一页失忆，而人在页面上看到的对象就是
-- 工位。
ALTER TABLE execution_station
    ADD COLUMN last_dispatch_answer_at timestamptz,
    -- 派发判定的机器取值（`dispatch` / `nothing_waiting` / `control_blocked` …）。
    ADD COLUMN last_dispatch_answer_code text,
    -- 判定为「被拦住」时，拦住它的那条具体原因码；其余判定为空。
    -- 两者分开存：「被拦住了」与「被什么拦住」是两个事实，合成一个字段就再也拆不开。
    ADD COLUMN last_dispatch_answer_reason text;

ALTER TABLE execution_station
    ADD CONSTRAINT execution_station_dispatch_answer_is_whole
        CHECK ((last_dispatch_answer_at IS NULL) = (last_dispatch_answer_code IS NULL)),
    -- 没有回答就不可能有「被什么拦住」。
    ADD CONSTRAINT execution_station_dispatch_reason_needs_answer
        CHECK (last_dispatch_answer_code IS NOT NULL OR last_dispatch_answer_reason IS NULL);
