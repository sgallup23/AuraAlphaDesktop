import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';

export default function useBrokerSetup() {
  const [brokers, setBrokers] = useState([]);
  const [configured, setConfigured] = useState([]);
  const [loading, setLoading] = useState(true);
  const [message, setMessage] = useState(null);

  const loadData = useCallback(async () => {
    try {
      const [allBrokers, configuredList] = await Promise.all([
        invoke('get_available_brokers'),
        invoke('list_configured_brokers'),
      ]);
      setBrokers(allBrokers);
      setConfigured(configuredList);
    } catch (err) {
      setMessage({ type: 'error', text: 'Failed to load brokers: ' + err });
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { loadData(); }, [loadData]);

  const isConfigured = useCallback((id) => configured.includes(id), [configured]);

  const saveBroker = useCallback(async (broker, credentials, waiverAccepted) => {
    setMessage(null);
    if (broker.waiver_required && !waiverAccepted) {
      setMessage({ type: 'error', text: 'You must accept the waiver to proceed.' });
      return false;
    }

    const missingFields = broker.credential_fields
      .filter(f => f.required && !credentials[f.name]?.trim())
      .map(f => f.label);

    if (missingFields.length > 0) {
      setMessage({ type: 'error', text: 'Missing required fields: ' + missingFields.join(', ') });
      return false;
    }

    try {
      await invoke('configure_broker', { broker: broker.id, credentials });
      setMessage({ type: 'success', text: `${broker.name} credentials saved.` });
      const updated = await invoke('list_configured_brokers');
      setConfigured(updated);
      return true;
    } catch (err) {
      setMessage({ type: 'error', text: 'Failed to save: ' + err });
      return false;
    }
  }, []);

  const deleteBroker = useCallback(async (brokerId) => {
    setMessage(null);
    try {
      await invoke('delete_broker_credentials', { broker: brokerId });
      const broker = brokers.find(b => b.id === brokerId);
      setMessage({ type: 'success', text: `${broker?.name || brokerId} credentials deleted.` });
      const updated = await invoke('list_configured_brokers');
      setConfigured(updated);
      return true;
    } catch (err) {
      setMessage({ type: 'error', text: 'Failed to delete: ' + err });
      return false;
    }
  }, [brokers]);

  return {
    brokers,
    configured,
    loading,
    message,
    setMessage,
    saveBroker,
    deleteBroker,
    isConfigured,
  };
}
