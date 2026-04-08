import { useState, useEffect, useCallback, useRef } from 'react';
import { api } from '../utils/api';
import useVisibility from './useVisibility';

export default function usePositions(pollMs = 10000) {
  const [positions, setPositions] = useState([]);
  const [orders, setOrders] = useState([]);
  const [loading, setLoading] = useState(true);
  const visible = useVisibility();
  const intervalRef = useRef(null);

  const fetchData = useCallback(async () => {
    const [posData, orderData] = await Promise.all([
      api('/positions', { silent: true }),
      api('/positions/orders', { silent: true }),
    ]);
    if (Array.isArray(posData)) setPositions(posData);
    else if (posData?.positions) setPositions(posData.positions);
    if (Array.isArray(orderData)) setOrders(orderData);
    else if (orderData?.orders) setOrders(orderData.orders);
    setLoading(false);
  }, []);

  useEffect(() => {
    if (intervalRef.current) {
      clearInterval(intervalRef.current);
      intervalRef.current = null;
    }
    if (visible) {
      fetchData();
      intervalRef.current = setInterval(fetchData, pollMs);
    }
    return () => {
      if (intervalRef.current) {
        clearInterval(intervalRef.current);
        intervalRef.current = null;
      }
    };
  }, [fetchData, pollMs, visible]);

  return { positions, orders, loading, refresh: fetchData };
}
