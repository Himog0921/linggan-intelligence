import React from 'react';

export default function ConfirmModal({
  open,
  title = '确认操作',
  message = '',
  detail = '',
  confirmText = '确认',
  confirmTone = 'danger',
  onConfirm,
  onCancel,
}) {
  if (!open) return null;

  return (
    <div className="batch-settings-overlay" style={{ display: 'flex' }} aria-hidden="false">
      <div className="batch-settings-dialog confirm-dialog" role="dialog" aria-modal="true">
        <span className={`confirm-dialog-badge tone-${confirmTone}`}>{confirmTone === 'danger' ? '高风险操作' : '确认操作'}</span>
        <h2>{title}</h2>
        <p className="batch-settings-subtitle">{message}</p>
        {detail ? <div className="confirm-dialog-detail">{detail}</div> : null}
        <div className="batch-dialog-actions">
          <button className="popup-btn outline" onClick={onCancel}>
            取消
          </button>
          <button className={`popup-btn ${confirmTone === 'danger' ? 'danger' : 'primary'}`} onClick={onConfirm}>
            {confirmText}
          </button>
        </div>
      </div>
    </div>
  );
}
