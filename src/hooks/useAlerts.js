import { useState, useEffect, useCallback } from 'react';
import { api, apiPost, apiDelete } from '../utils/api';

export default function useAlerts(pollMs = 15000) {
  const [alerts, setAlerts] = useState([]);
  const [triggered, setTriggered] = useState([]);
  const [loading, setLoading] = useState(true);

  const fetchAlerts = useCallback(async () => {
    const [activeData, trigData] = await Promise.all([
      api('/alerts', { silent: true }),
      api('/alerts/triggered', { silent: true }),
    ]);
    setAlerts(Array.isArray(activeData) ? activeData : activeData?.alerts || []);
    setTriggered(Array.isArray(trigData) ? trigData : trigData?.alerts || []);
    setLoading(false);
  }, []);

  useEffect(() => {
    fetchAlerts();
    const iv = setInterval(fetchAlerts, pollMs);
    return () => clearInterval(iv);
  }, [fetchAlerts, pollMs]);

  const createAlert = useCallback(async ({ symbol, condition, value }) => {
    await apiPost('/alerts', { symbol: symbol.toUpperCase(), condition, value: Number(value) });
    fetchAlerts();
  }, [fetchAlerts]);

  const deleteAlert = useCallback(async (id) => {
    await apiDelete(`/alerts/${id}`);
    fetchAlerts();
  }, [fetchAlerts]);

  return { alerts, triggered, loading, refresh: fetchAlerts, createAlert, deleteAlert };
}
