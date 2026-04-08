import { createContext, useContext, useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';

const AuthContext = createContext(null);

// Standalone desktop user — limited free tier, no server required
const STANDALONE_USER = {
  username: 'explorer',
  email: '',
  tier: 'explorer',
  display_name: 'Explorer (Offline)',
  standalone: true,
};
const STANDALONE_TOKEN = 'standalone_offline';

export function AuthProvider({ children }) {
  const [user, setUser] = useState(null);
  const [token, setToken] = useState(null);
  const [loading, setLoading] = useState(true);
  const [connectionStatus, setConnectionStatus] = useState('checking'); // checking | online | offline

  useEffect(() => {
    (async () => {
      try {
        const auth = await invoke('load_auth_token');
        if (auth?.access_token && auth.access_token !== 'null' && auth.access_token !== null
            && auth.access_token !== STANDALONE_TOKEN) {
          setToken(auth.access_token);
          const u = auth.user ? (typeof auth.user === 'string' ? JSON.parse(auth.user) : auth.user) : null;
          setUser(u);
          setConnectionStatus('online');
        } else {
          // No saved cloud token — enter standalone mode (free tier)
          setToken(STANDALONE_TOKEN);
          setUser(STANDALONE_USER);
          setConnectionStatus('offline');
        }
      } catch (e) {
        if (import.meta.env.DEV) console.warn('[Auth] Failed to load token, entering standalone mode:', e);
        setToken(STANDALONE_TOKEN);
        setUser(STANDALONE_USER);
        setConnectionStatus('offline');
      } finally {
        setLoading(false);
      }
    })();

    const onLogout = () => logout();
    window.addEventListener('auth:logout', onLogout);
    return () => window.removeEventListener('auth:logout', onLogout);
  }, []);

  const login = useCallback(async (username, password, remember = true) => {
    // Try server login (local API → cloud fallback)
    let text;
    for (let attempt = 0; attempt < 2; attempt++) {
      try {
        text = await invoke('api_proxy', {
          method: 'POST',
          path: '/api/auth/login',
          body: JSON.stringify({ username, password }),
          authToken: null,
        });
        break;
      } catch (e) {
        if (attempt < 1) {
          await new Promise(r => setTimeout(r, 1000));
          continue;
        }
        throw new Error(
          'Unable to reach Aura Alpha servers. Check your network connection or use Explorer mode (offline).'
        );
      }
    }
    const data = JSON.parse(text);
    setToken(data.access_token);
    setUser(data.user || { username });
    setConnectionStatus('online');
    if (remember) {
      await invoke('save_auth_token', {
        accessToken: data.access_token,
        refreshToken: data.refresh_token || '',
        userJson: JSON.stringify(data.user || { username }),
      });
    }
    // Auto-start grid worker after successful login
    setTimeout(async () => {
      try {
        const ws = await invoke('grid_worker_status').catch(() => null);
        if (ws && !ws.running) {
          await invoke('start_grid_worker').catch(() => {});
        }
      } catch {}
    }, 3000);
    return data;
  }, []);

  const enterStandaloneMode = useCallback(() => {
    setToken(STANDALONE_TOKEN);
    setUser(STANDALONE_USER);
    setConnectionStatus('offline');
  }, []);

  const logout = useCallback(async () => {
    setToken(null);
    setUser(null);
    setConnectionStatus('checking');
    try {
      await invoke('clear_auth_token');
    } catch (e) {
      if (import.meta.env.DEV) console.warn('[Auth] clear failed:', e);
    }
  }, []);

  return (
    <AuthContext.Provider value={{
      user, token, loading, login, logout, enterStandaloneMode,
      isAuthenticated: !!token,
      isStandalone: user?.standalone === true,
      connectionStatus,
    }}>
      {children}
    </AuthContext.Provider>
  );
}

export function useAuth() {
  const ctx = useContext(AuthContext);
  if (!ctx) throw new Error('useAuth must be inside AuthProvider');
  return ctx;
}
