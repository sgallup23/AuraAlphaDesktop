import { useState, useEffect, useCallback } from 'react';
import { api } from '../utils/api';

function normalizeStrategy(s) {
  return {
    name: s.name || s.key,
    label: s.display_name || s.label || s.name || s.key,
    direction: s.direction || 'long',
    winRate: s.stats?.avg_win_rate != null ? (s.stats.avg_win_rate > 1 ? s.stats.avg_win_rate : s.stats.avg_win_rate * 100) : (s.avg_win_rate || 0),
    avgReturn: s.stats?.avg_return != null ? (s.stats.avg_return > 5 ? s.stats.avg_return : s.stats.avg_return * 100) : (s.avg_return || 0),
    avgSharpe: s.stats?.avg_sharpe || s.avg_sharpe || s.sharpe || 0,
    totalTrades: s.stats?.total_trades || s.total_trades || 0,
    status: s.status || 'active',
    suppressed: s.suppressed || false,
  };
}

export default function useStrategies(initialAssetClass = 'us') {
  const [strategies, setStrategies] = useState([]);
  const [loading, setLoading] = useState(true);
  const [assetClass, setAssetClass] = useState(initialAssetClass);

  const fetchStrategies = useCallback(async () => {
    setLoading(true);
    const data = await api(`/strategies?asset_class=${assetClass}`, { silent: true });
    let strats = [];
    if (Array.isArray(data)) strats = data;
    else if (data?.strategies) strats = data.strategies;
    setStrategies(strats.map(normalizeStrategy));
    setLoading(false);
  }, [assetClass]);

  useEffect(() => { fetchStrategies(); }, [fetchStrategies]);

  return { strategies, loading, assetClass, setAssetClass, refresh: fetchStrategies };
}
