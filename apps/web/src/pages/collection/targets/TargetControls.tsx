import { Link } from 'react-router';
import type { Target, TargetAction } from './api';
import { listHref, returnFields, type Commands } from './useWorkspace';
export function Avatar({ target, drawer = false }: { target: Target; drawer?: boolean }) {
  const cls = drawer ? 'c-dw-avatar' : 'c-tg-avatar';
  return target.avatar.state === 'local' && target.avatar.path ? (
    <img
      className={cls}
      src={target.avatar.path}
      alt=""
      width={drawer ? 48 : 40}
      height={drawer ? 48 : 40}
      referrerPolicy="no-referrer"
    />
  ) : (
    <span
      className={`${cls} ${drawer ? 'c-dw-avatar-placeholder' : 'c-tg-avatar-state'}`}
      title={
        target.avatar.state === 'unavailable'
          ? '头像当前不可用'
          : target.avatar.state === 'pending'
            ? '头像正在准备'
            : '尚未取得头像'
      }
      aria-hidden="true"
    >
      {Array.from(target.name).find((c) => c.trim()) ?? '创'}
    </span>
  );
}
export function Action({
  target,
  action = target.action,
  params,
  commands,
  primary = false,
  className,
}: {
  target: Target;
  action?: TargetAction;
  params: URLSearchParams;
  commands: Commands;
  primary?: boolean;
  className?: string;
}) {
  const cls = className ?? (primary ? 'c-btn-primary' : 'c-tg-act');
  if (action.kind === 'archive' || action.kind === 'gaps')
    return (
      <form
        className={primary ? 'c-dw-primary-form' : undefined}
        onSubmit={(e) => {
          e.preventDefault();
          void commands.run('archive', {
            ...returnFields(params),
            return_drawer: params.get('drawer') ?? '',
            return_dtab: params.get('dtab') ?? 'overview',
            row_target_ref: target.targetRef,
            ...(params.get('domain') && params.get('domain') !== 'all'
              ? { domain_ref: params.get('domain') ?? '' }
              : {}),
            ...(action.kind === 'gaps' ? { archive_action: 'gaps' } : {}),
          });
        }}
      >
        <button className={cls} disabled={commands.busy}>
          {action.label}
        </button>
      </form>
    );
  const updates: Record<string, string | null> =
    action.kind === 'assign_domain'
      ? { assign_domain: target.targetRef }
      : action.kind === 'rule'
        ? { rule: target.targetRef, rule_slot: null }
        : {
            drawer: target.targetRef,
            dtab: action.tab,
            ...(action.kind === 'view_pending' ? { wview: 'list', catalog_filter: 'pending' } : {}),
          };
  return (
    <Link className={cls} to={listHref(params, updates, action.fragment)}>
      {action.label}
    </Link>
  );
}
