import { createContext, useContext, useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';

const AuthContext = createContext(null);

// Standalone desktop user — no server required
const STANDALONE_USER = {
  username: 'desktop_user',
  email: 'local@auraalpha.cc',
  tier: 'pro',
  display_name: 'Desktop User',
  standalone: true,
};
const STANDALONE_TOKEN = 'standalone_desktop_token';

export function AuthProvider({ children }) {
  const [user, setUser] = useState(null);
  const [token, setToken] = useState(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    (async () => {
      try {
        const auth = await invoke('load_auth_token');
        if (auth?.access_token && auth.access_token !== 'null' && auth.access_token !== null) {
          setToken(auth.access_token);
          const u = auth.user ? (typeof auth.user === 'string' ? JSON.parse(auth.user) : auth.user) : null;
          setUser(u);
        } else {
          // No saved token — auto-login as standalone desktop user
          setToken(STANDALONE_TOKEN);
          setUser(STANDALONE_USER);
        }
      } catch (e) {
        console.warn('[Auth] Failed to load token, entering standalone mode:', e);
        setToken(STANDALONE_TOKEN);
        setUser(STANDALONE_USER);
      } finally {
        setLoading(false);
      }
    })();

    const onLogout = () => logout();
    window.addEventListener('auth:logout', onLogout);
    return () => window.removeEventListener('auth:logout', onLogout);
  }, []);

  const login = useCallback(async (username, password, remember = true) => {
    // Try server login first (local API → cloud fallback)
    try {
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
          throw e;
        }
      }
      const data = JSON.parse(text);
      setToken(data.access_token);
      setUser(data.user || { username });
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
    } catch (serverErr) {
      // Server unreachable — fall back to standalone mode
      console.warn('[Auth] Server login failed, entering standalone mode:', serverErr);
      setToken(STANDALONE_TOKEN);
      setUser({ ...STANDALONE_USER, username, display_name: username });
      if (remember) {
        await invoke('save_auth_token', {
          accessToken: STANDALONE_TOKEN,
          refreshToken: '',
          userJson: JSON.stringify({ ...STANDALONE_USER, username }),
        }).catch(() => {});
      }
      return { access_token: STANDALONE_TOKEN, user: STANDALONE_USER };
    }
  }, []);

  const logout = useCallback(async () => {
    setToken(null);
    setUser(null);
    try {
      await invoke('clear_auth_token');
    } catch (e) {
      console.warn('[Auth] clear failed:', e);
    }
  }, []);

  return (
    <AuthContext.Provider value={{ user, token, loading, login, logout, isAuthenticated: !!token }}>
      {children}
    </AuthContext.Provider>
  );
}

export function useAuth() {
  const ctx = useContext(AuthContext);
  if (!ctx) throw new Error('useAuth must be inside AuthProvider');
  return ctx;
}
