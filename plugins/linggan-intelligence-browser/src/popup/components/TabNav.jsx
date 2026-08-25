import React from 'react';

export default function TabNav({ tabs, activeTab, onTabChange }) {
  return (
    <nav className="tab-nav" role="tablist">
      {tabs.map((tab) => (
        <button
          key={tab.id}
          id={tab.id}
          className={`tab-btn ${activeTab === tab.id ? 'active' : ''}`}
          role="tab"
          aria-selected={activeTab === tab.id}
          aria-controls={tab.ariaControls}
          onClick={() => onTabChange(tab.id)}
        >
          {tab.label}
        </button>
      ))}
    </nav>
  );
}
