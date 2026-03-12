import { useState, useCallback } from 'react';
import { api } from '../utils/api';

export default function useChartData() {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');

  const fetchChart = useCallback(async (symbol, timeframe, { onCandles, onVolumes } = {}) => {
    setLoading(true);
    setError('');
    try {
      const data = await api(`/charts/ohlcv/${symbol}?timeframe=${timeframe}&bars=500`, { silent: true });
      if (!data?.bars?.length && !Array.isArray(data)) {
        setError('No data available');
        return null;
      }
      const bars = data.bars || data;
      const candles = bars.map(b => ({
        time: typeof b.time === 'string' ? b.time : new Date(b.time * 1000).toISOString().slice(0, 10),
        open: b.open, high: b.high, low: b.low, close: b.close,
      })).sort((a, b) => a.time.localeCompare(b.time));

      const volumes = bars.map(b => ({
        time: typeof b.time === 'string' ? b.time : new Date(b.time * 1000).toISOString().slice(0, 10),
        value: b.volume || 0,
        color: b.close >= b.open ? 'rgba(63,185,80,0.3)' : 'rgba(248,81,73,0.3)',
      })).sort((a, b) => a.time.localeCompare(b.time));

      if (onCandles) onCandles(candles);
      if (onVolumes) onVolumes(volumes);
      return { candles, volumes };
    } catch (e) {
      setError(e.message || 'Failed to load chart');
      return null;
    } finally {
      setLoading(false);
    }
  }, []);

  return { fetchChart, loading, error };
}
