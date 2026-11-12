-- Upgrade only active system-seeded methods. Custom prompts and frozen batches retain identity.
-- Settings locks also serialize version allocation with prompt save/activation.
DO $migration$
DECLARE
 target record;
 seed linggan_ai_prompt_version%ROWTYPE;
 current_config linggan_ai_task_config%ROWTYPE;
 new_prompt uuid;
 new_config uuid;
 next_version bigint;
 revised_system text;
 revised_instruction text;
 core_rules text := $core$
Core来源约定：inputContract=topic-map.core-source.v1时，输入是已完成内容理解的固定Core，主题阶段只投影讨论、比较概念与归属，不重新理解原材料。评论不产生主题方向。文本依据是固定来源中的精确引文；core_image_evidence是固定Core的图片modelQuote或observation派生文字，verification=model_read，不是已经人工核对的原文或OCR。可以依据其明确表达的内容研究主题，但必须保留派生性质、表达者和不确定性；不能声称看过原图、验证了文字识别或将观察描述冒充作者逐字原话。sourceContent.anchor只标识来源及读取性质，不补充未引用事实。非Core历史合同仍按其冻结来源解释。
$core$;
 grouping_rules text := $group$
先比较本批units的概念边界，再与完整definitions目录比较。边界相同而措辞不同属于同一概念；不同立场、support/challenge及不同证据不自行拆主题。若本批同一新概念没有可匹配目录身份，各单元仍独立返回decision、原声与角色，但逐字复用同一规范proposedTopic：label、definition、inclusionCriteria、exclusionCriteria和parent保持一致；不为逐条措辞变化另写定义。真正不同的问题边界分别保留，不以共同关键词强合。不要引用尚未存在的主题ID，程序将复用同批一致定义的身份。
完整目录均参与判断，但relations只输出有实质意义的等价、包含、相关或差异关系，无关候选不必逐条生成distinct。equivalent必须matched；new不能复制已匹配主题。候选supportExamples是定义创建来源样例，用于理解既有边界，不代表全部样本，也不能为当前单元补事实依据。
$group$;
BEGIN
 PERFORM 1 FROM linggan_ai_task_setting
 WHERE task_key IN ('topic_research','topic_research_extract_review','topic_research_resolve','topic_research_resolve_review')
 ORDER BY task_key FOR UPDATE;
 FOR target IN SELECT * FROM (VALUES
  ('topic_research',2), ('topic_research_extract_review',1),
  ('topic_research_resolve',1), ('topic_research_resolve_review',1)
 ) AS targets(task_key,seed_version) ORDER BY task_key LOOP
  SELECT * INTO STRICT seed FROM linggan_ai_prompt_version p
   WHERE p.task_key=target.task_key AND p.version_no=target.seed_version;
  revised_system := seed.system_text;
  revised_instruction := seed.instruction_text;
  IF target.task_key='topic_research_resolve' THEN
   revised_system := replace(revised_system,
    'new 对每个候选提供 equivalent/broader/narrower/related/distinct/uncertain 的关系与差异理由；equivalent 应改 matched。',
    'new 综合完整候选目录判断，只输出有实质意义的关系与差异理由；无关候选无需逐项输出，equivalent 应改 matched。');
   revised_system := revised_system || grouping_rules;
  ELSIF target.task_key IN ('topic_research_extract_review','topic_research_resolve_review') THEN
   revised_system := replace(revised_system,
    'evidence.text是提案实际引用的原文',
    'evidence.text是提案实际引用的固定依据文字；文本引文与Core图片派生观察须按来源性质区分');
   revised_system := replace(revised_system,
    '逐一核查new对全部候选的比较。',
    '结合完整候选目录核查new是否重复已有概念；省略无关候选的relations不是失败理由。');
   revised_system := revised_system || $review$
同批new提案若概念边界相同，应复用一致的definition、inclusionCriteria、exclusionCriteria和parent，不能因措辞变化碎裂为不同主题；这一判断属于semantic_identity_relation，不新增checks。不同问题边界不能为了复用而强合。候选supportExamples仅供理解目录边界，不能作为当前单元statement的补充证据。图片Core的model_read依据只能证明提案与给定派生文字一致，不能证明原图识别准确；复核不补看图片、不把派生观察改称作者原话。
$review$;
  ELSE
   revised_instruction := '按 outputSchema 的短键合同投影固定来源。Core合同优先使用contentCore已有知识单元；只为作者内容提炼方向。引文f/q精确复制所引依据片段，图片Core派生文字不称为原文。保留旧raw合同来源约定。';
  END IF;
  revised_system := revised_system || core_rules;
  SELECT COALESCE(max(version_no),0)+1 INTO next_version
   FROM linggan_ai_prompt_version WHERE task_key=target.task_key;
  new_prompt := gen_random_uuid();
  INSERT INTO linggan_ai_prompt_version(version_ref,task_key,version_no,system_text,instruction_text)
   VALUES(new_prompt,target.task_key,next_version,revised_system,revised_instruction);
  SELECT c.* INTO STRICT current_config FROM linggan_ai_task_setting s
   JOIN linggan_ai_task_config c ON c.task_config_ref=s.current_config_ref WHERE s.task_key=target.task_key;
  IF current_config.prompt_version_ref=seed.version_ref THEN
   new_config := gen_random_uuid();
   INSERT INTO linggan_ai_task_config(task_config_ref,task_key,model_config_ref,skill_version_ref,
    execution_contract,material_columns,budget_mode,daily_token_limit,allowed_tools,origin,prompt_version_ref)
   VALUES(new_config,current_config.task_key,current_config.model_config_ref,current_config.skill_version_ref,
    current_config.execution_contract,current_config.material_columns,current_config.budget_mode,
    current_config.daily_token_limit,current_config.allowed_tools,'seed',new_prompt);
   UPDATE linggan_ai_task_setting SET current_config_ref=new_config,revision=revision+1,updated_at=scope_001_now()
    WHERE task_key=target.task_key;
  END IF;
 END LOOP;
END
$migration$;
