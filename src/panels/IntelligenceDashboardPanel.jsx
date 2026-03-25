import { useState, useEffect, useCallback } from 'react';
import { api } from '../utils/api';
import MetricTile from '../components/MetricTile';
import StatusDot from '../components/StatusDot';

const REGIME_COLORS = {
  risk_on: '#3FB950',
  bullish: '#3FB950',
  neutral: '#D29922',
  cautious: '#D29922',
  risk_off: '#F85149',
  bearish: '#F85149',
};

const REGIME_LABELS = {
  risk_on: 'Risk On',
  bullish: 'Bullish',
  neutral: 'Neutral',
  cautious: 'Cautious',
  risk_off: 'Risk Off',
  bearish: 'Bearish',
};

function regimeColor(regime) {
  return REGIME_COLORS[regime] || '#8B949E';
}

function regimeLabel(regime) {
  return REGIME_LABELS[regime] || regime || 'Unknown';
}

function trendArrow(trend) {
  if (!trend) return '';
  const t = trend.toLowerCase();
  if (t === 'rising' || t === 'up') return ' \u2191';
  if (t === 'falling' || t === 'down') return ' \u2193';
  return ' \u2192';
}

export default function IntelligenceDashboardPanel() {
  const [data, setData] = useState(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);

  const fetchData = useCallback(async () => {
    try {
      const result = await api('/intelligence-history/dashboard', { silent: true });
      if (result) {
        setData(result);
        setError(null);
      } else {
        setError('No data');
      }
    } catch (e) {
      console.warn('[IntelligenceDashboard] fetch error:', e);
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchData();
    const iv = setInterval(fetchData, 60000);
    return () => clearInterval(iv);
  }, [fetchData]);

  if (loading) {
    return <div className="p-4 text-aura-muted animate-pulse">Loading intelligence data...</div>;
  }

  if (error && !data) {
    return <div className="p-4 text-aura-muted text-sm">Intelligence data unavailable</div>;
  }

  const vix = data?.vix || {};
  const fearGreed = data?.fear_greed || data?.fear_and_greed || {};
  const regime = data?.regime || data?.overall_regime || 'unknown';
  const regimeStr = typeof regime === 'string' ? regime : regime?.regime || regime?.overall || 'unknown';
  const markets = data?.markets || data?.per_market || data?.market_regimes || {};
  const coverage = data?.coverage || data?.data_coverage || {};

  const vixLevel = vix.level ?? vix.current ?? vix.value ?? '--';
  const vixTrend = vix.trend || vix.direction || '';
  const fgScore = fearGreed.score ?? fearGreed.value ?? fearGreed.index ?? '--';
  const fgLabel = fearGreed.label ?? fearGreed.classification ?? '';

  const marketEntries = Array.isArray(markets)
    ? markets
    : Object.entries(markets).map(([k, v]) => ({
        market: k,
        ...(typeof v === 'string' ? { regime: v } : v),
      }));

  return (
    <div className="space-y-3">
      {/* Top metrics */}
      <div className="grid grid-cols-3 gap-2">
        <MetricTile
          label={`VIX${trendArrow(vixTrend)}`}
          value={typeof vixLevel === 'number' ? vixLevel.toFixed(2) : vixLevel}
          color={vixLevel > 30 ? '#F85149' : vixLevel > 20 ? '#D29922' : '#3FB950'}
        />
        <MetricTile
          label={`Fear & Greed${fgLabel ? ` (${fgLabel})` : ''}`}
          value={fgScore}
          color={fgScore > 60 ? '#3FB950' : fgScore < 40 ? '#F85149' : '#D29922'}
        />
        <div className="metric-tile">
          <div className="text-xs text-aura-muted mb-1">Overall Regime</div>
          <div className="text-lg font-bold" style={{ color: regimeColor(regimeStr) }}>
            {regimeLabel(regimeStr)}
          </div>
        </div>
      </div>

      {/* Per-market regimes */}
      {marketEntries.length > 0 && (
        <div className="glass-panel p-3">
          <div className="text-xs text-aura-muted mb-2 font-medium">Market Regimes</div>
          <div className="grid grid-cols-2 gap-x-4 gap-y-1.5">
            {marketEntries.slice(0, 10).map((m, i) => {
              const mRegime = m.regime || m.status || 'unknown';
              const mName = m.market || m.name || m.region || `Market ${i + 1}`;
              return (
                <div key={mName} className="flex items-center justify-between text-xs">
                  <div className="flex items-center gap-1.5">
                    <StatusDot color={regimeColor(mRegime)} size="sm" />
                    <span className="text-aura-text capitalize">{mName.replace(/_/g, ' ')}</span>
                  </div>
                  <span className="font-mono" style={{ color: regimeColor(mRegime) }}>
                    {regimeLabel(mRegime)}
                  </span>
                </div>
              );
            })}
          </div>
        </div>
      )}

      {/* Data coverage */}
      {Object.keys(coverage).length > 0 && (
        <div className="glass-panel p-3">
          <div className="text-xs text-aura-muted mb-2 font-medium">Data Coverage</div>
          <div className="grid grid-cols-2 gap-x-4 gap-y-1">
            {Object.entries(coverage).slice(0, 8).map(([key, val]) => (
              <div key={key} className="flex items-center justify-between text-xs">
                <span className="text-aura-muted capitalize">{key.replace(/_/g, ' ')}</span>
                <span className="font-mono text-aura-text">
                  {typeof val === 'number' ? (val > 1 ? val.toLocaleString() : `${(val * 100).toFixed(0)}%`) : String(val)}
                </span>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Last updated */}
      {data?.last_updated && (
        <div className="text-xs text-aura-muted text-right">
          Updated: {new Date(data.last_updated).toLocaleTimeString()}
        </div>
      )}
    </div>
  );
}
