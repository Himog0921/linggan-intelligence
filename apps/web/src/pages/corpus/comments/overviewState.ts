export interface OverviewPreferences { method: 'solution' | 'experience'; directory: boolean; seriesDays: number; seriesView: 'chart' | 'table' }
export function initialOverviewPreferences(): OverviewPreferences { return { method: 'solution', directory: false, seriesDays: 28, seriesView: window.matchMedia('(max-width:620px)').matches ? 'table' : 'chart' }; }
