import { useState, useEffect, useCallback, useRef } from 'react';
import { api, apiPost, apiDelete } from '../utils/api';
import useVisibility from './useVisibility';

export default function useAlerts(pollMs = 15000) {
  const [alerts, setAlerts] = useState([]);
  const [triggered, setTriggered] = useState([]);
  const [loading, setLoading] = useState(true);
  const visible = useVisibility();
  const intervalRef = useRef(null);

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
    if (intervalRef.current) {
      clearInterval(intervalRef.current);
      intervalRef.current = null;
    }
    if (visible) {
      fetchAlerts();
      intervalRef.current = setInterval(fetchAlerts, pollMs);
    }
    return () => {
      if (intervalRef.current) {
        clearInterval(intervalRef.current);
        intervalRef.current = null;
      }
    };
  }, [fetchAlerts, pollMs, visible]);

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
