import { useState, useEffect, useRef, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { theme, styles } from '../theme';

/**
 * BotStatusCard — Reusable card for a single running/stopped bot.
 *
 * Props:
 *   botName     — string
 *   broker      — string
 *   initialInfo — optional BotInfo from list_local_bots
 *   onStopped   — callback when bot is stopped
 */
export default function BotStatusCard({ botName, broker, initialInfo, onStopped }) {
  const [info, setInfo] = useState(initialInfo || { running: false, pid: null });
  const [logs, setLogs] = useState(null);
  const [showLogs, setShowLogs] = useState(false);
  const [stopping, setStopping] = useState(false);
  const intervalRef = useRef(null);

  // Poll status every 5 seconds
  const pollStatus = useCallback(async () => {
    try {
      const status = await invoke('get_local_bot_status', { botName });
      setInfo(status);
      // If the bot died, stop polling
      if (!status.running && intervalRef.current) {
        clearInterval(intervalRef.current);
        intervalRef.current = null;
      }
    } catch (err) {
      console.error('Status poll error:', err);
    }
  }, [botName]);

  useEffect(() => {
    // Initial fetch
    pollStatus();

    // Start polling if likely running
    intervalRef.current = setInterval(pollStatus, 5000);

    return () => {
      if (intervalRef.current) {
        clearInterval(intervalRef.current);
      }
    };
  }, [pollStatus]);

  const handleStop = async () => {
    setStopping(true);
    try {
      await invoke('stop_bot', { botName });
      setInfo((prev) => ({ ...prev, running: false, pid: null }));
      if (onStopped) onStopped(botName);
    } catch (err) {
      console.error('Stop error:', err);
    } finally {
      setStopping(false);
    }
  };

  const handleViewLogs = async () => {
    if (showLogs) {
      setShowLogs(false);
      return;
    }
    try {
      const logContent = await invoke('get_bot_log', { botName, tailLines: 50 });
      setLogs(logContent);
      setShowLogs(true);
    } catch (err) {
      setLogs('Error loading logs: ' + err);
      setShowLogs(true);
    }
  };

  const running = info.running;
  const statusColor = running ? theme.green : theme.red;
  const statusLabel = running ? 'RUNNING' : 'STOPPED';

  const startedAtStr = info.started_at
    ? new Date(info.started_at * 1000).toLocaleString()
    : null;

  return (
    <div style={styles.card}>
      {/* Header row */}
      <div style={{ ...styles.flexBetween, marginBottom: 12 }}>
        <div style={styles.flexRow}>
          <span style={styles.dot(statusColor)} />
          <span style={{ fontSize: 16, fontWeight: 600 }}>{botName}</span>
          <span style={styles.badge(statusColor)}>{statusLabel}</span>
        </div>
        <div style={styles.flexRow}>
          {running && (
            <button
              style={{ ...styles.buttonDanger, padding: '6px 14px', fontSize: 13 }}
              onClick={handleStop}
              disabled={stopping}
            >
              {stopping ? 'Stopping...' : 'Stop'}
            </button>
          )}
          <button
            style={{ ...styles.buttonSecondary, padding: '6px 14px', fontSize: 13 }}
            onClick={handleViewLogs}
          >
            {showLogs ? 'Hide Logs' : 'View Logs'}
          </button>
        </div>
      </div>

      {/* Details row */}
      <div style={{ display: 'flex', gap: 24, fontSize: 13, color: theme.textMuted }}>
        <span>
          <strong style={{ color: theme.text }}>Broker:</strong> {broker || info.broker || '—'}
        </span>
        {info.pid && (
          <span>
            <strong style={{ color: theme.text }}>PID:</strong>{' '}
            <span style={{ fontFamily: theme.mono }}>{info.pid}</span>
          </span>
        )}
        {startedAtStr && (
          <span>
            <strong style={{ color: theme.text }}>Started:</strong> {startedAtStr}
          </span>
        )}
      </div>

      {/* Log panel */}
      {showLogs && (
        <div
          style={{
            marginTop: 12,
            background: theme.bg,
            border: `1px solid ${theme.border}`,
            borderRadius: 8,
            padding: 12,
            maxHeight: 300,
            overflow: 'auto',
          }}
        >
          <pre
            style={{
              margin: 0,
              fontFamily: theme.mono,
              fontSize: 12,
              color: theme.textMuted,
              whiteSpace: 'pre-wrap',
              wordBreak: 'break-all',
            }}
          >
            {logs || 'No log output yet.'}
          </pre>
        </div>
      )}
    </div>
  );
}
