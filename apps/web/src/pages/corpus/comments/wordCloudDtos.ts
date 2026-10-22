import { bool, decode, list, num, str, type Json } from '../../../shared/decode';
import type { CommentKey } from './studyApi';

export type WordCategory = 'scenario' | 'emotion' | 'approach' | 'unclassified';
export interface WordFrequency {
  text: string;
  count: number;
  workCount: number;
  category: WordCategory;
  examples: CommentKey[];
}
export interface WordCloudResult {
  domainRef: string;
  days: 7 | 28 | 56;
  timezone: string;
  asOf: string;
  algorithmVersion: string;
  eligibleCommentCount: number;
  processedCommentCount: number;
  truncated: boolean;
  textPrefixCharacters: number;
  items: WordFrequency[];
}

function natural(value: number, name: string) {
  if (!Number.isSafeInteger(value) || value < 0) throw new Error(`Invalid word-cloud ${name}`);
  return value;
}
export function parseWordCloud(value: Json): WordCloudResult {
  return decode(value, root => {
    const days = num(root, 'days');
    if (days !== 7 && days !== 28 && days !== 56) throw new Error('Invalid word-cloud days');
    const eligibleCommentCount = natural(num(root, 'eligibleCommentCount'), 'eligibleCommentCount');
    const processedCommentCount = natural(num(root, 'processedCommentCount'), 'processedCommentCount');
    const truncated = bool(root, 'truncated');
    if (processedCommentCount > eligibleCommentCount || truncated !== (processedCommentCount < eligibleCommentCount)) {
      throw new Error('Contradictory word-cloud coverage');
    }
    const items = list(root, 'items', item => {
      const category = str(item, 'category');
      if (!['scenario', 'emotion', 'approach', 'unclassified'].includes(category)) throw new Error('Invalid phrase category');
      const text = str(item, 'text');
      if (!text || text.length > 64) throw new Error('Invalid phrase label');
      const count = natural(num(item, 'count'), 'count'), workCount = natural(num(item, 'workCount'), 'workCount');
      if (count > processedCommentCount || workCount > count) throw new Error('Invalid phrase support');
      return {
        text, count, workCount, category: category as WordCategory,
        examples: list(item, 'examples', example => ({
          workRef: str(example, 'workRef'),
          commentExternalId: str(example, 'commentExternalId'),
        })),
      };
    });
    return {
      domainRef: str(root, 'domainRef'), days, timezone: str(root, 'timezone'),
      asOf: str(root, 'asOf'), algorithmVersion: str(root, 'algorithmVersion'),
      eligibleCommentCount, processedCommentCount, truncated,
      textPrefixCharacters: natural(num(root, 'textPrefixCharacters'), 'textPrefixCharacters'),
      items,
    };
  });
}
