import { useState, useRef, useEffect } from 'react';
import { PANELS } from '../docking/panelRegistry';

export default function PanelMenu({ onAddPanel }) {
  const [open, setOpen] = useState(false);
  const ref = useRef(null);

  useEffect(() => {
    if (!open) return;
    const close = (e) => { if (ref.current && !ref.current.contains(e.target)) setOpen(false); };
    document.addEventListener('mousedown', close);
    return () => document.removeEventListener('mousedown', close);
  }, [open]);

  const groups = {};
  Object.values(PANELS).forEach(p => {
    const g = p.group || 'other';
    if (!groups[g]) groups[g] = [];
    groups[g].push(p);
  });

  return (
    <div ref={ref} className="relative">
      <button
        onClick={() => setOpen(!open)}
        className="text-xs px-2 py-1 rounded border border-aura-border text-aura-muted hover:text-aura-text hover:border-aura-blue transition-colors"
      >
        + Panel
      </button>
      {open && (
        <div className="absolute top-full left-0 mt-1 w-48 bg-aura-surface border border-aura-border rounded-lg shadow-lg z-50 py-1">
          {Object.entries(groups).map(([group, panels]) => (
            <div key={group}>
              <div className="px-3 py-1 text-xs text-aura-muted uppercase tracking-wider">{group}</div>
              {panels.map(p => (
                <button
                  key={p.id}
                  onClick={() => { onAddPanel(p.id); setOpen(false); }}
                  className="w-full text-left px-3 py-1.5 text-xs text-aura-text hover:bg-aura-surface2 transition-colors"
                >
                  {p.title}
                </button>
              ))}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
