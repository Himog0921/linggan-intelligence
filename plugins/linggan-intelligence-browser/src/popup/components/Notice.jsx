import React from 'react';
import { icon } from '../../shared/icons.js';
import { getFeedbackMeta } from '../../shared/feedback.js';

export default function Notice({ message, type, visible, onClose }) {
  if (!visible) return null;
  const meta = getFeedbackMeta(type);
  return (
    <section className={`popup-notice ${type}`} id="popupNotice" aria-live="polite" aria-atomic="true">
      <span
        className="popup-notice-icon"
        aria-hidden="true"
        dangerouslySetInnerHTML={{ __html: icon(meta.icon, { size: 16 }) }}
      />
      <div className="popup-notice-copy">
        <strong>{meta.title}</strong>
        <p>{message}</p>
      </div>
      {onClose && (
        <button type="button" className="popup-notice-close" aria-label="关闭提示" onClick={onClose}>
          ×
        </button>
      )}
    </section>
  );
}
