import { createContext, useContext, useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';

const PreferencesContext = createContext(null);

const DEFAULTS = {
  defaultSymbol: 'SPY',
  defaultTimeframe: '6M',
  pollInterval: 10000,
  compactMode: false,
  showVolume: true,
};

export function PreferencesProvider({ children }) {
  const [prefs, setPrefs] = useState(DEFAULTS);
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    invoke('load_preferences').then(saved => {
      if (saved && typeof saved === 'object') {
        setPrefs(p => ({ ...p, ...saved }));
      }
    }).catch(() => {}).finally(() => setLoaded(true));
  }, []);

  const setPref = useCallback(async (key, value) => {
    setPrefs(p => ({ ...p, [key]: value }));
    try {
      await invoke('save_preference', { key, value: JSON.parse(JSON.stringify(value)) });
    } catch (e) {
      console.warn('[Prefs] save failed:', e);
    }
  }, []);

  return (
    <PreferencesContext.Provider value={{ prefs, setPref, loaded }}>
      {children}
    </PreferencesContext.Provider>
  );
}

export function usePreferences() {
  const ctx = useContext(PreferencesContext);
  if (!ctx) throw new Error('usePreferences must be inside PreferencesProvider');
  return ctx;
}
