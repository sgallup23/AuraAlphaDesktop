import { useState, useEffect, useCallback } from 'react';
import { api } from '../utils/api';

export default function usePositions(pollMs = 10000) {
  const [positions, setPositions] = useState([]);
  const [orders, setOrders] = useState([]);
  const [loading, setLoading] = useState(true);

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
    fetchData();
    const iv = setInterval(fetchData, pollMs);
    return () => clearInterval(iv);
  }, [fetchData, pollMs]);

  return { positions, orders, loading, refresh: fetchData };
}
