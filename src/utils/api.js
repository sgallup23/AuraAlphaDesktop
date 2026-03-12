import { invoke } from '@tauri-apps/api/core';

const API_BASE = 'https://auraalpha.cc/api';

let refreshPromise = null;

export function getApiBase() {
  return API_BASE;
}

async function getToken() {
  try {
    const auth = await invoke('load_auth_token');
    return auth?.access_token || null;
  } catch {
    return null;
  }
}

async function getRefreshToken() {
  try {
    const auth = await invoke('load_auth_token');
    return auth?.refresh_token || null;
  } catch {
    return null;
  }
}

async function setTokens(access, refresh, user) {
  try {
    await invoke('save_auth_token', {
      accessToken: access,
      refreshToken: refresh,
      userJson: typeof user === 'string' ? user : JSON.stringify(user),
    });
  } catch (e) {
    console.warn('[Auth] Failed to save tokens:', e);
  }
}

async function tryRefreshToken() {
  if (refreshPromise) return refreshPromise;
  refreshPromise = (async () => {
    const rt = await getRefreshToken();
    if (!rt) throw new Error('No refresh token');
    const res = await fetch(`${API_BASE}/auth/refresh`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ refresh_token: rt }),
    });
    if (!res.ok) throw new Error('Refresh failed');
    const data = await res.json();
    await setTokens(data.access_token, data.refresh_token || rt, data.user || '{}');
    return data.access_token;
  })();
  try {
    return await refreshPromise;
  } finally {
    refreshPromise = null;
  }
}

export async function api(path, options = {}) {
  const { silent = false, signal, method, headers: extraHeaders, body, ...rest } = options;
  const token = await getToken();
  const url = path.startsWith('http') ? path : `${API_BASE}${path.startsWith('/') ? '' : '/'}${path}`;
  const headers = { ...extraHeaders };
  if (token) headers['Authorization'] = `Bearer ${token}`;
  if (body && typeof body === 'object' && !(body instanceof FormData)) {
    headers['Content-Type'] = 'application/json';
  }

  const fetchOpts = {
    method: method || (body ? 'POST' : 'GET'),
    headers,
    signal,
    ...rest,
  };
  if (body) {
    fetchOpts.body = typeof body === 'string' || body instanceof FormData ? body : JSON.stringify(body);
  }

  try {
    let res = await fetch(url, fetchOpts);
    if (res.status === 401 && token) {
      try {
        const newToken = await tryRefreshToken();
        headers['Authorization'] = `Bearer ${newToken}`;
        res = await fetch(url, { ...fetchOpts, headers });
      } catch {
        if (!silent) {
          window.dispatchEvent(new CustomEvent('auth:logout'));
        }
        return null;
      }
    }
    if (!res.ok) {
      if (silent) return null;
      throw new Error(`API ${res.status}: ${res.statusText}`);
    }
    const text = await res.text();
    try { return JSON.parse(text); } catch { return text; }
  } catch (e) {
    if (silent) return null;
    throw e;
  }
}

export async function apiPost(path, body) {
  return api(path, { method: 'POST', body });
}
export async function apiPut(path, body) {
  return api(path, { method: 'PUT', body });
}
export async function apiDelete(path) {
  return api(path, { method: 'DELETE' });
}
