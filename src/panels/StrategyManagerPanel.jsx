import { useState } from 'react';
import useStrategies from '../hooks/useStrategies';
import DataTable from '../components/DataTable';
import StatusDot from '../components/StatusDot';
import { formatPercent, pnlColor } from '../utils/formatters';

const COLUMNS = [
  { key: 'label', label: 'Strategy', render: (v) => <span className="font-medium text-aura-text truncate max-w-[140px] block">{v}</span> },
  {
    key: 'direction', label: 'Dir',
    render: (v) => (
      <span className={`text-xs px-1.5 py-0.5 rounded ${
        v === 'long' ? 'bg-aura-green/10 text-aura-green'
        : v === 'short' ? 'bg-aura-red/10 text-aura-red'
        : 'bg-aura-amber/10 text-aura-amber'
      }`}>
        {v}
      </span>
    ),
  },
  { key: 'winRate', label: 'Win%', mono: true, render: (v) => <span style={{ color: v > 55 ? '#3FB950' : v < 45 ? '#F85149' : '#8B949E' }}>{v.toFixed(1)}%</span> },
  { key: 'avgSharpe', label: 'Sharpe', mono: true, render: (v) => <span style={{ color: v > 1 ? '#3FB950' : v < 0 ? '#F85149' : '#8B949E' }}>{v.toFixed(2)}</span> },
  { key: 'avgReturn', label: 'Return', mono: true, render: (v) => <span style={{ color: pnlColor(v) }}>{formatPercent(v)}</span> },
  { key: 'totalTrades', label: 'Trades', mono: true, render: (v) => <span className="text-aura-muted">{v}</span> },
  { key: 'suppressed', label: 'Status', render: (_, row) => <StatusDot color={row.suppressed ? 'red' : 'green'} /> },
];

export default function StrategyManagerPanel() {
  const { strategies, loading, assetClass, setAssetClass } = useStrategies();
  const [filter, setFilter] = useState('');

  const filtered = strategies.filter(s =>
    !filter || s.label.toLowerCase().includes(filter.toLowerCase()) || s.name.toLowerCase().includes(filter.toLowerCase())
  );

  if (loading) return <div className="p-4 text-aura-muted animate-pulse">Loading strategies...</div>;

  return (
    <div className="space-y-2">
      <div className="flex items-center gap-2">
        <input
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          className="input-field text-xs flex-1"
          placeholder="Search strategies..."
        />
        <select
          value={assetClass}
          onChange={(e) => setAssetClass(e.target.value)}
          className="text-xs bg-aura-surface2 border border-aura-border rounded px-2 py-1.5 text-aura-text outline-none"
        >
          <option value="us">US Equities</option>
          <option value="crypto">Crypto</option>
          <option value="global_etfs">Global ETFs</option>
        </select>
      </div>

      <div className="text-xs text-aura-muted">{filtered.length} strategies</div>

      <DataTable
        columns={COLUMNS}
        data={filtered}
        defaultSort={{ key: 'avgSharpe', dir: 'desc' }}
      />
    </div>
  );
}
