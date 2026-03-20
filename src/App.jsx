window.__APP_LOAD_TIME = Date.now();
import { AuthProvider, useAuth } from './contexts/AuthContext';
import { PreferencesProvider } from './contexts/PreferencesContext';
import LoginPage from './pages/LoginPage';
import WorkspaceShell from './shell/WorkspaceShell';
import { Suspense, Component } from 'react';
import { PANELS } from './docking/panelRegistry';

class ErrorBoundary extends Component {
  state = { error: null };
  static getDerivedStateFromError(error) { return { error }; }
  render() {
    if (this.state.error) {
      const resetApp = async () => {
        try {
          const { invoke } = await import('@tauri-apps/api/core');
          await invoke('clear_auth_token').catch(() => {});
          await invoke('save_workspace', { name: 'default', layoutJson: '{}' }).catch(() => {});
          // Also clear any corrupted preferences
          await invoke('save_preference', { key: 'reset', value: true }).catch(() => {});
        } catch {}
        localStorage.clear();
        sessionStorage.clear();
        // Force clear IndexedDB if present
        try { indexedDB.databases().then(dbs => dbs.forEach(db => indexedDB.deleteDatabase(db.name))); } catch {}
        this.setState({ error: null });
        window.location.reload();
      };
      // Auto-reset on first launch crash (if error happens within 5s of load)
      const timeSinceLoad = Date.now() - (window.__APP_LOAD_TIME || Date.now());
      const isFirstLoadCrash = timeSinceLoad < 5000;
      if (isFirstLoadCrash && !sessionStorage.getItem('auto_reset_attempted')) {
        sessionStorage.setItem('auto_reset_attempted', '1');
        resetApp();
        return null;
      }
      return (
        <div style={{ minHeight: '100vh', display: 'flex', alignItems: 'center', justifyContent: 'center', background: '#0D1117', color: '#E6EDF3', fontFamily: 'system-ui, sans-serif' }}>
          <div style={{ textAlign: 'center', maxWidth: 480, padding: 32 }}>
            <div style={{ fontSize: 48, marginBottom: 16 }}>⚠</div>
            <h2 style={{ fontSize: 18, marginBottom: 8 }}>Something went wrong</h2>
            <p style={{ color: '#8B949E', fontSize: 13, marginBottom: 16 }}>{String(this.state.error)}</p>
            <div style={{ display: 'flex', gap: 12, justifyContent: 'center' }}>
              <button onClick={() => { this.setState({ error: null }); window.location.reload(); }}
                style={{ padding: '8px 20px', background: '#58A6FF', color: '#fff', border: 'none', borderRadius: 8, cursor: 'pointer', fontSize: 13 }}>
                Reload
              </button>
              <button onClick={resetApp}
                style={{ padding: '8px 20px', background: '#F85149', color: '#fff', border: 'none', borderRadius: 8, cursor: 'pointer', fontSize: 13 }}>
                Reset App
              </button>
            </div>
          </div>
        </div>
      );
    }
    return this.props.children;
  }
}

function PopOutPanel({ panelId }) {
  const panel = PANELS[panelId];
  if (!panel) return <div className="p-4 text-aura-muted">Unknown panel</div>;
  const Component = panel.component;
  return (
    <Suspense fallback={<div className="p-4 text-aura-muted animate-pulse">Loading...</div>}>
      <div className="min-h-screen bg-aura-bg p-3">
        <Component />
      </div>
    </Suspense>
  );
}

function AuthGate() {
  const { isAuthenticated, loading } = useAuth();

  if (loading) {
    return (
      <div className="min-h-screen flex items-center justify-center bg-aura-bg">
        <div className="text-center">
          <div className="inline-flex items-center justify-center w-12 h-12 rounded-xl mb-3 animate-pulse"
               style={{ background: 'linear-gradient(135deg, #58A6FF, #BC8CFF, #3FB950)' }}>
            <span className="text-white text-xl font-bold">A</span>
          </div>
          <p className="text-aura-muted text-sm">Loading...</p>
        </div>
      </div>
    );
  }

  if (!isAuthenticated) return <LoginPage />;

  // Check for pop-out panel
  const params = new URLSearchParams(window.location.search);
  const panelId = params.get('panel');
  if (panelId) return <PopOutPanel panelId={panelId} />;

  return <WorkspaceShell />;
}

export default function App() {
  return (
    <ErrorBoundary>
      <AuthProvider>
        <PreferencesProvider>
          <AuthGate />
        </PreferencesProvider>
      </AuthProvider>
    </ErrorBoundary>
  );
}
