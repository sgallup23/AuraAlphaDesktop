import useScanners from '../hooks/useScanners';
import DataTable from '../components/DataTable';
import { formatPercent, pnlColor } from '../utils/formatters';

const COLUMNS = [
  { key: 'symbol', label: 'Symbol', mono: true, render: (v) => <span className="font-medium text-aura-text">{v}</span> },
  { key: 'strategy', label: 'Strategy', render: (v) => <span className="text-aura-muted truncate max-w-[100px] block">{v}</span> },
  { key: 'winRate', label: 'Win%', mono: true, render: (v) => <span style={{ color: v > 55 ? '#3FB950' : v < 45 ? '#F85149' : '#8B949E' }}>{v.toFixed(1)}%</span> },
  { key: 'sharpe', label: 'Sharpe', mono: true, render: (v) => <span style={{ color: v > 1 ? '#3FB950' : v < 0 ? '#F85149' : '#8B949E' }}>{v.toFixed(2)}</span> },
  { key: 'totalReturn', label: 'Return', mono: true, render: (v) => <span style={{ color: pnlColor(v) }}>{formatPercent(v)}</span> },
  {
    key: 'strength', label: 'Signal', render: (v) => (
      <div className="flex items-center gap-1">
        <div className="w-12 h-1.5 rounded-full bg-aura-border overflow-hidden">
          <div className="h-full rounded-full" style={{ width: `${Math.min(v, 100)}%`, background: v > 70 ? '#3FB950' : v > 40 ? '#D29922' : '#F85149' }} />
        </div>
        <span className="font-mono text-aura-muted">{v.toFixed(0)}</span>
      </div>
    ),
  },
];

export default function ScannerPanel() {
  const { presets, selectedPreset, setSelectedPreset, results, loading, runScan } = useScanners();

  return (
    <div className="space-y-2">
      <div className="flex items-center gap-2">
        <select
          value={selectedPreset}
          onChange={(e) => setSelectedPreset(e.target.value)}
          className="flex-1 text-xs bg-aura-surface2 border border-aura-border rounded px-2 py-1.5 text-aura-text outline-none"
        >
          {presets.map(p => <option key={p.key || p.name} value={p.key || p.name}>{p.name || p.key}</option>)}
        </select>
        <button onClick={() => runScan()} disabled={loading} className="btn-primary text-xs py-1">
          {loading ? 'Scanning...' : 'Scan'}
        </button>
      </div>

      <div className="text-xs text-aura-muted">{results.length} results</div>

      <DataTable
        columns={COLUMNS}
        data={results}
        defaultSort={{ key: 'strength', dir: 'desc' }}
        emptyText={loading ? 'Scanning...' : 'No results'}
      />
    </div>
  );
}
