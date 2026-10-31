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
  origin?: string | null;
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
  /** 粉丝数那次主页资料的观察时间（原始时间串）；没有资料时为 null。 */
  followerObservedAt: string | null;
  biography: string | null;
  focus: Judgement;
  authorAnalysis: Analysis;
  traits: string[];
  topicHints: string[];
  /** 关键词搜索里发现过这位作者作品的关键词，按首次命中时间排序；空数组＝没有记录（不是「无关键词」这个事实）。 */
  discoveryKeywords: string[];
  /** 最早见到的那篇作品；没有作品时为 null。 */
  firstSeenWorkRef: string | null;
  /** 首次发现（进入本领域或关键词命中，取更早的一刻）；没有记录时为 null。 */
  firstSeenMs: number | null;
  /** 北京时间的日期 `YYYY-MM-DD`。 */
  firstSeenDate: string | null;
  firstSeenDisplay: string | null;
  /** recent＝近 7 天新见；earliest_record＝最早已知记录（更早的发现不可知）；domain_first＝本领域首次。 */
  firstSeenPrecision: string | null;
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
  inside: number | null;
  outsideViral: number | null;
  /** 近 7 天首次发现的作者数（北京时间的 7 个自然日）。 */
  recentlyDiscovered: number;
  /** 未加入观察、且同一作者至少 2 篇作品点赞不少于 500 的作者数。 */
  highLikeLeads: number | null;
  /** 当前材料范围内最近一次入库的时间。 */
  latestAddedMs: number | null;
  latestAddedDisplay: string | null;
  unknownDateWorks: number;
}

/** 高赞线索的口径由服务器给出，页面不另编一个数字。 */
export interface LeadRule {
  minLikes: number;
  minWorks: number;
}

export interface CreatorPolicy {
  likeThreshold: number | null;
  revision: number;
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
  /** 高赞线索口径；服务器没给时为 null，页面据此关闭该视图，不猜一个默认口径。 */
  leadRule: LeadRule | null;
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
