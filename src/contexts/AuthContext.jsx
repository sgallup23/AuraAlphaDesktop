import { createContext, useContext, useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { api } from '../utils/api';

const AuthContext = createContext(null);

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
        }
      } catch (e) {
        console.warn('[Auth] Failed to load token:', e);
      } finally {
        setLoading(false);
      }
    })();

    const onLogout = () => logout();
    window.addEventListener('auth:logout', onLogout);
    return () => window.removeEventListener('auth:logout', onLogout);
  }, []);

  const login = useCallback(async (username, password, remember = true) => {
    const res = await fetch('https://auraalpha.cc/api/auth/login', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ username, password }),
    });
    if (!res.ok) {
      const err = await res.json().catch(() => ({}));
      throw new Error(err.detail || `Login failed (${res.status})`);
    }
    const data = await res.json();
    setToken(data.access_token);
    setUser(data.user || { username });
    if (remember) {
      await invoke('save_auth_token', {
        accessToken: data.access_token,
        refreshToken: data.refresh_token || '',
        userJson: JSON.stringify(data.user || { username }),
      });
    }
    return data;
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
