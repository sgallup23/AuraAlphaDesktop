import { useState, useMemo } from 'react';
import useBacktests from '../hooks/useBacktests';
import DataTable from '../components/DataTable';
import MetricTile from '../components/MetricTile';
import { formatPercent, formatCurrency, formatPnl, pnlColor } from '../utils/formatters';

const COLUMNS = [
  {
    key: 'label', label: 'Strategy',
    render: (v) => <span className="font-medium text-aura-text truncate max-w-[140px] block">{v}</span>,
  },
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
  {
    key: 'count', label: 'Trades', mono: true,
    render: (v) => <span className="text-aura-muted">{v}</span>,
  },
  {
    key: 'winRate', label: 'Win%', mono: true,
    render: (v) => <span style={{ color: v > 50 ? '#3FB950' : v < 50 ? '#F85149' : '#8B949E' }}>{v.toFixed(1)}%</span>,
  },
  {
    key: 'avgReturn', label: 'Avg Return', mono: true,
    render: (v) => <span style={{ color: pnlColor(v) }}>{formatPercent(v)}</span>,
  },
  {
    key: 'totalPnl', label: 'P&L', mono: true,
    render: (v) => <span style={{ color: pnlColor(v) }}>{formatPnl(v)}</span>,
  },
  {
    key: 'profitFactor', label: 'PF', mono: true,
    render: (v) => <span style={{ color: v >= 1.5 ? '#3FB950' : v < 1 ? '#F85149' : '#8B949E' }}>{v.toFixed(2)}</span>,
  },
];

export default function BacktestPanel() {
  const { backtests, loading, region, setRegion } = useBacktests();
  const [filter, setFilter] = useState('');

  const filtered = backtests.filter(s =>
    !filter || s.label.toLowerCase().includes(filter.toLowerCase()) || s.name.toLowerCase().includes(filter.toLowerCase())
  );

  const summary = useMemo(() => {
    if (filtered.length === 0) return { count: 0, avgWinRate: 0, totalPnl: 0 };
    const avgWinRate = filtered.reduce((sum, s) => sum + s.winRate, 0) / filtered.length;
    const totalPnl = filtered.reduce((sum, s) => sum + s.totalPnl, 0);
    return { count: filtered.length, avgWinRate, totalPnl };
  }, [filtered]);

  if (loading) return <div className="p-4 text-aura-muted animate-pulse">Loading backtests...</div>;

  return (
    <div className="space-y-3">
      {/* Summary tiles */}
      <div className="grid grid-cols-3 gap-2">
        <MetricTile label="Total Strategies" value={summary.count} mono={false} />
        <MetricTile
          label="Avg Win Rate"
          value={`${summary.avgWinRate.toFixed(1)}%`}
          color={summary.avgWinRate > 50 ? '#3FB950' : '#F85149'}
        />
        <MetricTile
          label="Total P&L"
          value={formatPnl(summary.totalPnl)}
          color={pnlColor(summary.totalPnl)}
        />
      </div>

      {/* Controls */}
      <div className="flex items-center gap-2">
        <input
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          className="input-field text-xs flex-1"
          placeholder="Search strategies..."
        />
        <select
          value={region}
          onChange={(e) => setRegion(e.target.value)}
          className="text-xs bg-aura-surface2 border border-aura-border rounded px-2 py-1.5 text-aura-text outline-none"
        >
          <option value="us">US Equities</option>
          <option value="crypto">Crypto</option>
          <option value="global_etfs">Global ETFs</option>
          <option value="forex">Forex</option>
        </select>
      </div>

      <div className="text-xs text-aura-muted">{filtered.length} strategies</div>

      <DataTable
        columns={COLUMNS}
        data={filtered}
        defaultSort={{ key: 'avgReturn', dir: 'desc' }}
        emptyText="No backtest results"
      />
    </div>
  );
}
