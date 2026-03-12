import useLiveBots from '../hooks/useLiveBots';
import MetricTile from '../components/MetricTile';
import StatusDot from '../components/StatusDot';
import { formatCurrency, formatPnl, timeAgo, pnlColor } from '../utils/formatters';

export default function BotCommandPanel() {
  const { bots, loading, totalEquity, totalPnl, totalPositions } = useLiveBots();

  if (loading) return <div className="p-4 text-aura-muted animate-pulse">Loading bots...</div>;

  return (
    <div className="space-y-3">
      {/* Summary row */}
      <div className="grid grid-cols-3 gap-2">
        <MetricTile label="Total Equity" value={formatCurrency(totalEquity)} />
        <MetricTile label="Day P&L" value={formatPnl(totalPnl)} color={pnlColor(totalPnl)} />
        <MetricTile label="Positions" value={totalPositions} />
      </div>

      {/* Bot cards */}
      {bots.map(bot => (
        <div key={bot.key} className="glass-panel p-3">
          <div className="flex items-center justify-between mb-2">
            <div className="flex items-center gap-2">
              <StatusDot color={bot.status === 'OK' ? bot.color : '#F85149'} />
              <span className="font-semibold text-sm" style={{ color: bot.color }}>{bot.name}</span>
              <span className="text-xs px-2 py-0.5 rounded-full" style={{ background: bot.mode === 'LIVE' ? 'rgba(63,185,80,0.15)' : bot.mode === 'IDLE' ? 'rgba(139,148,158,0.15)' : 'rgba(210,153,34,0.15)', color: bot.mode === 'LIVE' ? '#3FB950' : bot.mode === 'IDLE' ? '#8B949E' : '#D29922' }}>
                {bot.mode}
              </span>
            </div>
            <span className="text-xs text-aura-muted">{bot.heartbeat ? timeAgo(bot.heartbeat) : ''}</span>
          </div>

          <div className="grid grid-cols-4 gap-3 text-xs">
            <div>
              <span className="text-aura-muted">Equity</span>
              <div className="font-mono font-medium text-aura-text">{formatCurrency(bot.equity)}</div>
            </div>
            <div>
              <span className="text-aura-muted">Day P&L</span>
              <div className="font-mono font-medium" style={{ color: pnlColor(bot.dayPnl) }}>{formatPnl(bot.dayPnl)}</div>
            </div>
            <div>
              <span className="text-aura-muted">Positions</span>
              <div className="font-mono font-medium text-aura-text">{bot.positionCount}</div>
            </div>
            <div>
              <span className="text-aura-muted">Buying Power</span>
              <div className="font-mono font-medium text-aura-text">{formatCurrency(bot.buyingPower)}</div>
            </div>
          </div>

          {bot.reason && <div className="text-xs text-aura-muted mt-2 italic">{bot.reason}</div>}

          {bot.positions.length > 0 && (
            <div className="mt-2 border-t border-aura-border pt-2">
              <div className="text-xs text-aura-muted mb-1">Open Positions</div>
              {bot.positions.map((pos, i) => (
                <div key={i} className="flex items-center justify-between text-xs py-0.5">
                  <span className="font-mono text-aura-text">{pos.symbol}</span>
                  <span className="font-mono text-aura-muted">{pos.qty} @ {formatCurrency(pos.avg_cost || pos.avg_price || 0)}</span>
                </div>
              ))}
            </div>
          )}
        </div>
      ))}

      {bots.length === 0 && <div className="text-sm text-aura-muted text-center py-8">No bots detected</div>}
    </div>
  );
}
