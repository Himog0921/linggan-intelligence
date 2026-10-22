export type WordCloudCategory = 'all' | 'scenario' | 'emotion' | 'approach';
export interface OverviewPreferences { method: 'solution' | 'experience'; directory: boolean; seriesDays: number; seriesView: 'chart' | 'table'; wordCategory: WordCloudCategory }
export function initialOverviewPreferences(): OverviewPreferences { return { method: 'solution', directory: false, seriesDays: 56, wordCategory: 'all', seriesView: window.matchMedia('(max-width:620px)').matches ? 'table' : 'chart' }; }
