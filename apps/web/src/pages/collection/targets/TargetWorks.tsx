import { TargetSelect } from './TargetSelect';
import { useState } from 'react';
import { Link } from 'react-router';
import type { Drawer } from './api';
import { Section } from './TargetSection';
export function Works({ data, params }: { data: Drawer; params: URLSearchParams }) {
  const creator = data.target.kind === 'creator',
    [query, setQuery] = useState(params.get('catalog_query') ?? ''),
    [filter, setFilter] = useState(params.get('catalog_filter') ?? 'all');
  const works = data.catalog;
  if (!works)
    return (
      <Section title={creator ? '作品目录' : '命中作品'} note="读取暂不可用" className="c-dw-works">
        <div className="c-dw-note"><b>当前读不到可核验的作品记录</b><p>这里不会用主表中的总数拼出一张作品表。恢复读取后，会显示每条真实记录。</p></div>
      </Section>
    );
  const selectedFilter = params.get('catalog_filter') ?? 'all',
    needle = (params.get('catalog_query') ?? '').trim().toLowerCase();
  const matching = works.filter(
    (w) =>
      (selectedFilter === 'pending'
        ? !w.complete
        : selectedFilter === 'complete'
          ? w.complete
          : selectedFilter === 'patrol_new'
            ? w.source === 'patrol'
            : true) &&
      (!needle || w.externalId.toLowerCase().includes(needle) || w.title?.toLowerCase().includes(needle)),
  );
  const completed = works.filter((w) => w.complete).length,
    patrol = works.filter((w) => w.source === 'patrol').length;
  return (
    <Section
      title={creator ? '作品目录' : '命中作品'}
      note={`${works.length} 条可查证${creator ? '作品' : '命中'}`}
      className="c-dw-works"
      id="target-works"
    >
      <div className="c-dw-catalog-summary">
        {[
          [works.length, creator ? '作品目录' : '命中目录'],
          [completed, '详情已取得'],
          [works.length - completed, '待取得详情'],
          [patrol, '巡查新增'],
        ].map(([n, label]) => (
          <div key={label}>
            <b>{n}</b>
            <span>{label}</span>
          </div>
        ))}
      </div>
      {creator && (
        <p className="c-dw-note">
          采集卡类型标记：视频 {works.filter((w) => w.kind === '视频').length}、图文{' '}
          {works.filter((w) => w.kind === '图文').length}、未标记{' '}
          {works.filter((w) => w.kind !== '视频' && w.kind !== '图文').length}
          。历史图文标记可能由插件缺省生成，仅供核对作品分布。
        </p>
      )}
      <form className="c-dw-catalog-filter" action="/collection/targets" method="get">
        {[...params]
          .filter(([k]) => !['catalog_query', 'catalog_filter'].includes(k))
          .map(([k, v]) => (
            <input key={k} type="hidden" name={k} value={v} />
          ))}
        <input
          type="search"
          name="catalog_query"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="搜索作品标题或作品 ID"
          aria-label="搜索作品标题或作品 ID"
        />
        <TargetSelect
          className="c-dw-field"
          name="catalog_filter"
          label="筛选作品"
          value={filter}
          onChange={setFilter}
          options={[
            { value: 'all', label: '全部' },
            { value: 'pending', label: '待采详情' },
            { value: 'complete', label: '详情已完成' },
            { value: 'patrol_new', label: '巡查新增' },
          ]}
        />
        <button className="c-tg-act">筛选</button>
      </form>
      <div className="c-dw-catalog-table-wrap">
        <table className="c-dw-catalog-table">
          <thead>
            <tr>
              {[
                '作品',
                '发布时间',
                creator ? '发现来源' : '创作者',
                creator ? '卡片类型' : '命中位置',
                '详情',
                '媒体处理',
                '评论',
                '最近采集',
                '操作',
              ].map((h) => (
                <th key={h}>{h}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {matching.map((w) => (
              <tr key={w.publicRef}>
                <td>
                  <div className="c-dw-catalog-work">
                    <b>{w.title ?? (w.complete ? '标题未收录' : '标题待取得')}</b>
                    <span>{w.externalId}</span>
                  </div>
                </td>
                <td>{w.published ?? '发布时间待取得'}</td>
                <td>
                  {creator ? (
                    w.source === 'archive' ? (
                      '初始建档'
                    ) : (
                      '巡查新增'
                    )
                  ) : w.creatorId && data.target.platform === 'xhs' ? (
                    <a
                      className="c-dw-author-link"
                      href={`https://www.xiaohongshu.com/user/profile/${encodeURIComponent(w.creatorId)}`}
                      target="_blank"
                      rel="noopener noreferrer"
                      title="根据作者 ID 打开小红书主页"
                    >
                      {w.creatorName ?? '创作者待取得'}
                    </a>
                  ) : (
                    (w.creatorName ?? '创作者待取得')
                  )}
                </td>
                <td>{creator ? (w.kind ?? '未标记') : (w.position ?? '未提供')}</td>
                <td>
                  <span className="c-dw-catalog-state" data-state={w.detailState}>
                    {w.detailLabel}
                  </span>
                </td>
                <td>{w.media}</td>
                <td>{w.comments ?? '—'}</td>
                <td>{w.last ?? '—'}</td>
                <td>
                  <Link
                    className="c-btn-secondary c-dw-work-open"
                    to={`/corpus/evidence?${params.get('domain') && params.get('domain') !== 'all' ? `domain=${encodeURIComponent(params.get('domain') ?? '')}&` : ''}work=${w.publicRef}`}
                  >
                    查看
                  </Link>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {matching.length === 0 && <p className="c-dw-note">当前筛选没有匹配的作品。</p>}
    </Section>
  );
}
