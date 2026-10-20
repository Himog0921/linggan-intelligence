import registry from '../generated/registry.json';
export const standard=registry.visual.typographyApplication;
export const CHANNEL='linggan:type-scale:04';
export const scenes=[['components','组件'],['tasks','采集账本'],['creators','创作者'],['overview','评论概览'],['reading','连续阅读'],['display','英文展示']];
export function variablesFor(){return {
 ...Object.fromEntries(Object.entries(standard.roles).flatMap(([id,r])=>[[`--ty-${id}-size`,`${r.size}px`],[`--ty-${id}-weight`,r.weight],[`--ty-${id}-line`,r.lineHeight]])),
 ...Object.fromEntries(Object.entries(standard.weights).map(([id,value])=>[`--ty-weight-${id}`,value]))
};}
export function frameUrl(scene,slot){return '/?'+new URLSearchParams({page:scene==='tasks'?'task-ledger':scene==='creators'?'creators':'type-preview',typePreview:'1',scene,slot});}
