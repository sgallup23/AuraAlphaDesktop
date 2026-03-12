import { useState, useEffect, useRef, useMemo, useCallback } from 'react';
import { AnimatePresence, motion } from 'framer-motion';
import { PANELS } from '../docking/panelRegistry';

// Build the command list once
const COMMANDS = [
  // Panels
  ...Object.values(PANELS).map(p => ({
    id: `panel:${p.id}`,
    label: `Add ${p.title}`,
    category: 'Panels',
    action: { type: 'add-panel', panelId: p.id },
  })),
  // Navigation
  { id: 'nav:pop-out', label: 'Pop Out Current Panel', category: 'Navigation', action: { type: 'action', actionId: 'pop-out' } },
  // Actions
  { id: 'action:health', label: 'Check Health', category: 'Actions', shortcut: null, action: { type: 'action', actionId: 'check-health' } },
  { id: 'action:compact', label: 'Toggle Compact Mode', category: 'Actions', action: { type: 'action', actionId: 'toggle-compact' } },
  { id: 'action:refresh', label: 'Refresh Data', category: 'Actions', action: { type: 'action', actionId: 'refresh-data' } },
  // Account
  { id: 'account:logout', label: 'Logout', category: 'Account', action: { type: 'action', actionId: 'logout' } },
];

// Simple fuzzy match — checks if all query chars appear in order in the target
function fuzzyMatch(query, target) {
  const q = query.toLowerCase();
  const t = target.toLowerCase();
  if (!q) return { match: true, score: 0 };
  let qi = 0;
  let score = 0;
  let lastIdx = -1;
  for (let ti = 0; ti < t.length && qi < q.length; ti++) {
    if (t[ti] === q[qi]) {
      // Bonus for consecutive matches
      score += (lastIdx === ti - 1) ? 2 : 1;
      // Bonus for match at start of word
      if (ti === 0 || t[ti - 1] === ' ') score += 3;
      lastIdx = ti;
      qi++;
    }
  }
  if (qi < q.length) return { match: false, score: 0 };
  return { match: true, score };
}

const CATEGORY_ORDER = ['Panels', 'Navigation', 'Actions', 'Account'];

export default function CommandPalette({ open, onClose, onAddPanel, onAction }) {
  const [query, setQuery] = useState('');
  const [selectedIdx, setSelectedIdx] = useState(0);
  const inputRef = useRef(null);
  const listRef = useRef(null);

  // Filter and sort commands by fuzzy match
  const filtered = useMemo(() => {
    if (!query.trim()) return COMMANDS;
    return COMMANDS
      .map(cmd => ({ ...cmd, ...fuzzyMatch(query, cmd.label + ' ' + cmd.category) }))
      .filter(c => c.match)
      .sort((a, b) => b.score - a.score);
  }, [query]);

  // Group by category preserving order
  const grouped = useMemo(() => {
    const groups = {};
    for (const cmd of filtered) {
      if (!groups[cmd.category]) groups[cmd.category] = [];
      groups[cmd.category].push(cmd);
    }
    // Sort categories by defined order
    return CATEGORY_ORDER
      .filter(cat => groups[cat])
      .map(cat => ({ category: cat, items: groups[cat] }));
  }, [filtered]);

  // Flat list for keyboard nav
  const flatItems = useMemo(() => grouped.flatMap(g => g.items), [grouped]);

  // Reset state when opening
  useEffect(() => {
    if (open) {
      setQuery('');
      setSelectedIdx(0);
      // Focus input after animation starts
      requestAnimationFrame(() => inputRef.current?.focus());
    }
  }, [open]);

  // Clamp selected index when filtered list changes
  useEffect(() => {
    setSelectedIdx(idx => Math.min(idx, Math.max(0, flatItems.length - 1)));
  }, [flatItems.length]);

  // Scroll selected item into view
  useEffect(() => {
    if (!listRef.current) return;
    const el = listRef.current.querySelector(`[data-idx="${selectedIdx}"]`);
    el?.scrollIntoView({ block: 'nearest' });
  }, [selectedIdx]);

  const executeCommand = useCallback((cmd) => {
    if (!cmd) return;
    onClose();
    if (cmd.action.type === 'add-panel') {
      onAddPanel?.(cmd.action.panelId);
    } else if (cmd.action.type === 'action') {
      onAction?.(cmd.action.actionId);
    }
  }, [onClose, onAddPanel, onAction]);

  const handleKeyDown = useCallback((e) => {
    if (e.key === 'Escape') {
      e.preventDefault();
      onClose();
      return;
    }
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setSelectedIdx(i => (i + 1) % Math.max(1, flatItems.length));
      return;
    }
    if (e.key === 'ArrowUp') {
      e.preventDefault();
      setSelectedIdx(i => (i - 1 + flatItems.length) % Math.max(1, flatItems.length));
      return;
    }
    if (e.key === 'Enter') {
      e.preventDefault();
      executeCommand(flatItems[selectedIdx]);
      return;
    }
  }, [flatItems, selectedIdx, onClose, executeCommand]);

  // Build a flat index counter so we can map grouped rendering to flat selectedIdx
  let flatCounter = 0;

  return (
    <AnimatePresence>
      {open && (
        <>
          {/* Backdrop */}
          <motion.div
            className="fixed inset-0 bg-black/50 z-[9998]"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.15 }}
            onClick={onClose}
          />

          {/* Palette */}
          <motion.div
            className="fixed top-[15%] left-1/2 w-full max-w-lg z-[9999]"
            style={{ x: '-50%' }}
            initial={{ opacity: 0, y: -20 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -20 }}
            transition={{ duration: 0.15, ease: 'easeOut' }}
          >
            <div className="bg-aura-surface border border-aura-border rounded-lg shadow-2xl overflow-hidden">
              {/* Search input */}
              <div className="flex items-center px-4 py-3 border-b border-aura-border gap-3">
                <svg className="w-4 h-4 text-aura-muted flex-shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                  <path strokeLinecap="round" strokeLinejoin="round" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
                </svg>
                <input
                  ref={inputRef}
                  type="text"
                  value={query}
                  onChange={(e) => setQuery(e.target.value)}
                  onKeyDown={handleKeyDown}
                  placeholder="Type a command..."
                  className="flex-1 bg-transparent text-sm text-aura-text outline-none placeholder:text-aura-muted"
                />
                <kbd className="text-[10px] text-aura-muted bg-aura-surface2 border border-aura-border rounded px-1.5 py-0.5 font-mono">ESC</kbd>
              </div>

              {/* Command list */}
              <div ref={listRef} className="max-h-72 overflow-y-auto py-1">
                {flatItems.length === 0 && (
                  <div className="px-4 py-6 text-center text-sm text-aura-muted">No matching commands</div>
                )}
                {grouped.map(({ category, items }) => (
                  <div key={category}>
                    <div className="px-4 pt-2.5 pb-1 text-[10px] font-semibold uppercase tracking-wider text-aura-muted">
                      {category}
                    </div>
                    {items.map((cmd) => {
                      const idx = flatCounter++;
                      const isSelected = idx === selectedIdx;
                      return (
                        <button
                          key={cmd.id}
                          data-idx={idx}
                          onClick={() => executeCommand(cmd)}
                          onMouseEnter={() => setSelectedIdx(idx)}
                          className={`w-full flex items-center justify-between px-4 py-2 text-sm transition-colors ${
                            isSelected
                              ? 'bg-aura-blue/15 text-aura-text'
                              : 'text-aura-muted hover:text-aura-text'
                          }`}
                        >
                          <span>{cmd.label}</span>
                          {cmd.shortcut && (
                            <kbd className="text-[10px] text-aura-muted bg-aura-surface2 border border-aura-border rounded px-1.5 py-0.5 font-mono ml-3">
                              {cmd.shortcut}
                            </kbd>
                          )}
                        </button>
                      );
                    })}
                  </div>
                ))}
              </div>

              {/* Footer hint */}
              <div className="px-4 py-2 border-t border-aura-border flex items-center gap-4 text-[10px] text-aura-muted">
                <span className="flex items-center gap-1">
                  <kbd className="bg-aura-surface2 border border-aura-border rounded px-1 py-0.5 font-mono">&uarr;&darr;</kbd>
                  navigate
                </span>
                <span className="flex items-center gap-1">
                  <kbd className="bg-aura-surface2 border border-aura-border rounded px-1 py-0.5 font-mono">&crarr;</kbd>
                  select
                </span>
                <span className="flex items-center gap-1">
                  <kbd className="bg-aura-surface2 border border-aura-border rounded px-1 py-0.5 font-mono">esc</kbd>
                  close
                </span>
              </div>
            </div>
          </motion.div>
        </>
      )}
    </AnimatePresence>
  );
}
