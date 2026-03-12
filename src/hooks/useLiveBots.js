import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { api } from '../utils/api';

const BOT_COLORS = { shawn: '#22d3ee', shane: '#a78bfa', nova: '#f59e0b' };
const BOT_META_KEYS = new Set(['gateway_connected', 'accounts', 'equity', 'position_count', 'trade_count']);

function parseBots(telData) {
  if (!telData?.bots) return [];
  return Object.entries(telData.bots)
    .filter(([k, v]) => !BOT_META_KEYS.has(k) && typeof v === 'object' && v !== null && !Array.isArray(v))
    .map(([name, data]) => {
      const p = data.payload || {};
      const eq = p.equity || {};
      return {
        name: name.toUpperCase(),
        key: name,
        status: data.status || 'UNKNOWN',
        mode: p.mode || 'UNKNOWN',
        enabled: p.enabled !== false,
        equity: eq.current || eq.starting || 0,
        dayPnl: eq.day_pnl || eq.day || 0,
        ddPct: eq.dd_pct || 0,
        buyingPower: eq.buying_power || eq.available_funds || 0,
        positions: Array.isArray(p.positions) ? p.positions : [],
        positionCount: Array.isArray(p.positions) ? p.positions.length : 0,
        tradeCount: Array.isArray(p.trades) ? p.trades.length : 0,
        heartbeat: data.last_heartbeat,
        color: BOT_COLORS[name] || '#58A6FF',
        reason: p.reason || '',
      };
    });
}

export default function useLiveBots(pollMs = 10000) {
  const [bots, setBots] = useState([]);
  const [health, setHealth] = useState(null);
  const [loading, setLoading] = useState(true);

  const fetchData = useCallback(async () => {
    try {
      const [telData, healthData] = await Promise.all([
        api('/telemetry/latest', { silent: true }),
        invoke('check_health').catch(() => null),
      ]);
      setHealth(healthData);
      setBots(parseBots(telData));
    } catch (e) {
      console.warn('[useLiveBots] fetch error:', e);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchData();
    const iv = setInterval(fetchData, pollMs);
    return () => clearInterval(iv);
  }, [fetchData, pollMs]);

  const totalEquity = bots.reduce((s, b) => s + b.equity, 0);
  const totalPnl = bots.reduce((s, b) => s + b.dayPnl, 0);
  const totalPositions = bots.reduce((s, b) => s + b.positionCount, 0);

  return { bots, health, loading, refresh: fetchData, totalEquity, totalPnl, totalPositions };
}
