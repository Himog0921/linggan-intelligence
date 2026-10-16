// Existing Collection receipt copy, migrated without changing its meaning.
const messages: Record<string, { className: string; title: string; explanation: string }> = {
  "domain_name_required": {
    "className": "c-src-feedback c-src-feedback-warn",
    "title": "新领域还没起名",
    "explanation": "目标没有建立。选了「新建一个领域」就要给它一个名字——领域名是后面所有材料归属的依据，不能留空。"
  },
  "domain_name_taken": {
    "className": "c-src-feedback c-src-feedback-warn",
    "title": "这个领域已经有了",
    "explanation": "目标没有建立，也没有新建重名领域。想归进已有的那个领域，直接在下拉里选它；这里不静默复用同名领域，否则你以为新开了一个领域，实际把目标归进了另一个已经在用的领域。"
  },
  "domain_create_failed": {
    "className": "c-src-feedback c-src-feedback-warn",
    "title": "领域没有建成",
    "explanation": "目标也没有建立。这次什么都没有改动，可以重试；如果反复失败，说明领域这部分功能当前不可用，而不是你填错了。"
  },
  "target_domain_unassigned": {
    "className": "c-src-feedback c-src-feedback-warn",
    "title": "这个目标还没归属领域",
    "explanation": "采集没有开始。请先把这个目标关联到一个研究领域，再发起采集。领域限定后续语料和研究范围；作品材料仍进入共享材料链。"
  },
  "target_domain_required": {
    "className": "c-src-feedback c-src-feedback-warn",
    "title": "还没选领域",
    "explanation": "新建观察目标前先选一个现有研究领域。新关联默认使用 primary 角色；之后可在领域管理调整。作品材料统一进入共享材料链。"
  },
  "target_domain_assigned": {
    "className": "c-src-feedback c-src-feedback-ok",
    "title": "领域已分配",
    "explanation": "这个目标已按 primary 角色关联到领域，可以继续建立档案。没有重新采集博主，也没有复制或改写已有材料。"
  },
  "target_domain_already_assigned": {
    "className": "c-src-feedback c-src-feedback-warn",
    "title": "这个目标已经有领域",
    "explanation": "没有覆盖现有领域。更换领域会改变后续材料归属，不能通过首次分配入口静默完成。"
  },
  "target_domain_unavailable": {
    "className": "c-src-feedback c-src-feedback-warn",
    "title": "所选领域当前不可用",
    "explanation": "没有修改目标。该领域可能已经停用或不存在，请重新打开分配面板选择当前可用领域。"
  },
  "target_domain_target_missing": {
    "className": "c-src-feedback c-src-feedback-warn",
    "title": "目标已经不存在",
    "explanation": "没有进行领域分配。请刷新观察目标列表确认当前状态。"
  },
  "target_domain_assignment_invalid": {
    "className": "c-src-feedback c-src-feedback-warn",
    "title": "领域分配没有完成",
    "explanation": "没有修改目标。请重新打开分配面板再试一次。"
  },
  "target_domain_assignment_failed": {
    "className": "c-src-feedback c-src-feedback-warn",
    "title": "领域分配没有完成",
    "explanation": "没有修改目标。请重新打开分配面板再试一次。"
  },
  "archive_requested": {
    "className": "c-src-feedback c-src-feedback-ok",
    "title": "建档已入队",
    "explanation": "已记录这次建立档案请求。系统会先取回作品链接（创作者取主页目录，关键词按排序翻搜索面），再逐篇补齐详情；目录和详情只会在接纳真实回执后更新。执行要等一个空闲工位，可能需要几分钟，不用再点一次。"
  },
  "creator_detail_requested": {
    "className": "c-src-feedback c-src-feedback-ok",
    "title": "已排入缺口补采",
    "explanation": "已按已登记作品冻结可补采范围，最多 200 篇。缺详情的作品会采详情、最多 30 条评论（含回复）及媒体，并按授权进行图片文字识别和语音转写；仅缺评论的作品会重访详情页，采最多 30 条评论（含回复），每条主评论最多 2 条回复，不重复下载媒体。已确认失效及正在采集的作品不会重复排入；实际取得情况以材料回执为准。"
  },
  "keyword_detail_requested": {
    "className": "c-src-feedback c-src-feedback-ok",
    "title": "已排入详情补采",
    "explanation": "这个词的链接已经拿到，正在按点赞从高到低逐篇补详情：正文、发布时间、最多 30 条评论（含回复，每条主评论最多 2 条回复），并把图片与视频文件一并取回（其中的文字与语音随后识别）。一次补三篇，补完之后调度会自己接着补下一批。执行要等一个空闲工位。"
  },
  "keyword_detail_complete": {
    "className": "c-src-feedback c-src-feedback-ok",
    "title": "详情已补齐",
    "explanation": "这个词当前拿到的作品都取过详情了。下一步是开始每周巡检，看它每周新增什么。"
  },
  "monitor_rule_retired": {
    "className": "c-src-feedback c-src-feedback-ok",
    "title": "这条规则已停用",
    "explanation": "它不再排期。已经签发过的工单与采回来的材料仍然留着——删掉会让那些材料说不清是按什么口径取回来的。想重新用它，按同一个排序再存一条规则即可。"
  },
  "monitor_rule_is_the_last_one": {
    "className": "c-src-feedback c-src-feedback-warn",
    "title": "这是最后一条规则",
    "explanation": "没有停用。观察中的目标必须有一条规则，否则调度不知道该按什么口径去看它。要完全停下来，用这一行的观察开关停止观察；那是另一个决定，不该由「停用一条规则」顺带完成。"
  },
  "monitor_rule_paused": {
    "className": "c-src-feedback c-src-feedback-ok",
    "title": "这条规则已暂停",
    "explanation": "只停了这一条口径，这个目标的其他规则照自己的周期继续跑。人工观察仍可申请。"
  },
  "monitor_rule_resumed": {
    "className": "c-src-feedback c-src-feedback-ok",
    "title": "这条规则已启用",
    "explanation": "它重新排期了；尚未据此声称已经派出任务。"
  },
  "monitor_rule_saved": {
    "className": "c-src-feedback c-src-feedback-ok",
    "title": "规则已保存",
    "explanation": "只写入了一版规则，没有据此声称采集已开始或完成。"
  },
  "monitor_rule_command_stale": {
    "className": "c-src-feedback c-src-feedback-warn",
    "title": "页面上的版本已经旧了",
    "explanation": "这一条规则在你打开这一页之后被改过（可能是另一个标签页，也可能是一次巡检推进了它的版本）。**什么都没有改动**。刷新一下看当前的样子再点。"
  },
  "monitor_rule_command_replayed": {
    "className": "c-src-feedback c-src-feedback-ok",
    "title": "这条命令已经处理过了",
    "explanation": "重复提交没有再改一次；显示的是原来那次的耐久结果。"
  },
  "monitor_rule_command_rejected": {
    "className": "c-src-feedback c-src-feedback-warn",
    "title": "这次命令没有被接受",
    "explanation": "规则一动没动。原因记在这条命令的回执里；从这一行的「管理巡查」进去可以看到逐字说明。"
  },
  "monitor_rule_unknown": {
    "className": "c-src-feedback c-src-feedback-warn",
    "title": "没找到这条规则",
    "explanation": "它可能已经被停用了。刷新一下看当前还有哪几条。"
  },
  "monitor_rule_retire_failed": {
    "className": "c-src-feedback c-src-feedback-warn",
    "title": "规则没有停用",
    "explanation": "这次什么都没有改动，可以重试。反复失败说明规则管理当前不可用，而不是你点错了。"
  },
  "archive_merge": {
    "className": "c-src-feedback c-src-feedback-warn",
    "title": "未重复提交",
    "explanation": "已有相同建档任务等待处理或执行中。已打开建档状态；本次没有创建第二个任务。"
  },
  "target_deleted": {
    "className": "c-src-feedback c-src-feedback-ok",
    "title": "观察目标已删除",
    "explanation": "它的采集申请、工单与租约一并清掉了。已经采到的作品、详情与评论仍在语料库里，也仍然属于这个博主——作者归属来自作品自己，不依赖观察目标是否存在。"
  },
  "target_delete_blocked": {
    "className": "c-src-failure",
    "title": "没有完成",
    "explanation": "这个目标下已有必须保留的材料或人工结论。没有做任何删除；在有独立保留方案前，不能为了删掉观察决定而把它们一起抹掉。"
  },
  "target_delete_name_mismatch": {
    "className": "c-src-failure",
    "title": "没有完成",
    "explanation": "输入的名字与目标名字不一致，没有删除任何东西。"
  },
  "target_delete_failed": {
    "className": "c-src-failure",
    "title": "没有完成",
    "explanation": "这次删除没有生效，目标与它的记录都没有变化。可以再试一次。"
  },
  "target_delete_invalid": {
    "className": "c-src-failure",
    "title": "没有完成",
    "explanation": "这次删除没有生效，目标与它的记录都没有变化。可以再试一次。"
  },
  "target_delete_missing": {
    "className": "c-src-failure",
    "title": "没有完成",
    "explanation": "这次删除没有生效，目标与它的记录都没有变化。可以再试一次。"
  },
  "patrol_paused": {
    "className": "c-src-feedback c-src-feedback-ok",
    "title": "已停止观察",
    "explanation": "不再排新的巡检；已经在跑的会跑完。语料与档案全部保留，随时可以恢复。"
  },
  "patrol_resumed": {
    "className": "c-src-feedback c-src-feedback-ok",
    "title": "已恢复观察",
    "explanation": "会按现有规则重新排巡检。"
  },
  "patrol_toggle_partial": {
    "className": "c-src-feedback c-src-feedback-warn",
    "title": "有规则没有翻过来",
    "explanation": "这个开关一次翻这个目标的全部规则，其中至少有一条在你打开这一页之后被改过（另一个标签页，或者一次巡检推进了它的版本），那一条没有改动。这一行的状态显示的是真实情况：只要还有一条在跑，它就仍然是「巡查中」。刷新后从检查器的规则台逐条确认。"
  },
  "patrol_toggle_no_rule": {
    "className": "c-src-failure",
    "title": "没有完成",
    "explanation": "这个目标还没有生效的观察规则，没有可以开关的东西。先设置一条规则。"
  },
  "patrol_toggle_failed": {
    "className": "c-src-failure",
    "title": "没有完成",
    "explanation": "这次开关没有生效，巡检状态没有变化。"
  },
  "patrol_toggle_invalid": {
    "className": "c-src-failure",
    "title": "没有完成",
    "explanation": "这次开关没有生效，巡检状态没有变化。"
  },
  "material_retirement_done": {
    "className": "c-src-feedback c-src-feedback-ok",
    "title": "已记下这几篇不存在了",
    "explanation": "它们仍留在作品目录里——博主当时确实发过——只是不再计入待补齐。作品若日后恢复并采到详情，会自动按已有详情算。"
  },
  "material_retirement_empty": {
    "className": "c-src-failure",
    "title": "没有完成",
    "explanation": "一篇都没有勾选。没有勾选不等于全部确认，这次没有写下任何判断。"
  },
  "material_retirement_none": {
    "className": "c-src-feedback c-src-feedback-warn",
    "title": "没有新的判断",
    "explanation": "勾选的作品此前已经确认过，或它们已经取到了详情。没有重复写入第二条记录。"
  },
  "material_retirement_failed": {
    "className": "c-src-failure",
    "title": "没有完成",
    "explanation": "这次判断没有写进去。作品目录与待补齐都没有变化，可以再试一次。"
  },
  "material_retirement_invalid": {
    "className": "c-src-failure",
    "title": "没有完成",
    "explanation": "这次提交没有说明是哪个观察目标，因此没有写下任何判断。"
  },
  "identity_unrecognised": {
    "className": "c-src-failure",
    "title": "没有完成",
    "explanation": "认不出这是谁。创作者请粘主页链接（里面带平台 ID），关键词直接写词就行。"
  },
  "store_failed": {
    "className": "c-src-failure",
    "title": "没有完成",
    "explanation": "没有保存成功。这个目标可能已经在观察列表里了。"
  },
  "archive_not_requestable": {
    "className": "c-src-failure",
    "title": "没有完成",
    "explanation": "目标当前所处阶段不接受这次建档或补采申请。没有新建工单；请查看目标状态。"
  },
  "archive_purpose_mismatch": {
    "className": "c-src-failure",
    "title": "没有完成",
    "explanation": "这次建档用途与已有档案任务的冻结用途不一致，没有新建工单。请核对目标原建档用途与当前授权。"
  },
  "archive_target_missing": {
    "className": "c-src-failure",
    "title": "没有完成",
    "explanation": "这个观察目标已不存在，没有发起采集。请刷新列表。"
  },
  "archive_request_failed": {
    "className": "c-src-feedback c-src-feedback-warn",
    "title": "提交结果待核对",
    "explanation": "建档或补采申请未收到可靠的提交结果。请先刷新目标与采集任务，保留诊断编号供排查，避免直接重复发起。"
  },
  "archive_in_progress": {
    "className": "c-src-failure",
    "title": "没有完成",
    "explanation": "已有一批作品正在补齐，请先查看当前进度。"
  },
  "archive_nothing_to_continue": {
    "className": "c-src-failure",
    "title": "当前无可补采内容缺口",
    "explanation": "当前目录没有可判定的详情或评论缺口；未知正文和零条评论本身不构成重采依据。"
  },
  "archive_baseline_complete": {
    "className": "c-src-feedback c-src-feedback-ok",
    "title": "本轮建档已收口",
    "explanation": "本轮根建档基线已收口，没有新建根建档工单。巡检新增作品的缺口请从补采入口查看；其它内容覆盖仍以材料回执为准。"
  },
  "archive_gap_not_schedulable": {
    "className": "c-src-feedback c-src-feedback-warn",
    "title": "缺口暂不能排入",
    "explanation": "目录仍有详情或评论缺口，但当前缺少可用访问地址，或受执行输入与重试条件限制。本次没有新建工单；请查看受阻作品和采集任务。"
  },
  "archive_refuse": {
    "className": "c-src-failure",
    "title": "没有完成",
    "explanation": "当前没有覆盖本次范围的采集授权，请完成授权后再试。"
  },
  "archive_defer": {
    "className": "c-src-failure",
    "title": "没有完成",
    "explanation": "当前暂无可用采集能力，或今天的采集额度已用完；稍后可以重试。"
  },
  "archive_lease_failed": {
    "className": "c-src-failure",
    "title": "没有完成",
    "explanation": "工单准备或执行资源分配未完成；请查看当前采集阶段与工位状态后再试。"
  },
  "archive_authorization_below_200": {
    "className": "c-src-failure",
    "title": "没有完成",
    "explanation": "当前采集授权不足以支持前 200 篇作品的有界建档。本次没有缩小范围后静默开始。"
  },
  "archive_unavailable": {
    "className": "c-src-failure",
    "title": "没有完成",
    "explanation": "这次请求没有送达采集链路，没有发起任何采集，目标状态也没有改动。可以稍后重试。"
  },
  "keyword_archive_target_missing": {
    "className": "c-src-failure",
    "title": "目标已不存在",
    "explanation": "没有建立档案，也没有创建新的采集任务。这个观察目标已被删除或当前地址指向了旧记录；刷新列表后再继续，而不是重试这次请求。"
  },
  "keyword_archive_incomplete": {
    "className": "c-src-feedback c-src-feedback-warn",
    "title": "关键词建档尚未完成",
    "explanation": "没有开启或恢复巡查。请先完成搜索面的有界建档，并补齐已发现作品的详情；这两段都由接纳的真实回执证明，不能用已提交任务或空状态代替。"
  },
  "keyword_patrol_archive_unreadable": {
    "className": "c-src-failure",
    "title": "无法读取关键词建档状态",
    "explanation": "没有开启或恢复巡查。未知不等于已完成；等建档读取恢复后，系统才会判断这个关键词是否可以进入巡查。"
  },
  "keyword_archive_not_requestable": {
    "className": "c-src-failure",
    "title": "没有完成",
    "explanation": "采集准入按这个词当前所处的状态拒绝了这次请求，因此这次没有发起采集，目标状态也没有改动。准入的前置是状态：历史建档只在开始观察之前接受；详情补采在这个词停止观察之后就不再接受。**重试不会改变结果**——这一条是状态前置，不是暂缺资源。"
  },
  "keyword_archive_unreadable": {
    "className": "c-src-failure",
    "title": "没有完成",
    "explanation": "读不到这个词的建档状态，因此这次没有发起采集（建档与补详情要看它才能定）。目标状态没有改动，可以稍后重试。"
  },
  "batch_nothing_selected": {
    "className": "c-src-failure",
    "title": "没有完成",
    "explanation": "没有选中任何观察目标。"
  },
  "batch_unknown_action": {
    "className": "c-src-failure",
    "title": "没有完成",
    "explanation": "当前不支持这个操作。"
  },
  "batch_failed": {
    "className": "c-src-failure",
    "title": "没有完成",
    "explanation": "这次操作没有完成，也没有改动任何观察目标。"
  },
  "monitoring_toggle_failed": {
    "className": "c-src-failure",
    "title": "没有完成",
    "explanation": "巡查开关没有切换成功。"
  },
  "read_model_not_connected": {
    "className": "c-src-failure",
    "title": "没有完成",
    "explanation": "当前无法读取目标状态，这次没有产生任何改动。"
  }
};
const fallback = { className: 'c-src-failure', title: '没有完成', explanation: '上一次动作没有完成。' };
export function targetFeedback(code: string | null, ahead: string | null = null) {
  if (!code) return null;
  const rawDiagnostic = code.startsWith('archive_request_failed_')
    ? code.slice('archive_request_failed_'.length)
    : null;
  const diagnostic = rawDiagnostic && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(rawDiagnostic)
    ? rawDiagnostic.toLowerCase()
    : null;
  const copy = messages[diagnostic ? 'archive_request_failed' : code] ?? fallback;
  if (copy.className === 'c-src-feedback c-src-feedback-ok') {
    const position = ahead !== null && /^-?\d+$/.test(ahead) ? Number(ahead) : null;
    const queue = position !== null && Number.isSafeInteger(position)
      ? position > 0 ? `前面还有 ${position} 个在排队。` : '它就排在最前面。'
      : '';
    return { ...copy, explanation: copy.explanation + queue };
  }
  return diagnostic ? { ...copy, explanation: `${copy.explanation} 诊断编号：${diagnostic}。` } : copy;
}
