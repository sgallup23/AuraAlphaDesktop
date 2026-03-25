import { useState, useEffect, useCallback } from 'react';
import { api } from '../utils/api';
import MetricTile from '../components/MetricTile';
import StatusDot from '../components/StatusDot';

const BOT_COLORS = { SHAWN: '#22d3ee', SHANE: '#a78bfa', NOVA: '#f59e0b' };

function allocationColor(pct) {
  if (pct == null || isNaN(pct)) return '#8B949E';
  if (pct >= 70) return '#3FB950';
  if (pct >= 40) return '#D29922';
  return '#F85149';
}

function allocationLabel(pct) {
  if (pct == null || isNaN(pct)) return 'N/A';
  return `${Number(pct).toFixed(0)}%`;
}

export default function MetaAllocatorPanel() {
  const [data, setData] = useState(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);

  const fetchData = useCallback(async () => {
    try {
      const result = await api('/meta-allocator/status', { silent: true });
      if (result) {
        setData(result);
        setError(null);
      } else {
        setError('No data');
      }
    } catch (e) {
      console.warn('[MetaAllocator] fetch error:', e);
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

  if (loading && !data) {
    return <div className="p-4 text-aura-muted animate-pulse">Loading meta allocator...</div>;
  }

  if (error && !data) {
    return <div className="p-4 text-aura-muted text-sm">Meta allocator unavailable</div>;
  }

  // Normalize response fields
  const composite = data?.composite_multiplier ?? data?.multiplier ?? data?.composite ?? null;
  const regime = data?.regime ?? data?.market_regime ?? data?.regime_factor ?? null;
  const vixFactor = data?.vix_factor ?? data?.vix ?? data?.vix_multiplier ?? null;
  const fgFactor = data?.fear_greed_factor ?? data?.fg_factor ?? data?.fear_greed ?? null;
  const reasoning = data?.reasoning || data?.reasons || data?.explanation || [];
  const reasonList = Array.isArray(reasoning) ? reasoning : (typeof reasoning === 'string' ? [reasoning] : []);

  // Per-bot allocations
  const botAllocations = data?.bot_allocations || data?.allocations || data?.bots || {};
  const botEntries = Array.isArray(botAllocations)
    ? botAllocations
    : Object.entries(botAllocations).map(([name, val]) => ({
        name: name.toUpperCase(),
        ...(typeof val === 'object' ? val : { allocation: val }),
      }));

  return (
    <div className="space-y-3">
      {/* Composite multiplier */}
      <div className="grid grid-cols-3 gap-2">
        <MetricTile
          label="Composite Multiplier"
          value={composite != null ? `${(composite * 100).toFixed(0)}%` : 'N/A'}
          color={composite != null ? allocationColor(composite * 100) : '#8B949E'}
        />
        <MetricTile
          label="Regime"
          value={regime ? (typeof regime === 'string' ? regime.charAt(0).toUpperCase() + regime.slice(1) : typeof regime === 'number' ? `${(regime * 100).toFixed(0)}%` : 'N/A') : 'N/A'}
          color={typeof regime === 'string' ? (regime === 'bullish' || regime === 'risk_on' ? '#3FB950' : regime === 'bearish' || regime === 'risk_off' ? '#F85149' : '#D29922') : '#8B949E'}
          mono={typeof regime === 'number'}
        />
        <MetricTile
          label="VIX Factor"
          value={vixFactor != null ? (typeof vixFactor === 'number' ? `${(vixFactor * 100).toFixed(0)}%` : vixFactor) : 'N/A'}
          color={typeof vixFactor === 'number' ? allocationColor(vixFactor * 100) : '#8B949E'}
        />
      </div>

      {/* F&G factor */}
      {fgFactor != null && (
        <div className="glass-panel p-3">
          <div className="flex items-center justify-between">
            <span className="text-xs text-aura-muted">Fear & Greed Factor</span>
            <span className="text-sm font-mono font-bold" style={{ color: typeof fgFactor === 'number' ? allocationColor(fgFactor * 100) : '#8B949E' }}>
              {typeof fgFactor === 'number' ? `${(fgFactor * 100).toFixed(0)}%` : fgFactor}
            </span>
          </div>
        </div>
      )}

      {/* Per-bot allocations */}
      {botEntries.length > 0 && (
        <div className="glass-panel p-3">
          <div className="text-xs text-aura-muted mb-2 font-medium">Bot Allocations</div>
          <div className="space-y-2">
            {botEntries.map((bot, i) => {
              const name = bot.name || bot.bot || bot.bot_name || `Bot ${i + 1}`;
              const nameUpper = name.toUpperCase();
              const alloc = bot.allocation ?? bot.pct ?? bot.weight ?? bot.multiplier ?? null;
              const allocPct = alloc != null ? (alloc > 1 ? alloc : alloc * 100) : null;
              const color = allocationColor(allocPct);
              const botColor = BOT_COLORS[nameUpper] || '#58A6FF';

              return (
                <div key={nameUpper}>
                  <div className="flex items-center justify-between mb-1">
                    <div className="flex items-center gap-2">
                      <StatusDot color={botColor} size="sm" />
                      <span className="text-xs font-semibold" style={{ color: botColor }}>{nameUpper}</span>
                    </div>
                    <span className="text-sm font-mono font-bold" style={{ color }}>
                      {allocPct != null ? `${allocPct.toFixed(0)}%` : 'N/A'}
                    </span>
                  </div>
                  {/* Allocation bar */}
                  {allocPct != null && (
                    <div className="h-1.5 rounded-full bg-aura-border overflow-hidden">
                      <div
                        className="h-full rounded-full transition-all duration-500"
                        style={{ width: `${Math.min(allocPct, 100)}%`, background: color }}
                      />
                    </div>
                  )}
                  {/* Bot-specific reasoning */}
                  {bot.reason && (
                    <div className="text-[10px] text-aura-muted mt-0.5 italic">{bot.reason}</div>
                  )}
                </div>
              );
            })}
          </div>
        </div>
      )}

      {/* Reasoning */}
      {reasonList.length > 0 && (
        <div className="glass-panel p-3">
          <div className="text-xs text-aura-muted mb-2 font-medium">Reasoning</div>
          <div className="space-y-1">
            {reasonList.map((r, i) => {
              const text = typeof r === 'string' ? r : (r.message || r.reason || r.text || JSON.stringify(r));
              return (
                <div key={i} className="flex items-start gap-2 text-xs">
                  <span className="text-aura-muted flex-shrink-0 mt-0.5">{'\u2022'}</span>
                  <span className="text-aura-text">{text}</span>
                </div>
              );
            })}
          </div>
        </div>
      )}

      {/* Last updated */}
      {data?.last_updated && (
        <div className="text-[10px] text-aura-muted text-right">
          Updated: {new Date(data.last_updated).toLocaleTimeString()}
        </div>
      )}
    </div>
  );
}
