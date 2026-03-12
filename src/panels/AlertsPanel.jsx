import { useState } from 'react';
import useAlerts from '../hooks/useAlerts';
import StatusDot from '../components/StatusDot';
import { formatCurrency, timeAgo } from '../utils/formatters';

export default function AlertsPanel() {
  const { alerts, triggered, loading, createAlert, deleteAlert } = useAlerts();
  const [tab, setTab] = useState('active');
  const [showCreate, setShowCreate] = useState(false);
  const [form, setForm] = useState({ symbol: '', condition: 'price_above', value: '' });

  const handleCreate = async (e) => {
    e.preventDefault();
    if (!form.symbol || !form.value) return;
    await createAlert(form);
    setForm({ symbol: '', condition: 'price_above', value: '' });
    setShowCreate(false);
  };

  if (loading) return <div className="p-4 text-aura-muted animate-pulse">Loading alerts...</div>;

  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between">
        <div className="flex gap-2">
          <button onClick={() => setTab('active')} className={`text-xs px-3 py-1 rounded ${tab === 'active' ? 'bg-aura-blue text-white' : 'text-aura-muted'}`}>
            Active ({alerts.length})
          </button>
          <button onClick={() => setTab('triggered')} className={`text-xs px-3 py-1 rounded ${tab === 'triggered' ? 'bg-aura-amber text-white' : 'text-aura-muted'}`}>
            Triggered ({triggered.length})
          </button>
        </div>
        <button onClick={() => setShowCreate(!showCreate)} className="text-xs text-aura-blue">+ New</button>
      </div>

      {showCreate && (
        <form onSubmit={handleCreate} className="glass-panel p-3 space-y-2">
          <div className="grid grid-cols-3 gap-2">
            <input value={form.symbol} onChange={(e) => setForm(f => ({ ...f, symbol: e.target.value.toUpperCase() }))} className="input-field text-xs" placeholder="Symbol" />
            <select value={form.condition} onChange={(e) => setForm(f => ({ ...f, condition: e.target.value }))} className="input-field text-xs">
              <option value="price_above">Price Above</option>
              <option value="price_below">Price Below</option>
              <option value="volume_above">Volume Above</option>
              <option value="pct_change_above">% Change Above</option>
              <option value="pct_change_below">% Change Below</option>
            </select>
            <input type="number" step="any" value={form.value} onChange={(e) => setForm(f => ({ ...f, value: e.target.value }))} className="input-field text-xs" placeholder="Value" />
          </div>
          <div className="flex gap-2">
            <button type="submit" className="btn-primary text-xs py-1">Create Alert</button>
            <button type="button" onClick={() => setShowCreate(false)} className="btn-secondary text-xs py-1">Cancel</button>
          </div>
        </form>
      )}

      {tab === 'active' ? (
        <div className="space-y-1">
          {alerts.map((a, i) => (
            <div key={a.id || i} className="flex items-center justify-between px-2 py-2 rounded hover:bg-aura-surface2/50 group">
              <div className="flex items-center gap-2">
                <StatusDot color="green" />
                <span className="font-mono text-xs font-medium text-aura-text">{a.symbol}</span>
                <span className="text-xs text-aura-muted">{(a.condition || '').replace(/_/g, ' ')}</span>
                <span className="font-mono text-xs text-aura-blue">{a.value}</span>
              </div>
              <button onClick={() => deleteAlert(a.id)} className="text-xs text-aura-muted hover:text-aura-red opacity-0 group-hover:opacity-100">&#x2715;</button>
            </div>
          ))}
          {alerts.length === 0 && <div className="text-xs text-aura-muted text-center py-6">No active alerts</div>}
        </div>
      ) : (
        <div className="space-y-1">
          {triggered.map((t, i) => (
            <div key={t.id || i} className="flex items-center justify-between px-2 py-2 rounded hover:bg-aura-surface2/50">
              <div className="flex items-center gap-2">
                <StatusDot color="amber" />
                <span className="font-mono text-xs font-medium text-aura-text">{t.symbol}</span>
                <span className="font-mono text-xs text-aura-muted">{formatCurrency(t.price)}</span>
              </div>
              <span className="text-xs text-aura-muted">{timeAgo(t.timestamp || t.triggered_at)}</span>
            </div>
          ))}
          {triggered.length === 0 && <div className="text-xs text-aura-muted text-center py-6">No triggered alerts</div>}
        </div>
      )}
    </div>
  );
}
