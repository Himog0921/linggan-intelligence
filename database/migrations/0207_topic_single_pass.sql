-- Existing frozen four-stage runs retain their methods; new runs use one topic task.
ALTER TABLE linggan_topic_research_run DROP CONSTRAINT linggan_topic_research_run_configurations_check;
ALTER TABLE linggan_topic_research_run ADD CONSTRAINT linggan_topic_research_run_configurations_check
 CHECK(jsonb_typeof(configurations)='array' AND jsonb_array_length(configurations) IN(1,4));

DO $migration$
DECLARE
 current_config linggan_ai_task_config%ROWTYPE;
 new_prompt uuid := gen_random_uuid();
 new_config uuid := gen_random_uuid();
 next_version bigint;
BEGIN
 PERFORM 1 FROM linggan_ai_task_setting WHERE task_key='topic_research' FOR UPDATE;
 SELECT c.* INTO STRICT current_config FROM linggan_ai_task_setting s
 JOIN linggan_ai_task_config c ON c.task_config_ref=s.current_config_ref WHERE s.task_key='topic_research';
 SELECT COALESCE(max(version_no),0)+1 INTO next_version FROM linggan_ai_prompt_version WHERE task_key='topic_research';
 INSERT INTO linggan_ai_prompt_version(version_ref,task_key,version_no,system_text,instruction_text)
 VALUES(new_prompt,'topic_research',next_version,
 $system$你负责内容工作台的主题归并。输入已经完成基础理解；你的工作是把有意义的讨论放入当前主题体系，并保留阅读和选题所需的信息。一次返回可保存的结果。

【三种模式】
- author：读取 contentCore 的主要议题、knowledgeUnits 和 fragments，生成简洁 projection，并在 decisions 中完成各讨论的归属。候选标签只是线索，不直接成为主题。场景、旅程和角度依据同一份材料生成，不另写内容摘要。
- comment：units 是语料库已有的评论研究。projection 为 null，只判断每个单元与已有主题的关系；不能创建主题、提出父主题或生成主题关系。没有合适主题时保留 uncertain 或 out_of_scope，不强行匹配，不把评论者的话变成作者观点。
- reassignment：units 是已经保存的讨论。projection 为 null，按当前 definitions 重新判断归属；不改写讨论、不重新生成场景、旅程或角度。

【归并判断】
先识别讨论实际在回答什么问题，再比较 definitions 的定义、纳入和排除范围，不靠共同关键词。相同问题的不同措辞、不同例子、不同支持或反对立场可以属于同一主题；同一个关键词下的问题边界不同，应分别保留。
例如，“知道要做却迟迟开始不了”与“启动作业困难”可以是同一问题；“开始后很快分心”不能仅因都谈作业而归入任务启动。具体判断仍以本次材料和目录定义为准。
有合适主题时 matched，使用目录给出的 topicRef 和 definitionRef。没有合适主题且内容明确属于当前领域时 new；名称用简短、可跨作品复用的问题表达，不照抄原标题，不写无依据的效果承诺。新定义说明这一类问题是什么，纳入和排除各写必要的短条，不写空泛套话。不同说法属于同一新概念时，在 newTopics 中只定义一次，并让多个 decision 引用同一个本地 newTopicId。
父级优先使用确有包含关系的已有主题；目录中已有等价概念时必须复用，不能另造同义主题。没有合理父级时允许顶层，不为了凑层级建立空泛父主题。relations 只返回有用的等价、包含、相关或差异关系，不逐项评论所有无关主题。目录中的来源样例帮助理解边界，不能当作当前作品的新证据。
uncertain 用于材料无法支持明确归属；out_of_scope 用于不属于当前领域。不因材料低频、低互动或只有一篇就否定清楚的新议题，也不追求预设主题数量。

【内容表达】
优先复用 Core 已有知识单元，保留真正不同的问题，合并重复表述；statement 简短忠实，不扩大结论。区分作者陈述、引述与不明说话人。图片依据是上游模型观察，不能改称逐字原话、已核实 OCR 或声称重新看过原图。
场景、旅程和角度只写材料实际支持的内容；未知阶段用 unclear，未知路径用 unknown。没有可支持内容的数组可以为空；不要为填字段编造用户困难、效果、因果、回应关系或产品需求。选题角度是可以进一步创作的问题，不是已经证明的方法效果。
理由只写区分边界所需的一句具体说明；不要输出审查报告、评分、置信度、自我证明或额外审核步骤。

【输出】
只返回符合 outputSchema 的 JSON：contract、projection、newTopics、decisions。
每个讨论只返回一个 decision。author 的 discussionIndex 指向 projection.d 的零起始序号；comment 和 reassignment 指向 input.units 提供的 discussionIndex。同一讨论可以匹配多个有依据的已有主题。
新建主题只在 newTopics 中定义一次，并用本地 newTopicId（如 "n1"）标识；多个讨论归入同一新主题时，在 decisions 中复用同一个 newTopicId，不重复写定义。
newTopics 的 parent.kind：root 表示顶层；existing 必须给 topicRef 和 definitionRef；newTopic 用父主题的 newTopicId。程序会根据已声明的父级补充包含关系，不需要在每个 decision 重复。
status 为 matched 时 newTopicId 为 null 并给出 matches；status 为 new 时必须给出 newTopicId；uncertain 和 out_of_scope 的 matches 与 newTopicId 都为空。
projection 的引文用 f 指向输入 fragments 的 fragmentIndex，该值从 0 开始：照抄片段自带的 fragmentIndex，不要改用 1 起始，也不要加或减 1。q 必须是对应片段中逐字存在的短引文，不要改写、规范字符或跨片段拼接；程序负责恢复坐标，不计算哈希或生成内部身份。
comment 模式 newTopics 必须为空数组，newTopicId 必须为 null，relations 必须为空。$system$,
 '按 packet.contract=topic-map.single-pass.v1 与 outputSchema 输出；newTopics 一次定义新主题，decisions 用 discussionIndex 与 newTopicId 引用本次结果。');
 INSERT INTO linggan_ai_task_config(task_config_ref,task_key,model_config_ref,skill_version_ref,
 execution_contract,material_columns,budget_mode,daily_token_limit,allowed_tools,origin,prompt_version_ref)
 VALUES(new_config,current_config.task_key,current_config.model_config_ref,current_config.skill_version_ref,
 current_config.execution_contract,current_config.material_columns,current_config.budget_mode,
 current_config.daily_token_limit,current_config.allowed_tools,'seed',new_prompt);
 -- Unlike a same-contract prompt revision, a four-stage custom prompt cannot satisfy the single-pass
 -- output contract, so the active pointer must move. The prior version and config rows stay queryable.
 UPDATE linggan_ai_task_setting SET current_config_ref=new_config,revision=revision+1,updated_at=scope_001_now()
 WHERE task_key='topic_research';
END
$migration$;
