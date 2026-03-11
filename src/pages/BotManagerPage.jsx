import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { theme, styles } from '../theme';
import BotStatusCard from '../components/BotStatusCard';

// Hardcoded top strategies for the multi-select
const STRATEGY_LIST = [
  'Bollinger Reversion',
  'Breakout Momentum',
  'MACD Crossover',
  'RSI Mean Reversion',
  'Dual MA Crossover',
  'Keltner Breakout',
  'Donchian Channel',
  'Stochastic Momentum',
  'Volume Weighted Momentum',
  'ATR Trailing Stop',
  'Opening Range Breakout',
  'Gap Fill Reversion',
  'VWAP Reversion',
  'EMA Ribbon Trend',
  'Ichimoku Cloud',
  'Parabolic SAR',
  'Williams %R Reversal',
  'CCI Divergence',
  'ADX Trend Strength',
  'Heikin Ashi Smoothed',
];

const DEFAULT_CONFIG = {
  bot_name: '',
  broker: '',
  strategies: [],
  allocation: {
    max_positions: 5,
    position_size_pct: 5.0,
  },
  risk: {
    max_drawdown_pct: 15.0,
    daily_loss_limit: 500,
  },
  signals_url: 'https://auraalpha.cc/api/signals',
};

export default function BotManagerPage() {
  const [configuredBrokers, setConfiguredBrokers] = useState([]);
  const [allBrokers, setAllBrokers] = useState([]);
  const [runningBots, setRunningBots] = useState([]);
  const [form, setForm] = useState({ ...DEFAULT_CONFIG });
  const [starting, setStarting] = useState(false);
  const [message, setMessage] = useState(null);
  const [showCreate, setShowCreate] = useState(false);

  const loadData = useCallback(async () => {
    try {
      const [configured, available, bots] = await Promise.all([
        invoke('list_configured_brokers'),
        invoke('get_available_brokers'),
        invoke('list_local_bots'),
      ]);
      setConfiguredBrokers(configured);
      setAllBrokers(available);
      setRunningBots(bots);
    } catch (err) {
      console.error('Load error:', err);
      setMessage({ type: 'error', text: 'Failed to load data: ' + err });
    }
  }, []);

  useEffect(() => {
    loadData();
  }, [loadData]);

  // Get broker display name by id
  const brokerName = (id) => {
    const b = allBrokers.find((b) => b.id === id);
    return b ? b.name : id;
  };

  const handleFieldChange = (path, value) => {
    setForm((prev) => {
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
    setForm((prev) => {
      const strats = prev.strategies.includes(name)
        ? prev.strategies.filter((s) => s !== name)
        : [...prev.strategies, name];
      return { ...prev, strategies: strats };
    });
  };

  const handleStartBot = async () => {
    setMessage(null);

    // Validate
    if (!form.bot_name.trim()) {
      setMessage({ type: 'error', text: 'Bot name is required.' });
      return;
    }
    if (!form.broker) {
      setMessage({ type: 'error', text: 'Select a broker.' });
      return;
    }
    if (form.strategies.length === 0) {
      setMessage({ type: 'error', text: 'Select at least one strategy.' });
      return;
    }

    // Check for duplicate name
    if (runningBots.some((b) => b.bot_name === form.bot_name.trim() && b.running)) {
      setMessage({ type: 'error', text: `Bot "${form.bot_name}" is already running.` });
      return;
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

      const result = await invoke('start_bot', { config });
      setMessage({
        type: 'success',
        text: `Bot "${result.bot_name}" started (PID ${result.pid}).`,
      });

      // Refresh running bots
      const bots = await invoke('list_local_bots');
      setRunningBots(bots);

      // Reset form
      setForm({ ...DEFAULT_CONFIG });
      setShowCreate(false);
    } catch (err) {
      setMessage({ type: 'error', text: 'Failed to start bot: ' + err });
    } finally {
      setStarting(false);
    }
  };

  const handleBotStopped = async () => {
    // Refresh list after a bot is stopped
    try {
      const bots = await invoke('list_local_bots');
      setRunningBots(bots);
    } catch (err) {
      console.error('Refresh error:', err);
    }
  };

  const activeCount = runningBots.filter((b) => b.running).length;
  const stoppedCount = runningBots.filter((b) => !b.running).length;

  return (
    <div style={styles.page}>
      <div style={{ maxWidth: 960, margin: '0 auto' }}>
        {/* Header */}
        <div style={{ ...styles.flexBetween, marginBottom: 24 }}>
          <div>
            <h1 style={styles.h1}>Bot Manager</h1>
            <p style={styles.subtitle}>
              Launch, monitor, and control local trading bots.
            </p>
          </div>
          <button
            style={styles.button}
            onClick={() => setShowCreate((prev) => !prev)}
          >
            {showCreate ? 'Cancel' : '+ New Bot'}
          </button>
        </div>

        {/* Summary bar */}
        <div
          style={{
            ...styles.card,
            display: 'flex',
            gap: 24,
            padding: '14px 20px',
          }}
        >
          <span style={{ fontSize: 13, color: theme.textMuted }}>
            <span style={styles.dot(theme.green)} />{' '}
            <strong style={{ color: theme.green, marginLeft: 4 }}>{activeCount}</strong>{' '}
            running
          </span>
          <span style={{ fontSize: 13, color: theme.textMuted }}>
            <span style={styles.dot(theme.red)} />{' '}
            <strong style={{ color: theme.textMuted, marginLeft: 4 }}>{stoppedCount}</strong>{' '}
            stopped
          </span>
          <span style={{ fontSize: 13, color: theme.textMuted }}>
            <strong style={{ color: theme.text }}>
              {configuredBrokers.length}
            </strong>{' '}
            brokers ready
          </span>
        </div>

        {/* Message banner */}
        {message && (
          <div
            style={{
              padding: '10px 16px',
              marginBottom: 16,
              borderRadius: 8,
              fontSize: 13,
              background:
                message.type === 'error' ? theme.red + '1A' : theme.green + '1A',
              color: message.type === 'error' ? theme.red : theme.green,
              border: `1px solid ${message.type === 'error' ? theme.red + '33' : theme.green + '33'}`,
            }}
          >
            {message.text}
          </div>
        )}

        {/* Create Bot Form */}
        {showCreate && (
          <div style={{ ...styles.card, marginBottom: 24 }}>
            <h2 style={{ ...styles.h2, marginBottom: 16 }}>Create New Bot</h2>

            {/* Row 1: Name + Broker */}
            <div style={styles.grid2}>
              <div style={styles.fieldGroup}>
                <label style={styles.label}>
                  Bot Name <span style={{ color: theme.red }}>*</span>
                </label>
                <input
                  type="text"
                  placeholder="my-alpha-bot"
                  value={form.bot_name}
                  onChange={(e) => handleFieldChange('bot_name', e.target.value)}
                  style={styles.input}
                  onFocus={(e) => (e.target.style.borderColor = theme.blue)}
                  onBlur={(e) => (e.target.style.borderColor = theme.border)}
                />
              </div>
              <div style={styles.fieldGroup}>
                <label style={styles.label}>
                  Broker <span style={{ color: theme.red }}>*</span>
                </label>
                <select
                  value={form.broker}
                  onChange={(e) => handleFieldChange('broker', e.target.value)}
                  style={{
                    ...styles.input,
                    cursor: 'pointer',
                    appearance: 'none',
                    backgroundImage: `url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='12' viewBox='0 0 12 12'%3E%3Cpath fill='%238B949E' d='M6 8L1 3h10z'/%3E%3C/svg%3E")`,
                    backgroundRepeat: 'no-repeat',
                    backgroundPosition: 'right 12px center',
                    paddingRight: 32,
                  }}
                >
                  <option value="">Select a broker...</option>
                  {configuredBrokers.map((id) => (
                    <option key={id} value={id}>
                      {brokerName(id)}
                    </option>
                  ))}
                </select>
                {configuredBrokers.length === 0 && (
                  <p style={{ fontSize: 12, color: theme.amber, marginTop: 4 }}>
                    No brokers configured. Set up credentials in the Brokers page first.
                  </p>
                )}
              </div>
            </div>

            {/* Risk / Allocation */}
            <div style={{ ...styles.grid2, marginTop: 4 }}>
              <div style={{ ...styles.grid2 }}>
                <div style={styles.fieldGroup}>
                  <label style={styles.label}>Max Positions</label>
                  <input
                    type="number"
                    min="1"
                    max="50"
                    value={form.allocation.max_positions}
                    onChange={(e) =>
                      handleFieldChange('allocation.max_positions', e.target.value)
                    }
                    style={styles.input}
                    onFocus={(e) => (e.target.style.borderColor = theme.blue)}
                    onBlur={(e) => (e.target.style.borderColor = theme.border)}
                  />
                </div>
                <div style={styles.fieldGroup}>
                  <label style={styles.label}>Position Size %</label>
                  <input
                    type="number"
                    min="0.5"
                    max="100"
                    step="0.5"
                    value={form.allocation.position_size_pct}
                    onChange={(e) =>
                      handleFieldChange('allocation.position_size_pct', e.target.value)
                    }
                    style={styles.input}
                    onFocus={(e) => (e.target.style.borderColor = theme.blue)}
                    onBlur={(e) => (e.target.style.borderColor = theme.border)}
                  />
                </div>
              </div>
              <div style={{ ...styles.grid2 }}>
                <div style={styles.fieldGroup}>
                  <label style={styles.label}>Max Drawdown %</label>
                  <input
                    type="number"
                    min="1"
                    max="100"
                    step="0.5"
                    value={form.risk.max_drawdown_pct}
                    onChange={(e) =>
                      handleFieldChange('risk.max_drawdown_pct', e.target.value)
                    }
                    style={styles.input}
                    onFocus={(e) => (e.target.style.borderColor = theme.blue)}
                    onBlur={(e) => (e.target.style.borderColor = theme.border)}
                  />
                </div>
                <div style={styles.fieldGroup}>
                  <label style={styles.label}>Daily Loss Limit $</label>
                  <input
                    type="number"
                    min="0"
                    step="50"
                    value={form.risk.daily_loss_limit}
                    onChange={(e) =>
                      handleFieldChange('risk.daily_loss_limit', e.target.value)
                    }
                    style={styles.input}
                    onFocus={(e) => (e.target.style.borderColor = theme.blue)}
                    onBlur={(e) => (e.target.style.borderColor = theme.border)}
                  />
                </div>
              </div>
            </div>

            {/* Strategy multi-select */}
            <div style={{ ...styles.fieldGroup, marginTop: 4 }}>
              <label style={styles.label}>
                Strategies <span style={{ color: theme.red }}>*</span>
                <span
                  style={{
                    color: theme.textMuted,
                    fontWeight: 400,
                    marginLeft: 8,
                  }}
                >
                  ({form.strategies.length} selected)
                </span>
              </label>
              <div
                style={{
                  display: 'flex',
                  flexWrap: 'wrap',
                  gap: 8,
                  maxHeight: 180,
                  overflow: 'auto',
                  padding: 12,
                  background: theme.surfaceAlt,
                  border: `1px solid ${theme.border}`,
                  borderRadius: 8,
                }}
              >
                {STRATEGY_LIST.map((name) => {
                  const selected = form.strategies.includes(name);
                  return (
                    <button
                      key={name}
                      type="button"
                      onClick={() => toggleStrategy(name)}
                      style={{
                        padding: '5px 12px',
                        borderRadius: 6,
                        fontSize: 12,
                        fontWeight: 500,
                        fontFamily: theme.font,
                        cursor: 'pointer',
                        border: `1px solid ${selected ? theme.blue : theme.border}`,
                        background: selected ? theme.blue + '1A' : 'transparent',
                        color: selected ? theme.blue : theme.textMuted,
                        transition: 'all 0.15s',
                      }}
                    >
                      {selected ? '✓ ' : ''}
                      {name}
                    </button>
                  );
                })}
              </div>
            </div>

            {/* Start button */}
            <div style={{ display: 'flex', gap: 12, marginTop: 8 }}>
              <button
                style={{
                  ...styles.button,
                  background: theme.green,
                  opacity: starting ? 0.5 : 1,
                  padding: '12px 28px',
                  fontSize: 15,
                }}
                disabled={starting}
                onClick={handleStartBot}
              >
                {starting ? 'Starting...' : 'Start Bot'}
              </button>
              <button
                style={styles.buttonSecondary}
                onClick={() => setShowCreate(false)}
              >
                Cancel
              </button>
            </div>
          </div>
        )}

        {/* Running Bots */}
        <div style={{ marginTop: 8 }}>
          <h2 style={{ ...styles.h2, marginBottom: 16 }}>
            Active Bots ({runningBots.length})
          </h2>

          {runningBots.length === 0 ? (
            <div
              style={{
                ...styles.card,
                textAlign: 'center',
                color: theme.textMuted,
                padding: 40,
              }}
            >
              <p style={{ fontSize: 14 }}>No bots are currently running.</p>
              <p style={{ fontSize: 13, marginTop: 8 }}>
                Click "+ New Bot" to launch your first trading bot.
              </p>
            </div>
          ) : (
            runningBots.map((bot) => (
              <BotStatusCard
                key={bot.bot_name}
                botName={bot.bot_name}
                broker={bot.broker}
                initialInfo={bot}
                onStopped={handleBotStopped}
              />
            ))
          )}
        </div>
      </div>
    </div>
  );
}
