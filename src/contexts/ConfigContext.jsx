import { createContext, useContext, useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';

const ConfigContext = createContext(null);

export function ConfigProvider({ children }) {
  const [config, setConfig] = useState(null);

  useEffect(() => {
    invoke('get_config').then(setConfig).catch(e => { if (import.meta.env.DEV) console.warn(e); });
  }, []);

  return (
    <ConfigContext.Provider value={config}>
      {children}
    </ConfigContext.Provider>
  );
}

export function useConfig() {
  return useContext(ConfigContext);
}
