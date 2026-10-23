import type { Metric,MetricKey } from './inspectionApi';
export const metricLabels:Record<MetricKey,string>={likes:'点赞',comments:'评论',collects:'收藏',shares:'分享'};
export const ruleLabels:Record<string,string>={primary:'作者主页',latest:'最新',most_liked:'最多点赞',most_collected:'最多收藏',most_commented:'最多评论',comprehensive:'综合'};
export const discoveryLabels:Record<string,string>={new_in_library:'新入库',first_in_scope:'库内已有 · 本规则首次发现',repeat:'再次命中',returned:'再次出现',not_observed_this_round:'本轮未命中',first_scope_unconfirmed:'首次范围未能确认',membership_unconfirmed:'本轮归属待核对'};
export const reasonLabels:Record<string,string>={
  left_round_incomplete:'上轮只取得部分结果',right_round_incomplete:'本轮只取得部分结果',historical_rule_unknown:'历史规则关联未知',different_rules:'两轮来自不同排序规则',execution_conditions_changed:'两轮实际采样条件不同',observation_account_changed:'观察账号发生变化',observation_account_unknown:'历史观察身份未能完整核对',observation_order_unknown:'观察时点无法确立先后',comparison_scope_truncated:'读取范围有限，不计算完整集合差异',metric_not_observed:'该指标未在两次观察中完整取得',metric_precision_insufficient:'来源精度不足，仅并列原值',no_observation_near_24_hours:'无发布后 24 小时附近的真实记录',qualified_publication_time_missing:'缺少可核对的发布时间',work_not_readable_in_scope:'此作品当前不可读取',
};
export function reason(code:string):string{return reasonLabels[code]??'该项暂不能形成可靠比较';}
export function moment(value:string|null):string {
  if(value===null)return '时间未取得';
  if(/^\d{4}-\d{2}-\d{2} \d{2}:\d{2}$/.test(value))return value;
  const date=new Date(value);if(!Number.isFinite(date.valueOf()))return '时间待核对';
  const parts=new Intl.DateTimeFormat('zh-CN',{timeZone:'Asia/Shanghai',year:'numeric',month:'2-digit',day:'2-digit',hour:'2-digit',minute:'2-digit',hourCycle:'h23'}).formatToParts(date);
  const p=(type:string)=>parts.find(item=>item.type===type)?.value??'';
  return `${p('year')}-${p('month')}-${p('day')} ${p('hour')}:${p('minute')}`;
}
export function number(value:number|null):string{return value===null?'—':new Intl.NumberFormat('zh-CN',{maximumFractionDigits:1}).format(value);}
export function metricValue(metric:Metric):string{return metric.state==='known'?number(metric.value):'未取得';}
export function duration(seconds:number|null):string{if(seconds===null)return '间隔待核对';const minutes=Math.round(seconds/60);if(minutes<60)return `${minutes} 分钟`;const hours=Math.floor(minutes/60);return `${hours} 小时${minutes%60?` ${minutes%60} 分钟`:''}`;}
export function age(metric:Metric,published:number|null):string{if(published===null||metric.observedEpochMs===null||metric.observedEpochMs<published)return '观察年龄待核对';return `发布后 ${duration((metric.observedEpochMs-published)/1000)}`;}
export function sourceName(value:string):string{return ({discovery_search:'关键词搜索',profile_discovery:'作者主页',content_detail:'作品详情',detail:'作品详情',discovery:'发现页'} as Record<string,string>)[value]??'来源待核对';}
