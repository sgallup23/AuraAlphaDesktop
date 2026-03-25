import { useState, useEffect, useRef, useCallback } from 'react';
import { createChart } from 'lightweight-charts';
import { api } from '../utils/api';
import MetricTile from '../components/MetricTile';
import DataTable from '../components/DataTable';
import { formatCurrency, formatPnl, pnlColor } from '../utils/formatters';

const BOT_COLORS = { SHAWN: '#22d3ee', SHANE: '#a78bfa', NOVA: '#f59e0b' };

const POSITION_COLUMNS = [
  {
    key: 'bot',
    label: 'Bot',
    render: (v) => (
      <span className="font-semibold" style={{ color: BOT_COLORS[v?.toUpperCase()] || '#8B949E' }}>
        {v}
      </span>
    ),
  },
  {
    key: 'symbol',
    label: 'Symbol',
    mono: true,
    render: (v) => <span className="font-medium text-aura-text">{v}</span>,
  },
  {
    key: 'direction',
    label: 'Side',
    render: (v) => (
      <span style={{ color: v === 'LONG' || v === 'BUY' ? '#3FB950' : '#F85149' }}>
        {v}
      </span>
    ),
  },
  { key: 'qty', label: 'Qty', mono: true },
  {
    key: 'entry',
    label: 'Entry',
    mono: true,
    render: (v) => formatCurrency(v),
  },
  {
    key: 'pnl',
    label: 'P&L',
    mono: true,
    render: (v) => <span style={{ color: pnlColor(v) }}>{formatPnl(v)}</span>,
  },
];

function normalizeSector(data) {
  if (Array.isArray(data)) return data;
  if (typeof data === 'object' && data !== null) {
    return Object.entries(data).map(([name, value]) => ({
      name,
      value: typeof value === 'number' ? value : value?.exposure || value?.weight || 0,
    }));
  }
  return [];
}

export default function PortfolioBrainPanel() {
  const [overview, setOverview] = useState(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);
  const chartContainerRef = useRef(null);
  const chartRef = useRef(null);
  const seriesRef = useRef(null);

  // Initialize mini exposure chart
  useEffect(() => {
    if (!chartContainerRef.current) return;
    let chart;
    try {
      chart = createChart(chartContainerRef.current, {
        layout: {
          background: { color: '#0D1117' },
          textColor: '#8B949E',
          fontFamily: "'Plus Jakarta Sans', sans-serif",
          fontSize: 10,
        },
        grid: {
          vertLines: { color: '#1C2128' },
          horzLines: { color: '#1C2128' },
        },
        rightPriceScale: {
          borderColor: '#30363D',
          scaleMargins: { top: 0.1, bottom: 0.1 },
        },
        timeScale: { borderColor: '#30363D', timeVisible: false },
        crosshair: {
          vertLine: { color: '#58A6FF', width: 1, style: 2, labelBackgroundColor: '#161B22' },
          horzLine: { color: '#58A6FF', width: 1, style: 2, labelBackgroundColor: '#161B22' },
        },
        handleScale: { mouseWheel: true },
        handleScroll: { mouseWheel: true },
      });

      const areaSeries = chart.addAreaSeries({
        lineColor: '#58A6FF',
        topColor: 'rgba(88, 166, 255, 0.25)',
        bottomColor: 'rgba(88, 166, 255, 0.02)',
        lineWidth: 2,
        priceLineVisible: false,
        lastValueVisible: true,
      });

      chartRef.current = chart;
      seriesRef.current = areaSeries;

      const ro = new ResizeObserver(entries => {
        for (const entry of entries) {
          chart.applyOptions({ width: entry.contentRect.width, height: entry.contentRect.height });
        }
      });
      ro.observe(chartContainerRef.current);

      return () => {
        ro.disconnect();
        chart.remove();
        chartRef.current = null;
        seriesRef.current = null;
      };
    } catch (err) {
      console.error('Exposure chart init failed:', err);
    }
  }, []);

  const fetchData = useCallback(async () => {
    try {
      const [overviewData, exposureData] = await Promise.all([
        api('/portfolio-brain/overview', { silent: true }),
        api('/intelligence-history/portfolio-snapshots/exposure-timeseries?days=30', { silent: true }),
      ]);

      if (overviewData) {
        setOverview(overviewData);
        setError(null);
      } else {
        setError('No data');
      }

      // Populate exposure mini chart
      if (exposureData && seriesRef.current) {
        const points = Array.isArray(exposureData)
          ? exposureData
          : exposureData?.timeseries || exposureData?.data || exposureData?.points || [];

        const chartData = points
          .map(p => {
            const t = p.time || p.date || p.timestamp;
            const v = p.value ?? p.exposure ?? p.total_exposure ?? p.market_value;
            if (!t || v == null) return null;
            let time;
            if (typeof t === 'string') {
              time = t.slice(0, 10);
            } else {
              time = t > 1e12 ? Math.floor(t / 1000) : t;
            }
            return { time, value: Number(v) };
          })
          .filter(Boolean)
          .sort((a, b) => (a.time < b.time ? -1 : a.time > b.time ? 1 : 0));

        if (chartData.length > 0) {
          seriesRef.current.setData(chartData);
          chartRef.current?.timeScale().fitContent();
        }
      }
    } catch (e) {
      console.warn('[PortfolioBrain] fetch error:', e);
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchData();
    const iv = setInterval(fetchData, 30000);
    return () => clearInterval(iv);
  }, [fetchData]);

  if (loading && !overview) {
    return <div className="p-4 text-aura-muted animate-pulse">Loading portfolio brain...</div>;
  }

  if (error && !overview) {
    return <div className="p-4 text-aura-muted text-sm">Portfolio brain unavailable</div>;
  }

  // Normalize response data
  const positions = (overview?.positions || overview?.cross_bot_positions || []).map(p => ({
    bot: p.bot || p.bot_name || '',
    symbol: p.symbol || '',
    direction: p.direction || p.side || (p.qty > 0 ? 'LONG' : 'SHORT'),
    qty: Math.abs(p.qty || p.quantity || 0),
    entry: p.entry || p.avg_cost || p.avg_price || 0,
    pnl: p.pnl || p.unrealized_pnl || p.current_pnl || 0,
  }));

  const conflicts = overview?.conflicts || overview?.conflict_alerts || [];
  const sectors = normalizeSector(overview?.sectors || overview?.sector_exposure || overview?.exposure_by_sector || {});
  const totalValue = overview?.total_market_value || overview?.total_value || overview?.market_value || 0;
  const totalPnl = overview?.total_pnl || positions.reduce((s, p) => s + p.pnl, 0);
  const positionCount = positions.length;

  return (
    <div className="flex flex-col h-full space-y-2 overflow-auto">
      {/* Summary */}
      <div className="grid grid-cols-3 gap-2 flex-shrink-0">
        <MetricTile label="Market Value" value={formatCurrency(totalValue)} />
        <MetricTile label="Total P&L" value={formatPnl(totalPnl)} color={pnlColor(totalPnl)} />
        <MetricTile label="Positions" value={positionCount} />
      </div>

      {/* Conflict alerts */}
      {conflicts.length > 0 && (
        <div className="glass-panel p-2 flex-shrink-0">
          <div className="flex items-center gap-2 mb-1">
            <span className="text-xs font-medium text-aura-red">Conflicts</span>
            <span className="text-xs px-1.5 py-0.5 rounded-full bg-aura-red/20 text-aura-red font-mono">
              {conflicts.length}
            </span>
          </div>
          {conflicts.slice(0, 5).map((c, i) => (
            <div key={i} className="text-xs text-aura-muted py-0.5 border-b border-aura-border/30 last:border-0">
              <span className="text-aura-red font-mono">{c.symbol || c.pair || ''}</span>
              {' '}
              <span>{c.description || c.message || c.reason || `${c.bot_a || ''} vs ${c.bot_b || ''}`}</span>
            </div>
          ))}
        </div>
      )}

      {/* Cross-bot positions */}
      <div className="flex-shrink-0">
        <DataTable
          columns={POSITION_COLUMNS}
          data={positions}
          defaultSort={{ key: 'pnl', dir: 'desc' }}
          emptyText={loading ? 'Loading...' : 'No positions'}
        />
      </div>

      {/* Sector exposure */}
      {sectors.length > 0 && (
        <div className="glass-panel p-3 flex-shrink-0">
          <div className="text-xs text-aura-muted mb-2 font-medium">Sector Exposure</div>
          <div className="space-y-1">
            {sectors.sort((a, b) => b.value - a.value).slice(0, 8).map((s, i) => {
              const maxVal = sectors[0]?.value || 1;
              const pct = maxVal > 0 ? (s.value / maxVal) * 100 : 0;
              return (
                <div key={s.name || i} className="flex items-center gap-2 text-xs">
                  <span className="text-aura-muted w-24 truncate" title={s.name}>{s.name}</span>
                  <div className="flex-1 h-1.5 rounded-full bg-aura-border overflow-hidden">
                    <div
                      className="h-full rounded-full bg-aura-blue"
                      style={{ width: `${Math.min(pct, 100)}%` }}
                    />
                  </div>
                  <span className="font-mono text-aura-text w-16 text-right">
                    {formatCurrency(s.value)}
                  </span>
                </div>
              );
            })}
          </div>
        </div>
      )}

      {/* Exposure over time mini chart */}
      <div className="flex-shrink-0">
        <div className="text-xs text-aura-muted px-1 mb-1 font-medium">Exposure (30d)</div>
        <div ref={chartContainerRef} className="h-28 min-h-0" />
      </div>
    </div>
  );
}
