import { useState } from 'react';
import useWatchlists from '../hooks/useWatchlists';
import { formatCurrency, pnlColor } from '../utils/formatters';

export default function WatchlistPanel() {
  const { watchlists, selected, setSelected, activeList, loading, prices, addSymbol, removeSymbol, createList } = useWatchlists();
  const [newSymbol, setNewSymbol] = useState('');
  const [newListName, setNewListName] = useState('');
  const [showCreate, setShowCreate] = useState(false);

  const handleAddSymbol = async (e) => {
    e.preventDefault();
    await addSymbol(newSymbol);
    setNewSymbol('');
  };

  const handleCreateList = async (e) => {
    e.preventDefault();
    await createList(newListName);
    setNewListName('');
    setShowCreate(false);
  };

  if (loading) return <div className="p-4 text-aura-muted animate-pulse">Loading watchlists...</div>;

  return (
    <div className="space-y-2">
      <div className="flex items-center gap-2">
        <select
          value={selected || ''}
          onChange={(e) => setSelected(e.target.value)}
          className="flex-1 text-xs bg-aura-surface2 border border-aura-border rounded px-2 py-1.5 text-aura-text outline-none"
        >
          {watchlists.map(w => <option key={w.id} value={w.id}>{w.name} ({(w.symbols || []).length})</option>)}
        </select>
        <button onClick={() => setShowCreate(!showCreate)} className="text-xs text-aura-blue hover:text-aura-text">+ New</button>
      </div>

      {showCreate && (
        <form onSubmit={handleCreateList} className="flex gap-2">
          <input value={newListName} onChange={(e) => setNewListName(e.target.value)} className="input-field text-xs flex-1" placeholder="List name" />
          <button type="submit" className="btn-primary text-xs py-1">Create</button>
        </form>
      )}

      {activeList && (
        <form onSubmit={handleAddSymbol} className="flex gap-2">
          <input value={newSymbol} onChange={(e) => setNewSymbol(e.target.value.toUpperCase())} className="input-field text-xs flex-1" placeholder="Add symbol" />
          <button type="submit" className="btn-primary text-xs py-1">Add</button>
        </form>
      )}

      <div className="space-y-0.5">
        {(activeList?.symbols || []).map((item, i) => {
          const sym = typeof item === 'string' ? item : item.symbol;
          const p = prices[sym];
          return (
            <div key={i} className="flex items-center justify-between px-2 py-1.5 rounded hover:bg-aura-surface2/50 group">
              <span className="font-mono text-xs font-medium text-aura-text">{sym}</span>
              <div className="flex items-center gap-3">
                {p && (
                  <>
                    <span className="font-mono text-xs text-aura-text">{formatCurrency(p.price)}</span>
                    <span className="font-mono text-xs" style={{ color: pnlColor(p.change) }}>
                      {p.change >= 0 ? '+' : ''}{p.change.toFixed(2)}%
                    </span>
                  </>
                )}
                <button onClick={() => removeSymbol(sym)} className="text-xs text-aura-muted hover:text-aura-red opacity-0 group-hover:opacity-100 transition-opacity">
                  &#x2715;
                </button>
              </div>
            </div>
          );
        })}
        {(!activeList?.symbols || activeList.symbols.length === 0) && (
          <div className="text-xs text-aura-muted text-center py-6">No symbols in watchlist</div>
        )}
      </div>
    </div>
  );
}
