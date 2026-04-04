import { useState, useMemo } from 'react';
import useStrategyRouting from '../hooks/useStrategyRouting';

const BROKER_OPTIONS = [
  { value: 'ibkr', label: 'Interactive Brokers' },
  { value: 'alpaca', label: 'Alpaca' },
  { value: 'kraken', label: 'Kraken' },
  { value: 'tradier', label: 'Tradier' },
  { value: 'tastytrade', label: 'Tastytrade' },
  { value: 'schwab', label: 'Schwab' },
  { value: 'webull', label: 'Webull' },
  { value: 'moomoo', label: 'Moomoo' },
];

const VENUE_OPTIONS = {
  ibkr: ['SMART', 'NYSE', 'NASDAQ', 'ARCA', 'BATS', 'IEX', 'LSE', 'TSE'],
  alpaca: ['ALPACA'],
  kraken: ['KRAKEN'],
  tradier: ['TRADIER'],
  tastytrade: ['TASTYTRADE'],
  schwab: ['SCHWAB'],
  webull: ['WEBULL'],
  moomoo: ['MOOMOO'],
};

const BOT_OPTIONS = ['shawn', 'shane', 'nova', 'daphne', 'layla'];

export default function StrategyRoutingPanel() {
  const {
    mergedView, loading, message, setMessage,
    saveRoute, deleteRoute, refresh,
  } = useStrategyRouting();

  const [search, setSearch] = useState('');
  const [filterType, setFilterType] = useState('all'); // all, explicit, default
  const [editing, setEditing] = useState(null); // strategy_name being edited
  const [editForm, setEditForm] = useState({});
  const [saving, setSaving] = useState(false);

  const filtered = useMemo(() => {
    let list = mergedView;
    if (search) {
      const q = search.toLowerCase();
      list = list.filter(s => s.strategy_name.toLowerCase().includes(q));
    }
    if (filterType === 'explicit') list = list.filter(s => s.has_explicit_route);
    if (filterType === 'default') list = list.filter(s => !s.has_explicit_route);
    return list;
  }, [mergedView, search, filterType]);

  const startEdit = (row) => {
    setEditing(row.strategy_name);
    setEditForm({
      bot_name: row.bot_name || 'shawn',
      broker_type: row.broker_type || 'ibkr',
      account_id: row.account_id || '',
      venue: row.venue || 'SMART',
      region: row.region || 'us',
      asset_class: row.asset_class || 'equity',
      enabled: row.enabled ?? true,
    });
  };

  const cancelEdit = () => {
    setEditing(null);
    setEditForm({});
  };

  const handleSave = async () => {
    if (!editing) return;
    setSaving(true);
    await saveRoute(editing, editForm);
    setSaving(false);
    setEditing(null);
  };

  const handleDelete = async (strategyName, botName) => {
    if (!confirm(`Remove explicit route for "${strategyName}"? It will fall back to bot defaults.`)) return;
    await deleteRoute(strategyName, botName);
  };

  const venues = VENUE_OPTIONS[editForm.broker_type] || ['SMART'];

  const explicitCount = mergedView.filter(s => s.has_explicit_route).length;
  const defaultCount = mergedView.filter(s => !s.has_explicit_route).length;

  return (
    <div style={{ padding: 16, height: '100%', overflow: 'auto', color: '#E6EDF3', fontFamily: 'JetBrains Mono, monospace', fontSize: 13 }}>
      {/* Header */}
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 12 }}>
        <div>
          <span style={{ fontSize: 15, fontWeight: 600 }}>Strategy Routing</span>
          <span style={{ color: '#8B949E', marginLeft: 12, fontSize: 12 }}>
            {explicitCount} explicit · {defaultCount} default · {mergedView.length} total
          </span>
        </div>
        <button onClick={refresh} style={btnStyle}>{loading ? '...' : '↻ Refresh'}</button>
      </div>

      {/* Message */}
      {message && (
        <div style={{
          padding: '8px 12px', marginBottom: 10, borderRadius: 4, fontSize: 12,
          background: message.type === 'error' ? '#3d1f1f' : '#1f3d2a',
          border: `1px solid ${message.type === 'error' ? '#F85149' : '#3FB950'}`,
          color: message.type === 'error' ? '#F85149' : '#3FB950',
        }}>
          {message.text}
          <button onClick={() => setMessage(null)} style={{ float: 'right', background: 'none', border: 'none', color: 'inherit', cursor: 'pointer' }}>✕</button>
        </div>
      )}

      {/* Filters */}
      <div style={{ display: 'flex', gap: 8, marginBottom: 12 }}>
        <input
          type="text" placeholder="Search strategies..."
          value={search} onChange={e => setSearch(e.target.value)}
          style={inputStyle}
        />
        <select value={filterType} onChange={e => setFilterType(e.target.value)} style={selectStyle}>
          <option value="all">All</option>
          <option value="explicit">Explicit routes</option>
          <option value="default">Using defaults</option>
        </select>
      </div>

      {/* Table */}
      <table style={{ width: '100%', borderCollapse: 'collapse' }}>
        <thead>
          <tr style={{ borderBottom: '1px solid #30363D', fontSize: 11, color: '#8B949E', textAlign: 'left' }}>
            <th style={thStyle}>Strategy</th>
            <th style={thStyle}>Dir</th>
            <th style={thStyle}>Route</th>
            <th style={thStyle}>Broker</th>
            <th style={thStyle}>Account</th>
            <th style={thStyle}>Venue</th>
            <th style={thStyle}>Bot</th>
            <th style={thStyle}>Status</th>
            <th style={thStyle}>Actions</th>
          </tr>
        </thead>
        <tbody>
          {filtered.map(row => {
            const isEditing = editing === row.strategy_name;
            return (
              <tr key={row.strategy_name} style={{ borderBottom: '1px solid #21262D' }}>
                <td style={tdStyle}>
                  <span style={{ fontWeight: 500 }}>{row.strategy_name}</span>
                </td>
                <td style={tdStyle}>
                  <span style={{ color: row.direction === 'short' ? '#F85149' : '#3FB950', fontSize: 11 }}>
                    {row.direction}
                  </span>
                </td>
                <td style={tdStyle}>
                  <span style={{
                    fontSize: 10, padding: '2px 6px', borderRadius: 3,
                    background: row.has_explicit_route ? '#1f3d2a' : '#1c2128',
                    color: row.has_explicit_route ? '#3FB950' : '#8B949E',
                  }}>
                    {row.has_explicit_route ? 'explicit' : 'default'}
                  </span>
                </td>

                {isEditing ? (
                  <>
                    <td style={tdStyle}>
                      <select value={editForm.broker_type} onChange={e => setEditForm(f => ({ ...f, broker_type: e.target.value, venue: VENUE_OPTIONS[e.target.value]?.[0] || 'SMART' }))} style={cellSelectStyle}>
                        {BROKER_OPTIONS.map(b => <option key={b.value} value={b.value}>{b.label}</option>)}
                      </select>
                    </td>
                    <td style={tdStyle}>
                      <input value={editForm.account_id} onChange={e => setEditForm(f => ({ ...f, account_id: e.target.value }))} placeholder="account" style={cellInputStyle} />
                    </td>
                    <td style={tdStyle}>
                      <select value={editForm.venue} onChange={e => setEditForm(f => ({ ...f, venue: e.target.value }))} style={cellSelectStyle}>
                        {venues.map(v => <option key={v} value={v}>{v}</option>)}
                      </select>
                    </td>
                    <td style={tdStyle}>
                      <select value={editForm.bot_name} onChange={e => setEditForm(f => ({ ...f, bot_name: e.target.value }))} style={cellSelectStyle}>
                        {BOT_OPTIONS.map(b => <option key={b} value={b}>{b}</option>)}
                      </select>
                    </td>
                    <td style={tdStyle}>
                      <label style={{ fontSize: 11, cursor: 'pointer' }}>
                        <input type="checkbox" checked={editForm.enabled} onChange={e => setEditForm(f => ({ ...f, enabled: e.target.checked }))} />
                        {' '}{editForm.enabled ? 'on' : 'off'}
                      </label>
                    </td>
                    <td style={tdStyle}>
                      <button onClick={handleSave} disabled={saving} style={{ ...btnSmallStyle, background: '#238636' }}>{saving ? '...' : 'Save'}</button>
                      <button onClick={cancelEdit} style={{ ...btnSmallStyle, marginLeft: 4 }}>Cancel</button>
                    </td>
                  </>
                ) : (
                  <>
                    <td style={tdStyle}>{row.broker_type}</td>
                    <td style={{ ...tdStyle, color: '#8B949E', fontSize: 11 }}>{row.account_id || '—'}</td>
                    <td style={tdStyle}>{row.venue}</td>
                    <td style={{ ...tdStyle, color: '#8B949E' }}>{row.bot_name || '—'}</td>
                    <td style={tdStyle}>
                      <span style={{ color: row.enabled ? '#3FB950' : '#F85149', fontSize: 11 }}>
                        {row.enabled ? 'enabled' : 'disabled'}
                      </span>
                    </td>
                    <td style={tdStyle}>
                      <button onClick={() => startEdit(row)} style={btnSmallStyle}>Edit</button>
                      {row.has_explicit_route && (
                        <button onClick={() => handleDelete(row.strategy_name, row.bot_name)} style={{ ...btnSmallStyle, marginLeft: 4, color: '#F85149' }}>✕</button>
                      )}
                    </td>
                  </>
                )}
              </tr>
            );
          })}
          {filtered.length === 0 && (
            <tr><td colSpan={9} style={{ ...tdStyle, color: '#8B949E', textAlign: 'center' }}>
              {loading ? 'Loading...' : 'No strategies found'}
            </td></tr>
          )}
        </tbody>
      </table>
    </div>
  );
}

const btnStyle = { background: '#21262D', color: '#E6EDF3', border: '1px solid #30363D', borderRadius: 4, padding: '4px 10px', cursor: 'pointer', fontSize: 12 };
const btnSmallStyle = { background: '#21262D', color: '#E6EDF3', border: '1px solid #30363D', borderRadius: 3, padding: '2px 8px', cursor: 'pointer', fontSize: 11 };
const inputStyle = { flex: 1, background: '#0D1117', color: '#E6EDF3', border: '1px solid #30363D', borderRadius: 4, padding: '6px 10px', fontSize: 12, outline: 'none' };
const selectStyle = { background: '#0D1117', color: '#E6EDF3', border: '1px solid #30363D', borderRadius: 4, padding: '6px 8px', fontSize: 12, outline: 'none' };
const cellInputStyle = { background: '#0D1117', color: '#E6EDF3', border: '1px solid #30363D', borderRadius: 3, padding: '3px 6px', fontSize: 11, width: 80, outline: 'none' };
const cellSelectStyle = { background: '#0D1117', color: '#E6EDF3', border: '1px solid #30363D', borderRadius: 3, padding: '3px 4px', fontSize: 11, outline: 'none' };
const thStyle = { padding: '6px 8px', fontWeight: 500 };
const tdStyle = { padding: '6px 8px', fontSize: 12 };
