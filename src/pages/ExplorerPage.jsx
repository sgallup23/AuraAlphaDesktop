import { useState, useEffect, useCallback, useMemo, memo } from 'react';
import { invoke } from '@tauri-apps/api/core';

// Format a decimal as percentage string
function pct(value, decimals = 1) {
  if (value == null || isNaN(value)) return '--';
  return `${(value * 100).toFixed(decimals)}%`;
}

// Format a number with commas
function fmt(value, decimals = 2) {
  if (value == null || isNaN(value)) return '--';
  return value.toLocaleString('en-US', {
    minimumFractionDigits: decimals,
    maximumFractionDigits: decimals,
  });
}

const MetricCard = memo(function MetricCard({ label, value, subtitle, positive }) {
  const color =
    positive === true
      ? '#3FB950'
      : positive === false
      ? '#F85149'
      : '#E6EDF3';
  return (
    <div
      style={{
        background: '#161B22',
        border: '1px solid #30363D',
        borderRadius: 12,
        padding: '16px 20px',
        flex: '1 1 0',
        minWidth: 150,
      }}
    >
      <div
        style={{ fontSize: 12, color: '#8B949E', marginBottom: 6, fontWeight: 500 }}
      >
        {label}
      </div>
      <div style={{ fontSize: 28, fontWeight: 700, color, lineHeight: 1.1 }}>
        {value}
      </div>
      {subtitle && (
        <div style={{ fontSize: 11, color: '#8B949E', marginTop: 4 }}>
          {subtitle}
        </div>
      )}
    </div>
  );
});

const SymbolRow = memo(function SymbolRow({ result, expanded, onToggle }) {
  const m = result.metrics;
  const returnColor = m.total_return >= 0 ? '#3FB950' : '#F85149';
  const sharpeColor = m.sharpe >= 0.5 ? '#3FB950' : m.sharpe >= 0 ? '#D29922' : '#F85149';

  return (
    <>
      <tr
        onClick={onToggle}
        style={{
          cursor: 'pointer',
          borderBottom: expanded ? 'none' : '1px solid #21262D',
        }}
      >
        <td style={{ padding: '10px 12px', fontWeight: 600 }}>{result.symbol}</td>
        <td style={{ padding: '10px 12px', color: returnColor, fontWeight: 600 }}>
          {pct(m.total_return)}
        </td>
        <td style={{ padding: '10px 12px', color: sharpeColor }}>{fmt(m.sharpe)}</td>
        <td style={{ padding: '10px 12px' }}>{fmt(m.win_rate, 1)}%</td>
        <td style={{ padding: '10px 12px' }}>{m.num_trades}</td>
        <td style={{ padding: '10px 12px', color: '#F85149' }}>{pct(m.max_drawdown)}</td>
        <td style={{ padding: '10px 12px' }}>{fmt(m.avg_hold_days, 0)}d</td>
        <td style={{ padding: '10px 8px', fontSize: 13 }}>
          {expanded ? '\u25B2' : '\u25BC'}
        </td>
      </tr>
      {expanded && (
        <tr style={{ borderBottom: '1px solid #21262D' }}>
          <td colSpan={8} style={{ padding: '8px 12px 14px' }}>
            <div
              style={{
                display: 'flex',
                gap: 16,
                flexWrap: 'wrap',
                fontSize: 12,
                color: '#8B949E',
              }}
            >
              <span>
                Profit Factor: <b style={{ color: '#E6EDF3' }}>{fmt(m.profit_factor)}</b>
              </span>
              <span>
                Sortino: <b style={{ color: '#E6EDF3' }}>{fmt(m.sortino)}</b>
              </span>
              <span>
                Wins: <b style={{ color: '#3FB950' }}>{m.wins}</b> / Losses:{' '}
                <b style={{ color: '#F85149' }}>{m.losses}</b>
              </span>
              <span>
                Avg Return: <b style={{ color: '#E6EDF3' }}>{pct(m.avg_return, 2)}</b>
              </span>
              {m.exit_reasons &&
                Object.entries(m.exit_reasons).map(([reason, count]) => (
                  <span key={reason}>
                    {reason}: <b style={{ color: '#E6EDF3' }}>{count}</b>
                  </span>
                ))}
            </div>
            {result.trades && result.trades.length > 0 && (
              <div style={{ marginTop: 8, fontSize: 11, color: '#8B949E' }}>
                First trade: {result.trades[0].entry_date} | Last trade:{' '}
                {result.trades[result.trades.length - 1].exit_date}
              </div>
            )}
          </td>
        </tr>
      )}
    </>
  );
});

const TABLE_HEADERS = ['Symbol', 'Return', 'Sharpe', 'Win Rate', 'Trades', 'Max DD', 'Avg Hold', ''];

// Wrapper to stabilize onToggle callback per row, preventing all SymbolRows from re-rendering on expand
const SymbolRowWrapper = memo(function SymbolRowWrapper({ result, expandedSymbol, setExpandedSymbol }) {
  const expanded = expandedSymbol === result.symbol;
  const onToggle = useCallback(() => {
    setExpandedSymbol((prev) => (prev === result.symbol ? null : result.symbol));
  }, [result.symbol, setExpandedSymbol]);

  return <SymbolRow result={result} expanded={expanded} onToggle={onToggle} />;
});

function ExplorerPage({ onSignIn }) {
  const [phase, setPhase] = useState('idle'); // idle | running | done | error
  const [data, setData] = useState(null);
  const [error, setError] = useState('');
  const [expandedSymbol, setExpandedSymbol] = useState(null);
  const [progress, setProgress] = useState('');

  const runDemo = useCallback(async () => {
    setPhase('running');
    setError('');
    setProgress('Loading bundled market data...');

    // Small delay so the user sees the animation
    await new Promise((r) => setTimeout(r, 400));
    setProgress('Running SMA crossover backtest on 10 symbols...');

    try {
      const result = await invoke('run_demo_backtest');
      setData(result);
      setPhase('done');
    } catch (err) {
      if (import.meta.env.DEV) console.error('[Explorer] Demo backtest failed:', err);
      setError(String(err));
      setPhase('error');
    }
  }, []);

  // Auto-run on mount
  useEffect(() => {
    runDemo();
  }, [runDemo]);

  return (
    <div
      style={{
        minHeight: '100vh',
        background: '#0D1117',
        color: '#E6EDF3',
        fontFamily: "'Plus Jakarta Sans', system-ui, sans-serif",
        overflow: 'auto',
      }}
    >
      {/* Header */}
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          padding: '16px 24px',
          borderBottom: '1px solid #21262D',
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
          <div
            style={{
              width: 36,
              height: 36,
              borderRadius: 10,
              background: 'linear-gradient(135deg, #58A6FF, #BC8CFF, #3FB950)',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
            }}
          >
            <span style={{ color: '#fff', fontSize: 18, fontWeight: 700 }}>A</span>
          </div>
          <div>
            <div style={{ fontWeight: 700, fontSize: 16 }}>Aura Alpha</div>
            <div style={{ fontSize: 11, color: '#8B949E' }}>
              Explorer Mode -- Computing on your hardware
            </div>
          </div>
        </div>
        <button
          onClick={onSignIn}
          style={{
            padding: '8px 20px',
            background: 'linear-gradient(135deg, #58A6FF, #BC8CFF)',
            border: 'none',
            borderRadius: 8,
            color: '#fff',
            fontWeight: 600,
            fontSize: 13,
            cursor: 'pointer',
          }}
        >
          Sign In for Full Access
        </button>
      </div>

      {/* Content */}
      <div style={{ maxWidth: 1100, margin: '0 auto', padding: '24px 24px 48px' }}>
        {/* Running state */}
        {phase === 'running' && (
          <div
            style={{
              textAlign: 'center',
              padding: '80px 0',
            }}
          >
            <div
              style={{
                width: 64,
                height: 64,
                borderRadius: 16,
                background: 'linear-gradient(135deg, #58A6FF, #BC8CFF, #3FB950)',
                display: 'inline-flex',
                alignItems: 'center',
                justifyContent: 'center',
                marginBottom: 20,
                animation: 'explorer-pulse 1.5s ease-in-out infinite',
              }}
            >
              <span style={{ color: '#fff', fontSize: 32, fontWeight: 700 }}>A</span>
            </div>
            <div style={{ fontSize: 20, fontWeight: 700, marginBottom: 8 }}>
              {progress}
            </div>
            <div style={{ fontSize: 14, color: '#8B949E', marginBottom: 24 }}>
              5 years of daily data across 10 major stocks -- all computed locally
            </div>
            <div
              style={{
                width: 280,
                height: 3,
                background: '#30363D',
                borderRadius: 2,
                overflow: 'hidden',
                margin: '0 auto',
              }}
            >
              <div
                style={{
                  width: '40%',
                  height: '100%',
                  background: 'linear-gradient(90deg, #58A6FF, #BC8CFF)',
                  borderRadius: 2,
                  animation: 'explorer-slide 1.5s infinite',
                }}
              />
            </div>
          </div>
        )}

        {/* Error state */}
        {phase === 'error' && (
          <div style={{ textAlign: 'center', padding: '80px 0' }}>
            <div style={{ fontSize: 48, marginBottom: 16 }}>&#x26A0;</div>
            <div style={{ fontSize: 18, fontWeight: 600, marginBottom: 8 }}>
              Demo backtest failed
            </div>
            <div
              style={{
                fontSize: 13,
                color: '#8B949E',
                marginBottom: 20,
                maxWidth: 400,
                margin: '0 auto 20px',
              }}
            >
              {error}
            </div>
            <button
              onClick={runDemo}
              style={{
                padding: '10px 24px',
                background: '#58A6FF',
                border: 'none',
                borderRadius: 8,
                color: '#0D1117',
                fontWeight: 600,
                cursor: 'pointer',
                fontSize: 14,
              }}
            >
              Retry
            </button>
          </div>
        )}

        {/* Results */}
        {phase === 'done' && data && (
          <>
            {/* Hero banner */}
            <div
              style={{
                background: 'linear-gradient(135deg, #161B22, #1C2333)',
                border: '1px solid #30363D',
                borderRadius: 16,
                padding: '24px 28px',
                marginBottom: 24,
              }}
            >
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', flexWrap: 'wrap', gap: 16 }}>
                <div>
                  <div style={{ fontSize: 13, color: '#8B949E', marginBottom: 4 }}>
                    SMA Crossover (10/30) -- 5 Years of Daily Data
                  </div>
                  <div style={{ fontSize: 24, fontWeight: 700 }}>
                    Demo Backtest Results
                  </div>
                </div>
                <div
                  style={{
                    fontSize: 12,
                    color: '#8B949E',
                    background: '#0D1117',
                    padding: '6px 12px',
                    borderRadius: 8,
                    border: '1px solid #30363D',
                  }}
                >
                  {data.symbols_processed} symbols | {data.total_bars.toLocaleString()} bars |{' '}
                  {data.elapsed_ms}ms
                </div>
              </div>
            </div>

            {/* Summary metrics */}
            <div
              style={{
                display: 'flex',
                gap: 12,
                marginBottom: 24,
                flexWrap: 'wrap',
              }}
            >
              <MetricCard
                label="Total Trades"
                value={data.summary.total_trades}
                subtitle="across all symbols"
              />
              <MetricCard
                label="Avg Sharpe Ratio"
                value={fmt(data.summary.avg_sharpe)}
                positive={data.summary.avg_sharpe > 0}
              />
              <MetricCard
                label="Avg Win Rate"
                value={`${fmt(data.summary.avg_win_rate, 1)}%`}
                positive={data.summary.avg_win_rate > 50}
              />
              <MetricCard
                label="Best Performer"
                value={data.summary.best_symbol}
                subtitle={pct(data.summary.best_return)}
                positive={true}
              />
              <MetricCard
                label="Worst Performer"
                value={data.summary.worst_symbol}
                subtitle={pct(data.summary.worst_return)}
                positive={data.summary.worst_return >= 0}
              />
            </div>

            {/* Results table */}
            <div
              style={{
                background: '#161B22',
                border: '1px solid #30363D',
                borderRadius: 12,
                overflow: 'hidden',
                marginBottom: 24,
              }}
            >
              <table
                style={{
                  width: '100%',
                  borderCollapse: 'collapse',
                  fontSize: 13,
                }}
              >
                <thead>
                  <tr style={{ borderBottom: '1px solid #30363D' }}>
                    {TABLE_HEADERS.map(
                      (h) => (
                        <th
                          key={h}
                          style={{
                            padding: '10px 12px',
                            textAlign: 'left',
                            fontWeight: 600,
                            fontSize: 11,
                            color: '#8B949E',
                            textTransform: 'uppercase',
                            letterSpacing: '0.5px',
                          }}
                        >
                          {h}
                        </th>
                      ),
                    )}
                  </tr>
                </thead>
                <tbody>
                  {data.results.map((r) => (
                    <SymbolRowWrapper
                      key={r.symbol}
                      result={r}
                      expandedSymbol={expandedSymbol}
                      setExpandedSymbol={setExpandedSymbol}
                    />
                  ))}
                </tbody>
              </table>
            </div>

            {/* CTA */}
            <div
              style={{
                background: 'linear-gradient(135deg, #161B22, #1C2333)',
                border: '1px solid #30363D',
                borderRadius: 12,
                padding: '24px 28px',
                textAlign: 'center',
              }}
            >
              <div style={{ fontSize: 18, fontWeight: 700, marginBottom: 8 }}>
                This was 1 strategy out of 83
              </div>
              <div
                style={{
                  fontSize: 14,
                  color: '#8B949E',
                  marginBottom: 20,
                  maxWidth: 500,
                  margin: '0 auto 20px',
                }}
              >
                Sign in to access AI-powered strategy selection, live market data,
                automated trading bots, and backtesting across 600+ symbols.
              </div>
              <div style={{ display: 'flex', gap: 12, justifyContent: 'center' }}>
                <button
                  onClick={onSignIn}
                  style={{
                    padding: '12px 32px',
                    background: 'linear-gradient(135deg, #58A6FF, #BC8CFF)',
                    border: 'none',
                    borderRadius: 10,
                    color: '#fff',
                    fontWeight: 700,
                    fontSize: 15,
                    cursor: 'pointer',
                  }}
                >
                  Sign In for Full Access
                </button>
                <button
                  onClick={runDemo}
                  style={{
                    padding: '12px 24px',
                    background: 'transparent',
                    border: '1px solid #30363D',
                    borderRadius: 10,
                    color: '#8B949E',
                    fontWeight: 600,
                    fontSize: 14,
                    cursor: 'pointer',
                  }}
                >
                  Run Again
                </button>
              </div>
            </div>

            {/* Disclaimer */}
            <div
              style={{
                fontSize: 11,
                color: '#484F58',
                textAlign: 'center',
                marginTop: 16,
                lineHeight: 1.5,
              }}
            >
              Demo uses historical data bundled at build time. Past performance does
              not guarantee future results. This is not financial advice.
            </div>
          </>
        )}
      </div>

      <style>{`
        @keyframes explorer-pulse {
          0%, 100% { transform: scale(1); opacity: 1; }
          50% { transform: scale(1.05); opacity: 0.85; }
        }
        @keyframes explorer-slide {
          0% { transform: translateX(-100%); }
          100% { transform: translateX(350%); }
        }
      `}</style>
    </div>
  );
}

export default memo(ExplorerPage);
