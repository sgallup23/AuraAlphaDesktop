import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useNavigate } from 'react-router-dom';
import { theme, styles } from '../theme';

export default function DashboardPage() {
  const navigate = useNavigate();
  const [health, setHealth] = useState(null);
  const [ec2Bots, setEc2Bots] = useState([]);
  const [localBots, setLocalBots] = useState([]);
  const [worker, setWorker] = useState(null);
  const [error, setError] = useState(null);
  const [loading, setLoading] = useState(true);

  const loadData = useCallback(async () => {
    setLoading(true);
    setError(null);

    try {
      const [healthData, botData, localBotData, workerData] = await Promise.allSettled([
        invoke('check_health'),
        invoke('get_bot_status'),
        invoke('list_local_bots'),
        invoke('get_worker_status'),
      ]);

      if (healthData.status === 'fulfilled') setHealth(healthData.value);
      else setError(healthData.reason);

      if (botData.status === 'fulfilled') setEc2Bots(botData.value);
      if (localBotData.status === 'fulfilled') setLocalBots(localBotData.value);
      if (workerData.status === 'fulfilled') setWorker(workerData.value);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadData();
    const interval = setInterval(loadData, 15000); // refresh every 15s
    return () => clearInterval(interval);
  }, [loadData]);

  const localActive = localBots.filter((b) => b.running).length;

  return (
    <div style={styles.page}>
      <div style={{ maxWidth: 960, margin: '0 auto' }}>
        {/* Header */}
        <div style={{ ...styles.flexBetween, marginBottom: 24 }}>
          <div>
            <h1 style={styles.h1}>Dashboard</h1>
            <p style={styles.subtitle}>
              System overview and bot status.
            </p>
          </div>
          <button style={styles.buttonSecondary} onClick={loadData} disabled={loading}>
            {loading ? 'Refreshing...' : 'Refresh'}
          </button>
        </div>

        {/* EC2 Health */}
        <div style={{ ...styles.card, marginBottom: 20 }}>
          <div style={{ ...styles.flexRow, marginBottom: 16 }}>
            <span
              style={styles.dot(
                health ? (health.api_up ? theme.green : theme.red) : theme.textMuted
              )}
            />
            <h2 style={styles.h2}>EC2 Production</h2>
            {health && health.api_up && (
              <span style={styles.badge(theme.green)}>Online</span>
            )}
            {health && !health.api_up && (
              <span style={styles.badge(theme.red)}>Offline</span>
            )}
            {!health && !error && (
              <span style={styles.badge(theme.textMuted)}>Checking...</span>
            )}
            {error && <span style={styles.badge(theme.red)}>Error</span>}
          </div>

          {health ? (
            <div style={styles.grid3}>
              <MetricTile label="Bots Active" value={health.bots_active} color={theme.blue} />
              <MetricTile label="Positions" value={health.total_positions} />
              <MetricTile
                label="Day P&L"
                value={
                  (health.total_pnl_today >= 0 ? '+$' : '-$') +
                  Math.abs(health.total_pnl_today).toFixed(2)
                }
                color={health.total_pnl_today >= 0 ? theme.green : theme.red}
              />
            </div>
          ) : error ? (
            <p style={{ fontSize: 13, color: theme.red }}>{String(error)}</p>
          ) : (
            <p style={{ fontSize: 13, color: theme.textMuted }}>
              Loading EC2 health data...
            </p>
          )}

          {/* EC2 bot chips */}
          {ec2Bots.length > 0 && (
            <div style={{ display: 'flex', gap: 8, marginTop: 16, flexWrap: 'wrap' }}>
              {ec2Bots.map((bot) => (
                <div
                  key={bot.name}
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: 6,
                    background: theme.surfaceAlt,
                    borderRadius: 6,
                    padding: '6px 12px',
                    fontSize: 12,
                    fontFamily: theme.mono,
                  }}
                >
                  <span
                    style={styles.dot(
                      bot.status === 'running' ? theme.green : theme.red
                    )}
                  />
                  {bot.name.toUpperCase()}
                  <span style={{ color: theme.textMuted }}>
                    ({bot.positions} pos)
                  </span>
                </div>
              ))}
            </div>
          )}
        </div>

        {/* Local Bots + Worker */}
        <div style={styles.grid2}>
          {/* Local Bots Card */}
          <div style={styles.card}>
            <div style={{ ...styles.flexBetween, marginBottom: 12 }}>
              <div style={styles.flexRow}>
                <span
                  style={styles.dot(localActive > 0 ? theme.green : theme.textMuted)}
                />
                <h2 style={styles.h2}>Local Bots</h2>
              </div>
              <button
                style={{ ...styles.button, padding: '6px 14px', fontSize: 13 }}
                onClick={() => navigate('/bots')}
              >
                Manage
              </button>
            </div>

            {localBots.length === 0 ? (
              <p style={{ fontSize: 13, color: theme.textMuted }}>
                No local bots running. Go to Bot Manager to start one.
              </p>
            ) : (
              <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
                {localBots.map((bot) => (
                  <div
                    key={bot.bot_name}
                    style={{
                      display: 'flex',
                      alignItems: 'center',
                      gap: 8,
                      padding: '8px 12px',
                      background: theme.surfaceAlt,
                      borderRadius: 6,
                      fontSize: 13,
                    }}
                  >
                    <span
                      style={styles.dot(bot.running ? theme.green : theme.red)}
                    />
                    <span style={{ fontWeight: 600 }}>{bot.bot_name}</span>
                    <span style={{ color: theme.textMuted, marginLeft: 'auto' }}>
                      {bot.broker}
                    </span>
                    {bot.pid && (
                      <span
                        style={{
                          fontFamily: theme.mono,
                          fontSize: 11,
                          color: theme.textMuted,
                        }}
                      >
                        PID {bot.pid}
                      </span>
                    )}
                  </div>
                ))}
              </div>
            )}
          </div>

          {/* Compute Worker Card */}
          <div style={styles.card}>
            <div style={{ ...styles.flexRow, marginBottom: 12 }}>
              <span
                style={styles.dot(
                  worker && worker.running ? theme.green : theme.amber
                )}
              />
              <h2 style={styles.h2}>Compute Worker</h2>
              {worker && worker.running && (
                <span style={styles.badge(theme.green)}>Active</span>
              )}
              {worker && !worker.running && (
                <span style={styles.badge(theme.amber)}>Inactive</span>
              )}
            </div>

            {worker ? (
              <div style={{ fontSize: 13, color: theme.textMuted }}>
                {worker.running ? (
                  <>
                    <p>
                      PID:{' '}
                      <span style={{ fontFamily: theme.mono, color: theme.text }}>
                        {worker.pid}
                      </span>
                    </p>
                    {worker.project_path && (
                      <p style={{ marginTop: 4 }}>
                        Path:{' '}
                        <span style={{ fontFamily: theme.mono, fontSize: 11 }}>
                          {worker.project_path}
                        </span>
                      </p>
                    )}
                  </>
                ) : (
                  <p>
                    Worker not running. It processes optimization and backtest jobs
                    queued from EC2.
                  </p>
                )}
                <div style={{ display: 'flex', gap: 8, marginTop: 12 }}>
                  {!worker.running && (
                    <WorkerButton action="start_worker" label="Start Worker" />
                  )}
                  {worker.running && (
                    <WorkerButton action="stop_worker" label="Stop Worker" danger />
                  )}
                </div>
              </div>
            ) : (
              <p style={{ fontSize: 13, color: theme.textMuted }}>
                Loading worker status...
              </p>
            )}
          </div>
        </div>

        {/* Quick links */}
        <div
          style={{
            ...styles.card,
            marginTop: 4,
            display: 'flex',
            gap: 12,
            justifyContent: 'center',
            padding: '16px 20px',
          }}
        >
          <a
            href="https://auraalpha.cc"
            target="_blank"
            rel="noopener noreferrer"
            style={{
              ...styles.buttonSecondary,
              textDecoration: 'none',
              display: 'inline-block',
            }}
          >
            Open Web Dashboard
          </a>
          <button
            style={styles.buttonSecondary}
            onClick={() => navigate('/brokers')}
          >
            Broker Setup
          </button>
          <button
            style={styles.buttonSecondary}
            onClick={() => navigate('/bots')}
          >
            Bot Manager
          </button>
        </div>
      </div>
    </div>
  );
}

function MetricTile({ label, value, color }) {
  return (
    <div
      style={{
        background: theme.surfaceAlt,
        borderRadius: 8,
        padding: 12,
        textAlign: 'center',
      }}
    >
      <div
        style={{
          fontSize: 11,
          color: theme.textMuted,
          textTransform: 'uppercase',
          letterSpacing: 0.5,
          marginBottom: 4,
        }}
      >
        {label}
      </div>
      <div
        style={{
          fontSize: 20,
          fontWeight: 600,
          fontFamily: theme.mono,
          color: color || theme.text,
        }}
      >
        {value}
      </div>
    </div>
  );
}

function WorkerButton({ action, label, danger }) {
  const [loading, setLoading] = useState(false);

  const handleClick = async () => {
    setLoading(true);
    try {
      await invoke(action);
    } catch (err) {
      console.error(`${action} error:`, err);
    } finally {
      setLoading(false);
    }
  };

  return (
    <button
      style={{
        ...(danger ? styles.buttonDanger : styles.button),
        padding: '6px 14px',
        fontSize: 13,
        opacity: loading ? 0.5 : 1,
      }}
      disabled={loading}
      onClick={handleClick}
    >
      {loading ? 'Working...' : label}
    </button>
  );
}
