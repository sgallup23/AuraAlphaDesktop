import { useState, useEffect, useRef, useCallback } from 'react';
import { createChart } from 'lightweight-charts';
import { api } from '../utils/api';
import StatusDot from '../components/StatusDot';

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

const REGIME_SOLID_COLORS = {
  risk_on: '#3FB950',
  bullish: '#3FB950',
  normal: '#58A6FF',
  neutral: '#D29922',
  cautious: '#D29922',
  risk_off: '#F85149',
  bearish: '#F85149',
  crisis: '#DA3633',
};

function regimeBadgeColor(regime) {
  const r = (regime || '').toLowerCase();
  if (r === 'risk_on' || r === 'bullish') return { bg: 'rgba(63,185,80,0.15)', text: '#3FB950' };
  if (r === 'risk_off' || r === 'bearish' || r === 'crisis') return { bg: 'rgba(248,81,73,0.15)', text: '#F85149' };
  if (r === 'normal') return { bg: 'rgba(88,166,255,0.15)', text: '#58A6FF' };
  return { bg: 'rgba(210,153,34,0.15)', text: '#D29922' };
}

function regimeSolidColor(regime) {
  return REGIME_SOLID_COLORS[(regime || '').toLowerCase()] || '#8B949E';
}

export default function RegimePanel() {
  const containerRef = useRef(null);
  const chartRef = useRef(null);
  const seriesRef = useRef(null);
  const [tab, setTab] = useState('summary');
  const [market, setMarket] = useState('us');
  const [summary, setSummary] = useState(null);
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
      const [summaryData, vixData, transData] = await Promise.all([
        api('/intelligence-history/regimes/summary?days=365', { silent: true }),
        api('/intelligence-history/market-intelligence/vix-timeseries?days=365&resolution=daily', { silent: true }),
        api(`/intelligence-history/regimes/transitions?market=${market}&days=365`, { silent: true }),
      ]);

      // Store summary (regime distribution per market)
      if (summaryData) {
        setSummary(summaryData);
      }

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

          // Add regime transition markers
          if (transData) {
            const trans = Array.isArray(transData)
              ? transData
              : transData?.transitions || transData?.data || [];

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
        setTransitions(trans.slice(0, 30));
      }
    } catch (e) {
      console.warn('[RegimePanel] fetch error:', e);
    } finally {
      setLoading(false);
    }
  }, [market]);

  useEffect(() => {
    fetchData();
    const iv = setInterval(fetchData, 300000);
    return () => clearInterval(iv);
  }, [fetchData]);

  // Normalize summary into per-market regime distribution
  const marketSummaries = [];
  if (summary) {
    const markets = summary.markets || summary.per_market || summary.distribution || summary;
    const entries = Array.isArray(markets) ? markets : Object.entries(typeof markets === 'object' ? markets : {});
    for (const entry of entries) {
      if (Array.isArray(entry) && entry.length === 2) {
        const [mKey, mVal] = entry;
        const current = typeof mVal === 'string' ? mVal : (mVal?.current || mVal?.regime || '');
        const dist = typeof mVal === 'object' ? (mVal?.distribution || mVal?.history || mVal) : {};
        marketSummaries.push({ market: mKey, current, distribution: dist });
      } else if (typeof entry === 'object' && entry !== null) {
        marketSummaries.push({
          market: entry.market || entry.name || entry.key || '',
          current: entry.current || entry.regime || '',
          distribution: entry.distribution || entry.history || {},
        });
      }
    }
  }

  return (
    <div className="flex flex-col h-full">
      {/* Toolbar */}
      <div className="flex items-center gap-2 p-2 border-b border-aura-border flex-shrink-0">
        <div className="flex gap-1">
          <button
            onClick={() => setTab('summary')}
            className={`text-xs px-3 py-1 rounded ${tab === 'summary' ? 'bg-aura-blue text-white' : 'text-aura-muted hover:text-aura-text'}`}
          >
            Summary
          </button>
          <button
            onClick={() => setTab('transitions')}
            className={`text-xs px-3 py-1 rounded ${tab === 'transitions' ? 'bg-aura-blue text-white' : 'text-aura-muted hover:text-aura-text'}`}
          >
            Transitions
          </button>
        </div>
        {tab === 'transitions' && (
          <select
            value={market}
            onChange={(e) => setMarket(e.target.value)}
            className="text-xs bg-aura-surface2 border border-aura-border rounded px-2 py-1.5 text-aura-text outline-none"
          >
            {MARKETS.map(m => (
              <option key={m.key} value={m.key}>{m.label}</option>
            ))}
          </select>
        )}
        {loading && <span className="text-xs text-aura-muted animate-pulse ml-auto">Loading...</span>}
      </div>

      {tab === 'summary' ? (
        <div className="flex-1 min-h-0 overflow-auto p-2 space-y-3">
          {/* Regime distribution per market */}
          {marketSummaries.length > 0 ? (
            marketSummaries.map((ms) => {
              const badge = regimeBadgeColor(ms.current);
              // Build distribution bars
              const distEntries = typeof ms.distribution === 'object' && !Array.isArray(ms.distribution)
                ? Object.entries(ms.distribution).filter(([k]) => k !== 'current' && k !== 'regime' && k !== 'market')
                : [];
              const totalDist = distEntries.reduce((s, [, v]) => s + (typeof v === 'number' ? v : 0), 0);

              return (
                <div key={ms.market} className="glass-panel p-3">
                  <div className="flex items-center justify-between mb-2">
                    <div className="flex items-center gap-2">
                      <StatusDot color={regimeSolidColor(ms.current)} size="sm" />
                      <span className="text-xs font-medium text-aura-text capitalize">{ms.market.replace(/_/g, ' ')}</span>
                    </div>
                    <span
                      className="text-xs font-mono px-2 py-0.5 rounded-full capitalize"
                      style={{ background: badge.bg, color: badge.text }}
                    >
                      {(ms.current || 'unknown').replace(/_/g, ' ')}
                    </span>
                  </div>
                  {distEntries.length > 0 && totalDist > 0 && (
                    <>
                      {/* Stacked bar */}
                      <div className="flex h-2 rounded-full overflow-hidden mb-1.5">
                        {distEntries.map(([regime, count]) => {
                          const pct = totalDist > 0 ? (count / totalDist) * 100 : 0;
                          if (pct < 1) return null;
                          return (
                            <div
                              key={regime}
                              className="h-full"
                              style={{ width: `${pct}%`, background: regimeSolidColor(regime) }}
                              title={`${regime}: ${count} days (${pct.toFixed(0)}%)`}
                            />
                          );
                        })}
                      </div>
                      {/* Legend */}
                      <div className="flex flex-wrap gap-x-3 gap-y-0.5">
                        {distEntries.map(([regime, count]) => {
                          const pct = totalDist > 0 ? (count / totalDist) * 100 : 0;
                          return (
                            <div key={regime} className="flex items-center gap-1 text-[10px]">
                              <span className="w-2 h-2 rounded-full inline-block" style={{ background: regimeSolidColor(regime) }} />
                              <span className="text-aura-muted capitalize">{regime.replace(/_/g, ' ')}</span>
                              <span className="font-mono text-aura-text">{pct.toFixed(0)}%</span>
                            </div>
                          );
                        })}
                      </div>
                    </>
                  )}
                </div>
              );
            })
          ) : (
            <div className="text-xs text-aura-muted text-center py-6">
              {loading ? 'Loading regime summary...' : 'No regime summary data available'}
            </div>
          )}
        </div>
      ) : (
        <div className="flex flex-col flex-1 min-h-0">
          {/* VIX Chart */}
          {chartError ? (
            <div className="h-48 flex items-center justify-center text-xs text-aura-muted flex-shrink-0">
              Chart unavailable
            </div>
          ) : (
            <div ref={containerRef} className="h-48 min-h-0 flex-shrink-0" />
          )}

          {/* Regime transitions list */}
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
      )}
    </div>
  );
}
