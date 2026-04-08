import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';

export default function StartupPage({ onReady, onNeedsLogin }) {
  const [status, setStatus] = useState('checking');
  const [message, setMessage] = useState('Starting Aura Alpha...');
  const [crashDetected, setCrashDetected] = useState(false);

  useEffect(() => {
    let cancelled = false;

    (async () => {
      try {
        setMessage('Running startup checks...');
        const result = await invoke('startup_check');

        if (cancelled) return;

        if (result.previous_crash) {
          setCrashDetected(true);
          setMessage('Previous session did not close cleanly. Recovering...');
          await new Promise(r => setTimeout(r, 1500));
          if (cancelled) return;
        }

        setMessage('Checking connection...');
        let apiReachable = result.api_reachable;

        if (!apiReachable) {
          setMessage('Connecting to Aura Alpha servers...');
          for (let i = 0; i < 3; i++) {
            await new Promise(r => setTimeout(r, 2000));
            if (cancelled) return;
            try {
              const retry = await invoke('startup_check');
              if (retry.api_reachable) {
                apiReachable = true;
                break;
              }
            } catch {}
          }
        }

        if (cancelled) return;

        setMessage('Restoring session...');
        const auth = await invoke('load_auth_token').catch(() => null);

        if (cancelled) return;

        if (auth?.access_token && auth.access_token !== 'null' && auth.access_token !== null) {
          setStatus('ready');
          setMessage('Welcome back!');
          await new Promise(r => setTimeout(r, 500));
          if (!cancelled) onReady(auth);
        } else {
          setStatus('login');
          onNeedsLogin();
        }
      } catch (err) {
        if (import.meta.env.DEV) console.error('[Startup] failed:', err);
        if (!cancelled) {
          setMessage('Startup failed. Please restart.');
          setStatus('error');
        }
      }
    })();

    return () => { cancelled = true; };
  }, [onReady, onNeedsLogin]);

  return (
    <div style={{
      height: '100vh', display: 'flex', flexDirection: 'column',
      alignItems: 'center', justifyContent: 'center',
      background: '#0D1117', color: '#E6EDF3', fontFamily: "'Plus Jakarta Sans', sans-serif",
    }}>
      {/* Logo */}
      <div style={{
        display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
        width: 56, height: 56, borderRadius: 16, marginBottom: 16,
        background: 'linear-gradient(135deg, #58A6FF, #BC8CFF, #3FB950)',
      }}>
        <span style={{ color: '#fff', fontSize: 28, fontWeight: 700 }}>A</span>
      </div>

      <div style={{ fontSize: 32, fontWeight: 700, marginBottom: 8 }}>Aura Alpha</div>
      <div style={{ fontSize: 14, color: '#8B949E', marginBottom: 32 }}>{message}</div>

      {status === 'checking' && (
        <div style={{
          width: 200, height: 3, background: '#30363D', borderRadius: 2, overflow: 'hidden',
        }}>
          <div style={{
            width: '40%', height: '100%', background: '#58A6FF', borderRadius: 2,
            animation: 'startup-slide 1.5s infinite',
          }} />
        </div>
      )}

      {crashDetected && (
        <div style={{
          marginTop: 16, padding: '8px 16px', background: '#D2992211',
          border: '1px solid #D2992244', borderRadius: 8,
          fontSize: 13, color: '#D29922',
        }}>
          Recovered from previous crash. Your data is safe.
        </div>
      )}

      {status === 'error' && (
        <button
          onClick={() => window.location.reload()}
          style={{
            marginTop: 16, padding: '10px 24px', background: '#58A6FF',
            border: 'none', borderRadius: 8, color: '#0D1117',
            fontWeight: 600, cursor: 'pointer', fontSize: 14,
          }}
        >
          Retry
        </button>
      )}

      <style>{`
        @keyframes startup-slide {
          0% { transform: translateX(-100%); }
          100% { transform: translateX(350%); }
        }
      `}</style>
    </div>
  );
}
