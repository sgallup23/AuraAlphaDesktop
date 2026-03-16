import { useState, useEffect, useCallback } from 'react';
import { api } from '../utils/api';

function normalizeBacktest(s) {
  return {
    name: s.name || s.key || '',
    label: s.display_name || s.label || s.name || s.key || '',
    direction: s.direction || 'long',
    count: s.count || s.total_trades || 0,
    avgReturn: s.avg_return != null ? s.avg_return : 0,
    winRate: s.win_rate != null ? (s.win_rate > 1 ? s.win_rate : s.win_rate * 100) : 0,
    totalPnl: s.total_pnl != null ? s.total_pnl : 0,
    grossProfit: s.gross_profit != null ? s.gross_profit : 0,
    grossLoss: s.gross_loss != null ? s.gross_loss : 0,
    profitFactor: s.profit_factor != null ? s.profit_factor : 0,
  };
}

export default function useBacktests(initialRegion = 'us') {
  const [backtests, setBacktests] = useState([]);
  const [loading, setLoading] = useState(true);
  const [region, setRegion] = useState(initialRegion);

  const fetchBacktests = useCallback(async () => {
    setLoading(true);
    const data = await api(`/backtest/strategies?region=${region}`, { silent: true });
    let strats = [];
    if (Array.isArray(data)) strats = data;
    else if (data?.strategies) strats = data.strategies;
    setBacktests(strats.map(normalizeBacktest));
    setLoading(false);
  }, [region]);

  useEffect(() => { fetchBacktests(); }, [fetchBacktests]);

  return { backtests, loading, region, setRegion, refresh: fetchBacktests };
}
