import React from 'react';

export default function PageContextInfo({ platform, scene, hint, tags, capabilities }) {
  return (
    <div className="context-section">
      <div className="context-card">
        <p className="context-scene">{scene}</p>
        <p className="context-hint">{hint}</p>
        <div className="context-tags">
          {tags.map((tag, i) => (
            <span key={i} className="context-tag">{tag}</span>
          ))}
        </div>
      </div>
    </div>
  );
}
