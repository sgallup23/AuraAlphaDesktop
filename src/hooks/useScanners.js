import { useState, useEffect, useCallback } from 'react';
import { api } from '../utils/api';

function normalizeResults(data) {
  let matches = [];
  if (Array.isArray(data)) matches = data;
  else if (data?.matches) matches = data.matches;
  else if (data?.results) matches = data.results;

  return matches.map(m => ({
    symbol: m.symbol || m.sym,
    strategy: m.strategy || m.strat || '',
    region: m.region || 'us',
    winRate: (m.win_rate || m.winRate || 0) > 1 ? (m.win_rate || m.winRate) : (m.win_rate || m.winRate || 0) * 100,
    sharpe: m.sharpe || m.sharpe_ratio || 0,
    totalReturn: (m.total_return || m.return_pct || 0) > 5 ? (m.total_return || m.return_pct) : (m.total_return || m.return_pct || 0) * 100,
    strength: m.signal_strength || m.strength || m.score || 0,
    trades: m.trades || m.trade_count || 0,
  }));
}

export default function useScanners() {
  const [presets, setPresets] = useState([]);
  const [selectedPreset, setSelectedPreset] = useState('');
  const [results, setResults] = useState([]);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    api('/scanners', { silent: true }).then(data => {
      const p = Array.isArray(data) ? data : data?.presets || [];
      setPresets(p);
      if (p.length > 0) setSelectedPreset(p[0].key || p[0].name);
    });
  }, []);

  const runScan = useCallback(async (preset) => {
    const key = preset || selectedPreset;
    if (!key) return;
    setLoading(true);
    try {
      const data = await api(`/scanners/${key}`, { silent: true });
      setResults(normalizeResults(data));
    } catch (e) {
      console.warn('[useScanners]', e);
    } finally {
      setLoading(false);
    }
  }, [selectedPreset]);

  useEffect(() => { if (selectedPreset) runScan(); }, [selectedPreset, runScan]);

  return { presets, selectedPreset, setSelectedPreset, results, loading, runScan };
}
