/** 查询键。键里带完整参数，所以范围一变就是另一个查询——晚到的响应不会覆盖新范围。 */
export const LIST_KEY = 'evidence-list';
export const DETAIL_KEY = 'evidence-detail';
export const DOSSIER_KEY = 'evidence-dossier';

export const listKey = (domain: string | null, params: string) => [LIST_KEY, domain, params] as const;
export const detailKey = (domain: string | null, path: string | null) => [DETAIL_KEY, domain, path] as const;
export const dossierKey = (domain: string, work: string) => [DOSSIER_KEY, domain, work] as const;
