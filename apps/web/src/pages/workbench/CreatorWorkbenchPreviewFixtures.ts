export type PreviewParagraph = {
  id: string;
  role: string;
  text: string;
  source?: string;
  alternatives?: string[];
  bridge?: boolean;
};

export type PreviewDraft = {
  id: string;
  title: string;
  account: string;
  accountMark: string;
  kind: '试新' | '复用';
  origin: string;
  state: string;
  must: number;
  warn: number;
  angle: string;
  related?: { title: string; reads: number };
  sampleCode: string;
  sampleName: string;
  styleName: string;
  cover: string;
  coverCandidates: string[];
  noteTitle: string;
  paragraphs: PreviewParagraph[];
  charCount: number;
  paragraphCount: number;
  experienceCount: number;
  tags: string[];
};

const D1_PARAGRAPHS: PreviewParagraph[] = [
  { id: 'p1', role: '钩子', text: '孩子之前有段时间总把书包里的东西弄得乱七八糟。别人问我，我也只是笑笑，都觉得我不怎么在意这些小事。' },
  { id: 'p2', role: '自问', text: '真的不着急吗？', alternatives: ['可我真的不着急吗？'] },
  { id: 'p3', role: '过去的我', text: '可哪有父母真的不操心，尤其是自己的孩子！' },
  { id: 'p4', role: '过去的我', text: '比如刚上小学那会，他总忘记整理书本，我一下班就先检查书包，提醒他每一件事；', source: '经历库 · JL-0001 · 自己整理书包' },
  { id: 'p5', role: '过去的我', text: '上学前我发现他总丢三落四，立刻列好每天要带的清单，生怕第二天忘记；', source: '经历库 · JL-0002 · 每日物品清单' },
  { id: 'p6', role: '过去的我', text: '后来我试过不少办法，提醒、检查、再提醒，基本每件事都要我跟着做，生怕遗漏，总想盯着。', source: '经历库 · JL-0003 · 反复检查书包' },
  { id: 'p7', role: '过去的我', text: '我跟很多家长一样，盯着、催着、提醒着。', alternatives: ['那段时间，我和很多家长一样，盯着、催着、提醒着。'] },
  { id: 'p8', role: '过去的我', text: '一点小事都不敢放下，总怕他又会忘记。' },
  { id: 'p9', role: '转折', text: '后来我才发现，提醒越多，他越不愿自己开始了。', alternatives: ['直到后来我才发现，提醒越多，他越不愿自己开始。'] },
  { id: 'p10', role: '转折场景', text: '那时候他整理书包，我总站在旁边，拿起一本我提醒一句，一漏掉我马上补，一慢下来我就催。我站在身后的时候，他一直等着我开口；我去做自己的事，过一会回来看，他反倒已经整理好了。我才慢慢意识到。', source: '经历库 · JL-0004 · 自己整理后反而更快' },
  { id: 'p11', role: '转折', text: '起初我以为他不够认真，后来才明白，我不停提醒这件事本身，就让他没法自己安排。', alternatives: ['一开始我以为他不够认真，后来才明白：是我不停提醒，让他没法自己安排。'] },
  { id: 'p12', role: '改变', text: '想通之后，我把提醒他的劲收回了。' },
  { id: 'p13', role: '改变', text: '不再把注意力放在每一件小事上以后，周末我们把原来排满的安排改成一起去公园走走，他更感兴趣了，不聊任务我们之间也轻松多了，我才发现他喜欢自己安排路线，现在周末常常会主动提议去哪儿。这让我很意外。', source: '经历库 · JL-0005 · 周末自己规划路线' },
  { id: 'p14', role: '意外收获', text: '除此以外，给我带来惊喜的是后来他开始主动整理自己的东西了，他还会提前一天看天气和路线，和家里人讨论第二天要带什么，慢慢把这些小事安排得有条有理，这些都是以前我没想到的。这些变化让我觉得很意外。', source: '经历库 · JL-0006 · 提前准备出行物品' },
  { id: 'p15', role: '观点', text: '所以对孩子，太着急帮忙，有时反而碍事。', alternatives: ['所以对孩子来说，太着急帮忙，反而容易碍事。'] },
  { id: 'p16', role: '对比', text: '有些事你替他做，他就少了一次自己尝试的机会。孩子不一样，只有亲手做过，才知道下一步怎么安排。' },
  { id: 'pb', role: '衔接', text: '', bridge: true },
  { id: 'p18', role: '收尾', text: '带久了我会想，孩子早晚得自己安排、自己负责，而这部分只能在有人愿意放手的空间里慢慢长出来，也能慢慢找到自己的节奏。' },
  { id: 'p19', role: '收尾', text: '你现在替他操心的每件小事，都是他少了一次自己承担的机会，到最后你也很难放下。所以慢一点不是不管，是忍住不替他做，把地方留给他自己。' },
];

const D2_PARAGRAPHS: PreviewParagraph[] = [
  { id: 'q1', role: '钩子', text: '三年级开学，我给自己定了一条规矩：不再替孩子收书包。' },
  { id: 'q2', role: '自问', text: '心疼吗？当然会。' },
  { id: 'q3', role: '过去的我', text: '刚上小学那会，他总是忘记整理东西。我一回家就先检查书包，生怕漏掉第二天要用的课本。', source: '经历库 · JL-0001 · 自己整理书包' },
  { id: 'q4', role: '转折场景', text: '上周他把一本练习册落在学校，回家有点着急。我没有像以前一样马上替他想办法，而是陪他一起想第二天怎么和老师说明。' },
  { id: 'q5', role: '转折场景', text: '他整理书包时我总站在旁边，一漏掉就提醒，一慢下来就催。我站在身后，他一直等着我开口；我去做自己的事，回来一看，他反倒整理好了。', source: '经历库 · JL-0004 · 自己整理后反而更快' },
  { id: 'q6', role: '观点', text: '书包也是一样。我替他收得越周到，他越不觉得这是自己的事。', alternatives: ['替他兜住每一个小失误，他就会一直等着别人来兜。'] },
  { id: 'qb', role: '衔接', text: '', bridge: true },
  { id: 'q8', role: '收尾', text: '忘带一次课本，天不会塌下来。让他自己经历一次，比我说十遍更有用。' },
];

const makeShortDraft = (
  id: string, title: string, account: string, kind: PreviewDraft['kind'], state: string,
  must: number, warn: number, angle: string, cover: string,
): PreviewDraft => ({
  id, title, account, accountMark: account.slice(0, 1), kind, origin: kind === '复用' ? '复用' : '成品包',
  state, must, warn, angle, sampleCode: 'YB-0001', sampleName: '具体场景松手型', styleName: '真实经历表达',
  cover, coverCandidates: [cover, '先给孩子一点空间', '从一件小事开始'], noteTitle: title,
  paragraphs: D2_PARAGRAPHS.map(item => ({ ...item, id: id + '-' + item.id })),
  charCount: 260, paragraphCount: 7, experienceCount: 2, tags: ['#家庭成长', '#自理能力', '#亲子沟通'],
});

export const INITIAL_DRAFTS: PreviewDraft[] = [
  {
    id: 'd1', title: '不要太替孩子操心', account: '示例妈妈', accountMark: '示',
    kind: '试新', origin: '打样稿', state: '必须先改', must: 1, warn: 5,
    angle: '把空间留给孩子自己，让他学着承担自己的小事',
    related: { title: '先让孩子自己整理', reads: 6116 },
    sampleCode: 'YB-0001', sampleName: '具体场景松手型', styleName: '真实经历表达',
    cover: '不要太替孩子操心',
    coverCandidates: ['不要太替孩子操心', '别总替孩子安排', '带孩子，先学会放手'],
    noteTitle: '不要太替孩子操心', paragraphs: D1_PARAGRAPHS,
    charCount: 771, paragraphCount: 18, experienceCount: 6,
    tags: ['#家庭成长', '#自理能力', '#家庭成长', '#亲子沟通', '#周末安排'],
  },
  {
    id: 'd2', title: '我不再替他收书包了', account: '示例妈妈', accountMark: '示',
    kind: '试新', origin: '成品包', state: '有提醒', must: 2, warn: 2,
    angle: '少替孩子兜底，让他自己承担小后果', sampleCode: 'YB-0001',
    sampleName: '具体场景松手型', styleName: '真实经历表达',
    cover: '我不再替他收书包了',
    coverCandidates: ['我不再替他收书包了', '让他自己收书包', '少替孩子兜底'],
    noteTitle: '我不再替他收书包了', paragraphs: D2_PARAGRAPHS,
    charCount: 312, paragraphCount: 7, experienceCount: 2,
    tags: ['#家庭成长', '#自理能力', '#放手'],
  },
  makeShortDraft('d3', '一次普通周末的安排', '示例家长', '试新', '已检查', 0, 2, '把周末安排留出一点弹性', '周末只做两件事'),
  makeShortDraft('d4', '有些变化是慢慢发生的', '示例妈妈', '复用', '有提醒', 0, 3, '记录一次持续的小变化', '变化其实很具体'),
  makeShortDraft('d5', '给自己留一点调整空间', '示例老师', '试新', '已复制', 0, 0, '一次临时调整带来的新发现', '计划之外也没关系'),
  makeShortDraft('d6', '一件小事带来的提醒', '示例家长', '复用', '已登记发布', 0, 0, '从一个日常细节里找到提醒', '从一件小事开始'),
]);
