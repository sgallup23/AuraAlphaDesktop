import { useState, useEffect, useCallback } from 'react';
import { api, apiPost, apiDelete } from '../utils/api';

export default function useWatchlists() {
  const [watchlists, setWatchlists] = useState([]);
  const [selected, setSelected] = useState(null);
  const [loading, setLoading] = useState(true);
  const [prices, setPrices] = useState({});

  const fetchWatchlists = useCallback(async () => {
    const data = await api('/watchlists', { silent: true });
    const lists = Array.isArray(data) ? data : data?.watchlists || [];
    setWatchlists(lists);
    if (!selected && lists.length > 0) setSelected(lists[0].id);
    setLoading(false);
  }, [selected]);

  useEffect(() => { fetchWatchlists(); }, [fetchWatchlists]);

  const activeList = watchlists.find(w => w.id === selected);

  // Fetch prices for watchlist symbols
  useEffect(() => {
    if (!activeList?.symbols?.length) return;
    const fetchPrices = async () => {
      const syms = activeList.symbols.map(s => typeof s === 'string' ? s : s.symbol);
      for (const sym of syms.slice(0, 20)) {
        if (prices[sym]) continue;
        const data = await api(`/charts/ohlcv/${sym}?bars=2`, { silent: true });
        const bars = data?.bars || (Array.isArray(data) ? data : []);
        if (bars.length > 0) {
          const last = bars[bars.length - 1];
          const prev = bars.length > 1 ? bars[bars.length - 2] : last;
          setPrices(p => ({ ...p, [sym]: { price: last.close, change: ((last.close - prev.close) / prev.close) * 100 } }));
        }
      }
    };
    fetchPrices();
  }, [activeList]);

  const addSymbol = useCallback(async (sym) => {
    if (!sym?.trim() || !selected) return;
    await apiPost(`/watchlists/${selected}/symbols`, { symbol: sym.trim().toUpperCase() });
    fetchWatchlists();
  }, [selected, fetchWatchlists]);

  const removeSymbol = useCallback(async (sym) => {
    if (!selected) return;
    await apiDelete(`/watchlists/${selected}/symbols?symbol=${sym}`);
    fetchWatchlists();
  }, [selected, fetchWatchlists]);

  const createList = useCallback(async (name) => {
    if (!name?.trim()) return;
    await apiPost('/watchlists', { name: name.trim() });
    fetchWatchlists();
  }, [fetchWatchlists]);

  return { watchlists, selected, setSelected, activeList, loading, prices, addSymbol, removeSymbol, createList };
}
