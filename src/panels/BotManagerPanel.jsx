import { useState } from 'react';
import useLocalBots from '../hooks/useLocalBots';

const STRATEGY_LIST = [
  'Bollinger Reversion', 'Breakout Momentum', 'MACD Crossover', 'RSI Mean Reversion',
  'Dual MA Crossover', 'Keltner Breakout', 'Donchian Channel', 'Stochastic Momentum',
  'Volume Weighted Momentum', 'ATR Trailing Stop', 'Opening Range Breakout',
  'Gap Fill Reversion', 'VWAP Reversion', 'EMA Ribbon Trend', 'Ichimoku Cloud',
  'Parabolic SAR', 'Williams %R Reversal', 'CCI Divergence', 'ADX Trend Strength',
  'Heikin Ashi Smoothed',
];

const DEFAULT_CONFIG = {
  bot_name: '',
  broker: '',
  strategies: [],
  allocation: { max_positions: 5, position_size_pct: 5.0 },
  risk: { max_drawdown_pct: 15.0, daily_loss_limit: 500 },
  signals_url: 'https://auraalpha.cc/api/signals',
};

export default function BotManagerPanel() {
  const {
    configuredBrokers, allBrokers, runningBots,
    message, setMessage, startBot, stopBot, viewLogs, brokerName,
  } = useLocalBots();

  const [form, setForm] = useState({ ...DEFAULT_CONFIG });
  const [starting, setStarting] = useState(false);
  const [showCreate, setShowCreate] = useState(false);
  const [expandedBot, setExpandedBot] = useState(null);
  const [botLogs, setBotLogs] = useState({});
  const [stopping, setStopping] = useState(null);

  const handleFieldChange = (path, value) => {
    setForm(prev => {
      const next = { ...prev };
      const parts = path.split('.');
      if (parts.length === 2) {
        next[parts[0]] = { ...next[parts[0]], [parts[1]]: value };
      } else {
        next[path] = value;
      }
      return next;
    });
  };

  const toggleStrategy = (name) => {
    setForm(prev => {
      const strats = prev.strategies.includes(name)
        ? prev.strategies.filter(s => s !== name)
        : [...prev.strategies, name];
      return { ...prev, strategies: strats };
    });
  };

  const handleStartBot = async () => {
    setMessage(null);
    if (!form.bot_name.trim()) { setMessage({ type: 'error', text: 'Bot name is required.' }); return; }
    if (!form.broker) { setMessage({ type: 'error', text: 'Select a broker.' }); return; }
    if (form.strategies.length === 0) { setMessage({ type: 'error', text: 'Select at least one strategy.' }); return; }
    if (runningBots.some(b => b.bot_name === form.bot_name.trim() && b.running)) {
      setMessage({ type: 'error', text: `Bot "${form.bot_name}" is already running.` }); return;
    }

    setStarting(true);
    try {
      const config = {
        ...form,
        bot_name: form.bot_name.trim(),
        allocation: {
          max_positions: parseInt(form.allocation.max_positions, 10) || 5,
          position_size_pct: parseFloat(form.allocation.position_size_pct) || 5.0,
        },
        risk: {
          max_drawdown_pct: parseFloat(form.risk.max_drawdown_pct) || 15.0,
          daily_loss_limit: parseFloat(form.risk.daily_loss_limit) || 500,
        },
      };
      await startBot(config);
      setForm({ ...DEFAULT_CONFIG });
      setShowCreate(false);
    } catch {
      // message already set by hook
    } finally {
      setStarting(false);
    }
  };

  const handleStop = async (botName) => {
    setStopping(botName);
    try {
      await stopBot(botName);
    } catch {
      // message already set by hook
    } finally {
      setStopping(null);
    }
  };

  const handleViewLogs = async (botName) => {
    if (expandedBot === botName) { setExpandedBot(null); return; }
    const logContent = await viewLogs(botName, 50);
    setBotLogs(prev => ({ ...prev, [botName]: logContent }));
    setExpandedBot(botName);
  };

  const activeCount = runningBots.filter(b => b.running).length;
  const stoppedCount = runningBots.filter(b => !b.running).length;

  return (
    <div className="space-y-2">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3 text-xs text-aura-muted">
          <span><span className="status-dot bg-aura-green mr-1" /><strong className="text-aura-green">{activeCount}</strong> running</span>
          <span><span className="status-dot bg-aura-red mr-1" /><strong className="text-aura-muted">{stoppedCount}</strong> stopped</span>
          <span><strong className="text-aura-text">{configuredBrokers.length}</strong> brokers</span>
        </div>
        <button onClick={() => setShowCreate(prev => !prev)} className="text-xs text-aura-blue hover:text-aura-text">
          {showCreate ? 'Cancel' : '+ New Bot'}
        </button>
      </div>

      {/* Message */}
      {message && (
        <div className={`px-3 py-2 rounded text-xs border ${
          message.type === 'error'
            ? 'bg-aura-red/10 text-aura-red border-aura-red/20'
            : 'bg-aura-green/10 text-aura-green border-aura-green/20'
        }`}>
          {message.text}
        </div>
      )}

      {/* Create bot form */}
      {showCreate && (
        <div className="glass-panel p-3 space-y-2">
          <h3 className="text-xs font-semibold text-aura-text">Create New Bot</h3>

          {/* Name + Broker */}
          <div className="grid grid-cols-2 gap-2">
            <div>
              <label className="block text-[11px] text-aura-muted font-medium mb-1">
                Bot Name <span className="text-aura-red">*</span>
              </label>
              <input
                type="text"
                placeholder="my-alpha-bot"
                value={form.bot_name}
                onChange={(e) => handleFieldChange('bot_name', e.target.value)}
                className="input-field text-xs py-1.5"
              />
            </div>
            <div>
              <label className="block text-[11px] text-aura-muted font-medium mb-1">
                Broker <span className="text-aura-red">*</span>
              </label>
              <select
                value={form.broker}
                onChange={(e) => handleFieldChange('broker', e.target.value)}
                className="input-field text-xs py-1.5"
              >
                <option value="">Select broker...</option>
                {configuredBrokers.map(id => (
                  <option key={id} value={id}>{brokerName(id)}</option>
                ))}
              </select>
              {configuredBrokers.length === 0 && (
                <p className="text-[10px] text-aura-amber mt-0.5">No brokers configured. Set up credentials first.</p>
              )}
            </div>
          </div>

          {/* Allocation + Risk */}
          <div className="grid grid-cols-4 gap-2">
            <div>
              <label className="block text-[11px] text-aura-muted font-medium mb-1">Max Positions</label>
              <input
                type="number" min="1" max="50"
                value={form.allocation.max_positions}
                onChange={(e) => handleFieldChange('allocation.max_positions', e.target.value)}
                className="input-field text-xs py-1.5"
              />
            </div>
            <div>
              <label className="block text-[11px] text-aura-muted font-medium mb-1">Position Size %</label>
              <input
                type="number" min="0.5" max="100" step="0.5"
                value={form.allocation.position_size_pct}
                onChange={(e) => handleFieldChange('allocation.position_size_pct', e.target.value)}
                className="input-field text-xs py-1.5"
              />
            </div>
            <div>
              <label className="block text-[11px] text-aura-muted font-medium mb-1">Max Drawdown %</label>
              <input
                type="number" min="1" max="100" step="0.5"
                value={form.risk.max_drawdown_pct}
                onChange={(e) => handleFieldChange('risk.max_drawdown_pct', e.target.value)}
                className="input-field text-xs py-1.5"
              />
            </div>
            <div>
              <label className="block text-[11px] text-aura-muted font-medium mb-1">Daily Loss $</label>
              <input
                type="number" min="0" step="50"
                value={form.risk.daily_loss_limit}
                onChange={(e) => handleFieldChange('risk.daily_loss_limit', e.target.value)}
                className="input-field text-xs py-1.5"
              />
            </div>
          </div>

          {/* Strategies */}
          <div>
            <label className="block text-[11px] text-aura-muted font-medium mb-1">
              Strategies <span className="text-aura-red">*</span>
              <span className="text-aura-muted font-normal ml-1">({form.strategies.length} selected)</span>
            </label>
            <div className="flex flex-wrap gap-1 max-h-28 overflow-auto p-2 bg-aura-surface2 border border-aura-border rounded-lg">
              {STRATEGY_LIST.map(name => {
                const selected = form.strategies.includes(name);
                return (
                  <button
                    key={name}
                    type="button"
                    onClick={() => toggleStrategy(name)}
                    className={`px-2 py-1 rounded text-[11px] font-medium border transition-colors ${
                      selected
                        ? 'border-aura-blue bg-aura-blue/10 text-aura-blue'
                        : 'border-aura-border text-aura-muted hover:border-aura-blue/50'
                    }`}
                  >
                    {selected ? '\u2713 ' : ''}{name}
                  </button>
                );
              })}
            </div>
          </div>

          {/* Actions */}
          <div className="flex gap-2">
            <button
              className="btn-primary text-xs py-1.5 px-4 bg-aura-green disabled:opacity-50"
              disabled={starting}
              onClick={handleStartBot}
            >
              {starting ? 'Starting...' : 'Start Bot'}
            </button>
            <button
              className="btn-secondary text-xs py-1.5 px-3"
              onClick={() => setShowCreate(false)}
            >
              Cancel
            </button>
          </div>
        </div>
      )}

      {/* Bot list */}
      <div className="space-y-1.5">
        {runningBots.length === 0 && !showCreate && (
          <div className="text-xs text-aura-muted text-center py-6">
            <p>No bots running.</p>
            <p className="mt-1">Click "+ New Bot" to launch one.</p>
          </div>
        )}

        {runningBots.map(bot => {
          const running = bot.running;
          const expanded = expandedBot === bot.bot_name;
          const startedAt = bot.started_at
            ? new Date(bot.started_at * 1000).toLocaleString()
            : null;

          return (
            <div key={bot.bot_name} className="glass-panel overflow-hidden">
              {/* Bot header */}
              <div className="flex items-center justify-between px-3 py-2">
                <div className="flex items-center gap-2 min-w-0">
                  <span className={`status-dot ${running ? 'bg-aura-green' : 'bg-aura-red'}`} />
                  <span className="text-xs font-semibold text-aura-text truncate">{bot.bot_name}</span>
                  <span className={`text-[10px] px-1.5 py-0.5 rounded-full font-semibold uppercase ${
                    running ? 'bg-aura-green/10 text-aura-green' : 'bg-aura-red/10 text-aura-red'
                  }`}>
                    {running ? 'Running' : 'Stopped'}
                  </span>
                </div>
                <div className="flex items-center gap-1.5">
                  {running && (
                    <button
                      className="text-[11px] px-2 py-1 rounded bg-aura-red/10 text-aura-red hover:bg-aura-red/20 disabled:opacity-50"
                      disabled={stopping === bot.bot_name}
                      onClick={() => handleStop(bot.bot_name)}
                    >
                      {stopping === bot.bot_name ? 'Stopping...' : 'Stop'}
                    </button>
                  )}
                  <button
                    className="text-[11px] px-2 py-1 rounded text-aura-muted hover:text-aura-text border border-aura-border hover:border-aura-blue/50"
                    onClick={() => handleViewLogs(bot.bot_name)}
                  >
                    {expanded ? 'Hide Logs' : 'Logs'}
                  </button>
                </div>
              </div>

              {/* Bot details */}
              <div className="flex gap-4 px-3 pb-2 text-[11px] text-aura-muted">
                <span><strong className="text-aura-text">Broker:</strong> {bot.broker || '\u2014'}</span>
                {bot.pid && <span><strong className="text-aura-text">PID:</strong> <span className="font-mono">{bot.pid}</span></span>}
                {startedAt && <span><strong className="text-aura-text">Started:</strong> {startedAt}</span>}
              </div>

              {/* Logs */}
              {expanded && (
                <div className="mx-3 mb-3 bg-aura-bg border border-aura-border rounded-lg p-2 max-h-48 overflow-auto">
                  <pre className="font-mono text-[11px] text-aura-muted whitespace-pre-wrap break-all m-0">
                    {botLogs[bot.bot_name] || 'No log output yet.'}
                  </pre>
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
