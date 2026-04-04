import { useState, useCallback, useEffect } from 'react';
import { api, apiPost, apiPut, apiDelete } from '../utils/api';

/**
 * Hook for strategy execution routing — CRUD + validation.
 * Connects to /api/strategy-routes/* endpoints on the cloud API.
 */
export default function useStrategyRouting() {
  const [routes, setRoutes] = useState([]);
  const [strategies, setStrategies] = useState([]);
  const [brokers, setBrokers] = useState([]);
  const [loading, setLoading] = useState(true);
  const [message, setMessage] = useState(null);

  const fetchRoutes = useCallback(async () => {
    try {
      const data = await api('/api/strategy-routes/', { silent: true });
      setRoutes(data?.routes || []);
    } catch (e) {
      console.warn('Failed to fetch routes:', e);
    }
  }, []);

  const fetchStrategies = useCallback(async () => {
    try {
      const data = await api('/api/strategies', { silent: true });
      const list = data?.strategies || data || [];
      setStrategies(Array.isArray(list) ? list : []);
    } catch (e) {
      console.warn('Failed to fetch strategies:', e);
    }
  }, []);

  const fetchBrokers = useCallback(async () => {
    try {
      const data = await api('/api/strategy-routes/brokers', { silent: true });
      setBrokers(data?.brokers || []);
    } catch (e) {
      // Brokers endpoint may not exist yet — use defaults
      setBrokers([
        { id: 'ibkr', name: 'Interactive Brokers', asset_classes: ['equity','options','forex','futures','crypto'] },
        { id: 'kraken', name: 'Kraken', asset_classes: ['crypto'] },
        { id: 'alpaca', name: 'Alpaca', asset_classes: ['equity','crypto'] },
      ]);
    }
  }, []);

  const refresh = useCallback(async () => {
    setLoading(true);
    await Promise.all([fetchRoutes(), fetchStrategies(), fetchBrokers()]);
    setLoading(false);
  }, [fetchRoutes, fetchStrategies, fetchBrokers]);

  useEffect(() => { refresh(); }, [refresh]);

  const getRouteForStrategy = useCallback(async (strategyName, botName) => {
    try {
      const params = botName ? `?bot_name=${botName}` : '';
      return await api(`/api/strategy-routes/strategy/${encodeURIComponent(strategyName)}${params}`);
    } catch (e) {
      return { route: null, using_default: true };
    }
  }, []);

  const saveRoute = useCallback(async (strategyName, routeData) => {
    try {
      // Try PATCH first (update existing), fall back to POST (create new)
      let result;
      try {
        result = await apiPut(`/api/strategy-routes/strategy/${encodeURIComponent(strategyName)}`, routeData);
      } catch (patchErr) {
        if (patchErr?.status === 404) {
          result = await apiPost(`/api/strategy-routes/strategy/${encodeURIComponent(strategyName)}`, routeData);
        } else {
          throw patchErr;
        }
      }
      setMessage({ type: 'success', text: `Route saved for ${strategyName}` });
      await fetchRoutes();
      return result;
    } catch (e) {
      const detail = e?.detail || e?.message || 'Save failed';
      setMessage({ type: 'error', text: detail });
      return null;
    }
  }, [fetchRoutes]);

  const deleteRoute = useCallback(async (strategyName, botName) => {
    try {
      const params = botName ? `?bot_name=${botName}` : '';
      await apiDelete(`/api/strategy-routes/strategy/${encodeURIComponent(strategyName)}${params}`);
      setMessage({ type: 'success', text: `Route deleted for ${strategyName}` });
      await fetchRoutes();
      return true;
    } catch (e) {
      setMessage({ type: 'error', text: e?.detail || 'Delete failed' });
      return false;
    }
  }, [fetchRoutes]);

  const validateRoute = useCallback(async (strategyName) => {
    try {
      return await apiPost(`/api/strategy-routes/validate/${encodeURIComponent(strategyName)}`);
    } catch (e) {
      return { valid: false, errors: [e?.detail || 'Validation failed'] };
    }
  }, []);

  // Merge strategies with their routes for unified display
  const mergedView = strategies.map(s => {
    const name = s.name || s.key || s.display_name || '';
    const route = routes.find(r => r.strategy_name === name);
    return {
      strategy_name: name,
      direction: s.direction || 'long',
      win_rate: s.win_rate || s.avg_win_rate || s.stats?.avg_win_rate || 0,
      has_explicit_route: !!route,
      broker_type: route?.broker_type || 'ibkr',
      account_id: route?.account_id || '',
      venue: route?.venue || 'SMART',
      region: route?.region || 'us',
      asset_class: route?.asset_class || 'equity',
      bot_name: route?.bot_name || '',
      enabled: route?.enabled ?? true,
      validation_status: route?.validation_status || 'unchecked',
    };
  });

  return {
    routes,
    strategies,
    brokers,
    mergedView,
    loading,
    message,
    setMessage,
    refresh,
    getRouteForStrategy,
    saveRoute,
    deleteRoute,
    validateRoute,
  };
}
