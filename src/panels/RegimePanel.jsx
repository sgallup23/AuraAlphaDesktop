import { useState, useEffect, useRef, useCallback } from 'react';
import { createChart } from 'lightweight-charts';
import { api } from '../utils/api';
import { timeAgo } from '../utils/formatters';

const MARKETS = [
  { key: 'us', label: 'US Equities' },
  { key: 'crypto', label: 'Crypto' },
  { key: 'global_etfs', label: 'Global ETFs' },
  { key: 'europe_lse', label: 'Europe (LSE)' },
  { key: 'asia', label: 'Asia' },
  { key: 'commodities', label: 'Commodities' },
  { key: 'forex', label: 'Forex' },
  { key: 'bonds', label: 'Bonds' },
];

const REGIME_COLORS = {
  risk_on: 'rgba(63, 185, 80, 0.12)',
  bullish: 'rgba(63, 185, 80, 0.12)',
  neutral: 'rgba(210, 153, 34, 0.12)',
  cautious: 'rgba(210, 153, 34, 0.12)',
  risk_off: 'rgba(248, 81, 73, 0.12)',
  bearish: 'rgba(248, 81, 73, 0.12)',
};

const REGIME_BORDER_COLORS = {
  risk_on: '#3FB950',
  bullish: '#3FB950',
  neutral: '#D29922',
  cautious: '#D29922',
  risk_off: '#F85149',
  bearish: '#F85149',
};

function regimeBadgeColor(regime) {
  const r = (regime || '').toLowerCase();
  if (r === 'risk_on' || r === 'bullish') return { bg: 'rgba(63,185,80,0.15)', text: '#3FB950' };
  if (r === 'risk_off' || r === 'bearish') return { bg: 'rgba(248,81,73,0.15)', text: '#F85149' };
  return { bg: 'rgba(210,153,34,0.15)', text: '#D29922' };
}

export default function RegimePanel() {
  const containerRef = useRef(null);
  const chartRef = useRef(null);
  const seriesRef = useRef(null);
  const [market, setMarket] = useState('us');
  const [transitions, setTransitions] = useState([]);
  const [loading, setLoading] = useState(true);
  const [chartError, setChartError] = useState(null);

  // Initialize chart once
  useEffect(() => {
    if (!containerRef.current) return;
    let chart;
    try {
      chart = createChart(containerRef.current, {
        layout: {
          background: { color: '#0D1117' },
          textColor: '#8B949E',
          fontFamily: "'Plus Jakarta Sans', sans-serif",
          fontSize: 11,
        },
        grid: {
          vertLines: { color: '#1C2128' },
          horzLines: { color: '#1C2128' },
        },
        crosshair: {
          mode: 0,
          vertLine: { color: '#58A6FF', width: 1, style: 2, labelBackgroundColor: '#161B22' },
          horzLine: { color: '#58A6FF', width: 1, style: 2, labelBackgroundColor: '#161B22' },
        },
        rightPriceScale: {
          borderColor: '#30363D',
          scaleMargins: { top: 0.05, bottom: 0.05 },
        },
        timeScale: { borderColor: '#30363D', timeVisible: false },
        handleScale: { mouseWheel: true, pinch: true },
        handleScroll: { mouseWheel: true, pressedMouseMove: true },
      });

      const lineSeries = chart.addLineSeries({
        color: '#58A6FF',
        lineWidth: 2,
        priceLineVisible: false,
        lastValueVisible: true,
      });

      chartRef.current = chart;
      seriesRef.current = lineSeries;

      const ro = new ResizeObserver(entries => {
        for (const entry of entries) {
          chart.applyOptions({ width: entry.contentRect.width, height: entry.contentRect.height });
        }
      });
      ro.observe(containerRef.current);

      return () => {
        ro.disconnect();
        chart.remove();
        chartRef.current = null;
        seriesRef.current = null;
      };
    } catch (err) {
      console.error('Regime chart init failed:', err);
      setChartError(String(err));
    }
  }, []);

  const fetchData = useCallback(async () => {
    setLoading(true);
    try {
      const [vixData, transData] = await Promise.all([
        api('/intelligence-history/market-intelligence/vix-timeseries?days=365&resolution=daily', { silent: true }),
        api(`/intelligence-history/regimes/transitions?market=${market}&days=365`, { silent: true }),
      ]);

      // Process VIX timeseries for chart
      if (vixData && seriesRef.current) {
        const points = Array.isArray(vixData)
          ? vixData
          : vixData?.timeseries || vixData?.data || vixData?.points || [];

        const chartData = points
          .map(p => {
            const t = p.time || p.date || p.timestamp;
            const v = p.value ?? p.close ?? p.vix ?? p.level;
            if (!t || v == null) return null;
            // Convert to lightweight-charts time format (YYYY-MM-DD or unix timestamp)
            let time;
            if (typeof t === 'string') {
              time = t.slice(0, 10); // YYYY-MM-DD
            } else {
              time = t > 1e12 ? Math.floor(t / 1000) : t;
            }
            return { time, value: Number(v) };
          })
          .filter(Boolean)
          .sort((a, b) => (a.time < b.time ? -1 : a.time > b.time ? 1 : 0));

        if (chartData.length > 0) {
          seriesRef.current.setData(chartData);

          // Add regime color bands as markers
          if (transData) {
            const trans = Array.isArray(transData)
              ? transData
              : transData?.transitions || transData?.data || [];

            // Create colored horizontal lines at VIX thresholds
            const markers = [];
            trans.forEach(t => {
              const date = (t.date || t.time || t.timestamp || '').slice(0, 10);
              const regime = t.regime || t.to_regime || t.new_regime || '';
              const badgeCol = regimeBadgeColor(regime);
              if (date) {
                markers.push({
                  time: date,
                  position: 'aboveBar',
                  color: badgeCol.text,
                  shape: 'circle',
                  size: 0.5,
                });
              }
            });
            if (markers.length > 0) {
              seriesRef.current.setMarkers(markers.sort((a, b) => (a.time < b.time ? -1 : 1)));
            }
          }

          chartRef.current?.timeScale().fitContent();
        }
      }

      // Process transitions list
      if (transData) {
        const trans = Array.isArray(transData)
          ? transData
          : transData?.transitions || transData?.data || [];
        setTransitions(trans.slice(0, 20));
      }
    } catch (e) {
      console.warn('[RegimePanel] fetch error:', e);
    } finally {
      setLoading(false);
    }
  }, [market]);

  useEffect(() => {
    fetchData();
    const iv = setInterval(fetchData, 300000); // 5 min
    return () => clearInterval(iv);
  }, [fetchData]);

  return (
    <div className="flex flex-col h-full">
      {/* Toolbar */}
      <div className="flex items-center gap-2 p-2 border-b border-aura-border flex-shrink-0">
        <select
          value={market}
          onChange={(e) => setMarket(e.target.value)}
          className="flex-1 text-xs bg-aura-surface2 border border-aura-border rounded px-2 py-1.5 text-aura-text outline-none"
        >
          {MARKETS.map(m => (
            <option key={m.key} value={m.key}>{m.label}</option>
          ))}
        </select>
        {loading && <span className="text-xs text-aura-muted animate-pulse">Loading...</span>}
      </div>

      {/* VIX Chart */}
      {chartError ? (
        <div className="h-48 flex items-center justify-center text-xs text-aura-muted">
          Chart unavailable
        </div>
      ) : (
        <div ref={containerRef} className="h-48 min-h-0 flex-shrink-0" />
      )}

      {/* Regime transitions */}
      <div className="flex-1 min-h-0 overflow-auto border-t border-aura-border">
        <div className="p-2">
          <div className="text-xs text-aura-muted mb-2 font-medium">Regime Transitions</div>
          {transitions.length === 0 && !loading && (
            <div className="text-xs text-aura-muted text-center py-4">No transitions found</div>
          )}
          <div className="space-y-1">
            {transitions.map((t, i) => {
              const from = t.from_regime || t.from || t.previous || '';
              const to = t.to_regime || t.regime || t.new_regime || t.to || '';
              const date = t.date || t.time || t.timestamp || '';
              const reason = t.reason || t.trigger || '';
              const badge = regimeBadgeColor(to);

              return (
                <div key={i} className="flex items-center justify-between text-xs py-1 border-b border-aura-border/30">
                  <div className="flex items-center gap-2">
                    <span className="text-aura-muted font-mono w-20 flex-shrink-0">
                      {typeof date === 'string' ? date.slice(0, 10) : new Date(date * 1000).toLocaleDateString()}
                    </span>
                    {from && (
                      <>
                        <span style={{ color: regimeBadgeColor(from).text }} className="capitalize">
                          {from.replace(/_/g, ' ')}
                        </span>
                        <span className="text-aura-muted">{'\u2192'}</span>
                      </>
                    )}
                    <span
                      className="px-1.5 py-0.5 rounded text-xs capitalize"
                      style={{ background: badge.bg, color: badge.text }}
                    >
                      {to.replace(/_/g, ' ')}
                    </span>
                  </div>
                  {reason && (
                    <span className="text-aura-muted truncate max-w-[120px]" title={reason}>
                      {reason}
                    </span>
                  )}
                </div>
              );
            })}
          </div>
        </div>
      </div>
    </div>
  );
}
