/**
 * 创作者页读到的数据形状。合同来源：`GET /api/local/creators` 的 `discovery::aggregate`
 * （`crates/evidence/src/creator_discovery.rs`）与 `apps/api/src/local_web/creators.rs`。
 * 只列页面真正用到的字段；服务器多给的字段忽略。
 */
export interface Judgement {
  /** 判断值：related / yes / vertical_tendency / unknown …；读不到时为 unknown，不是别的默认值。 */
  value: string;
  reason: string | null;
  evidenceFragmentIds: string[];
}

export interface Fragment {
  fragmentId: string;
  /** title / body / ocr / transcript / biography … */
  field: string;
  sourceType: string | null;
  workRef: string | null;
  workTitle: string | null;
  text: string;
}

export interface ManualEntry {
  judgement: Judgement;
  supportFragmentIds: string[];
  at: string | null;
  /** 原人工记录保留，但当时支持它的材料已变化或不可读。 */
  supportUnavailable: boolean;
}

export interface Analysis {
  /** idle / queued / running / failed / paused；没有分析记录时为 null。 */
  state: string | null;
  lastErrorCode: string | null;
  resultAt: string | null;
  resultAtDisplay: string | null;
  relevance: Judgement | null;
  traits: Record<string, Judgement>;
  focus: Judgement | null;
  institution: Judgement | null;
  /** 本次实际入模的作品摘要篇数；旧结果与主线 worker 未记录时为 null（不显示成 0）。 */
  sampleWorkCount: number | null;
  availableWorkCount: number | null;
  sampleBasis: string | null;
  sampleLimitations: string[];
  manual: Record<string, ManualEntry>;
  /** 自动结果引用的片段、支持片段和身份片段的并集，按 fragmentId 查找。 */
  fragments: Fragment[];
  supportFragments: Fragment[];
  identitySupportFragments: Fragment[];
}

export interface Work {
  workRef: string;
  title: string | null;
  relevance: string;
  likes: number | null;
  collects: number | null;
  comments: number | null;
  acquisitionKind: string | null;
  firstAdded: string | null;
  firstAddedDisplay: string | null;
  likesObservedAt: string | null;
  likesObservedDisplay: string | null;
  analysis: Analysis;
}

export interface Observation {
  /** unavailable：观察关系没读到，未知，不能当「未纳入」。 */
  lookup: 'available' | 'unavailable';
  inCurrentDomain: boolean | null;
  monitoringState: string | null;
  lifecycleState: string | null;
  usageRole: string | null;
  targetRef: string | null;
  existsInOtherDomains: boolean | null;
  hasOtherPrimaryDomain: boolean | null;
}

export interface CreatorItem {
  creatorKey: string;
  displayName: string | null;
  /** 名称来源：observation_target 表示名字取自观察目标档案，不是作品作者字段。 */
  displayNameKind: string | null;
  authorExternalId: string | null;
  followerCount: number | null;
  biography: string | null;
  focus: Judgement;
  authorAnalysis: Analysis;
  traits: string[];
  topicHints: string[];
  observation: Observation;
  collectedWorkCount: number;
  relatedWorkCount: number;
  matchedRelatedWorkCount: number;
  unrelatedWorkCount: number;
  unknownRelevanceWorkCount: number;
  viralWorkCount: number | null;
  maxRelatedLikeCount: number | null;
  candidateHighLikeCount: number | null;
  matchedWorkRefs: string[];
  representatives: Work[];
}

export interface CreatorStats {
  authors: number;
  works: number;
  unknownAuthorWorks: number;
  relatedAuthors: number;
  vertical: number;
  personal: number;
  /** 观察关系没读到时为 null（未知），不是 0。 */
  outside: number | null;
  outsideViral: number | null;
  unknownDateWorks: number;
}

export interface CreatorPolicy {
  likeThreshold: number | null;
  revision: number;
  analysisEnabled: boolean;
  configRef: string | null;
  dailyTokenLimit: number | null;
}

export interface CreatorDomain {
  domainRef: string;
  name: string | null;
  description: string | null;
  researchGoal: string | null;
  status: string;
}

export interface CreatorList {
  domain: CreatorDomain;
  policy: CreatorPolicy;
  stats: CreatorStats;
  total: number;
  items: CreatorItem[];
  asOf: string | null;
  displayAsOf: string | null;
}

/** 作品资源批量读取的结果（标题、头像、封面）；地址已通过同源媒体路径校验。 */
export interface WorkResource {
  publicRef: string;
  title: string | null;
  avatarUrl: string | null;
  coverUrl: string | null;
}

export interface RegisterReceipt {
  targetRef: string | null;
  created: boolean;
  relationshipAdded: boolean;
  inCurrentDomain: boolean;
  usageRole: string;
  lifecycleState: string | null;
  monitoringEnabled: boolean | null;
}

export interface ModelChoice {
  configRef: string;
  /** 给人看的名字：连接 / 模型 ID；查不到模型记录时才退回到「默认研究配置」。 */
  label: string;
}
