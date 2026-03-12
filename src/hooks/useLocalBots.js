import { useState, useEffect, useCallback, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';

export default function useLocalBots(pollMs = 10000) {
  const [configuredBrokers, setConfiguredBrokers] = useState([]);
  const [allBrokers, setAllBrokers] = useState([]);
  const [runningBots, setRunningBots] = useState([]);
  const [loading, setLoading] = useState(true);
  const [message, setMessage] = useState(null);
  const pollRef = useRef(null);

  const loadData = useCallback(async () => {
    try {
      const [configured, available, bots] = await Promise.all([
        invoke('list_configured_brokers'),
        invoke('get_available_brokers'),
        invoke('list_local_bots'),
      ]);
      setConfiguredBrokers(configured);
      setAllBrokers(available);
      setRunningBots(bots);
    } catch (err) {
      setMessage({ type: 'error', text: 'Failed to load data: ' + err });
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadData();
    pollRef.current = setInterval(loadData, pollMs);
    return () => { if (pollRef.current) clearInterval(pollRef.current); };
  }, [loadData, pollMs]);

  const brokerName = useCallback((id) => {
    const b = allBrokers.find(b => b.id === id);
    return b ? b.name : id;
  }, [allBrokers]);

  const startBot = useCallback(async (config) => {
    setMessage(null);
    try {
      const result = await invoke('start_bot', { config });
      setMessage({ type: 'success', text: `Bot "${result.bot_name}" started (PID ${result.pid}).` });
      const bots = await invoke('list_local_bots');
      setRunningBots(bots);
      return result;
    } catch (err) {
      setMessage({ type: 'error', text: 'Failed to start: ' + err });
      throw err;
    }
  }, []);

  const stopBot = useCallback(async (botName) => {
    try {
      await invoke('stop_bot', { botName });
      const bots = await invoke('list_local_bots');
      setRunningBots(bots);
    } catch (err) {
      setMessage({ type: 'error', text: 'Failed to stop: ' + err });
      throw err;
    }
  }, []);

  const viewLogs = useCallback(async (botName, tailLines = 50) => {
    try {
      return await invoke('get_bot_log', { botName, tailLines });
    } catch (err) {
      return 'Error loading logs: ' + err;
    }
  }, []);

  return {
    configuredBrokers,
    allBrokers,
    runningBots,
    loading,
    message,
    setMessage,
    startBot,
    stopBot,
    viewLogs,
    brokerName,
  };
}
