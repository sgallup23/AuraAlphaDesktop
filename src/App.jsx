window.__APP_LOAD_TIME = Date.now();
import { AuthProvider, useAuth } from './contexts/AuthContext';
import { PreferencesProvider } from './contexts/PreferencesContext';
import { ConfigProvider } from './contexts/ConfigContext';
import LoginPage from './pages/LoginPage';
import StartupPage from './pages/StartupPage';
import ExplorerPage from './pages/ExplorerPage';
import WorkspaceShell from './shell/WorkspaceShell';
import ForceUpdateModal from './components/ForceUpdateModal';
import UpdateAvailableBanner from './components/UpdateAvailableBanner';
import { Suspense, Component, useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { PANELS } from './docking/panelRegistry';

class ErrorBoundary extends Component {
  state = { error: null };
  static getDerivedStateFromError(error) { return { error }; }
  render() {
    if (this.state.error) {
      const resetApp = async () => {
        try {
          const { invoke } = await import('@tauri-apps/api/core');
          // Preserve auth credentials — only reset workspace/preferences
          await invoke('save_workspace', { name: 'default', layoutJson: '{}' }).catch(() => {});
          await invoke('save_preference', { key: 'reset', value: true }).catch(() => {});
        } catch {}
        // Don't clear localStorage entirely — preserve auth tokens
        const token = localStorage.getItem('aura_token');
        const user = localStorage.getItem('aura_user');
        localStorage.clear();
        if (token) localStorage.setItem('aura_token', token);
        if (user) localStorage.setItem('aura_user', user);
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
            <div style={{ fontSize: 48, marginBottom: 16 }}>&#x26A0;</div>
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

function AppWithStartup() {
  // 'startup' = show startup screen, 'explorer' = demo backtest, 'app' = normal auth flow
  const [phase, setPhase] = useState('startup');

  // Version enforcement state
  // null = not yet checked, object = result from check_for_update
  const [updateInfo, setUpdateInfo] = useState(null);
  // latestVersion fetched from update_endpoint for the soft-update banner
  const [latestVersion, setLatestVersion] = useState(null);
  const [bannerDismissed, setBannerDismissed] = useState(false);

  // Register clean shutdown handler
  useEffect(() => {
    const handleBeforeUnload = () => {
      // Fire-and-forget — browser may kill us before this completes,
      // but the lock file cleanup is best-effort
      try { invoke('clean_shutdown'); } catch {}
    };
    window.addEventListener('beforeunload', handleBeforeUnload);
    return () => window.removeEventListener('beforeunload', handleBeforeUnload);
  }, []);

  // Run version check as soon as startup transitions to 'app'.
  // check_for_update reads AppConfigState which is populated during Tauri setup,
  // so by the time StartupPage calls onReady/onNeedsLogin the config is ready.
  const runVersionCheck = useCallback(async () => {
    try {
      const info = await invoke('check_for_update');
      setUpdateInfo(info);

      // If not force-required, try to learn the latest available version from
      // the update_endpoint so the soft banner can show a meaningful label.
      if (!info.update_required && info.update_endpoint) {
        try {
          const res = await fetch(info.update_endpoint, {
            method: 'GET',
            headers: { Accept: 'application/json' },
            signal: AbortSignal.timeout(5000),
          });
          if (res.ok) {
            const data = await res.json();
            // Support common update-manifest shapes: { version } or { latest_version }
            const ver = data.version || data.latest_version || null;
            if (ver) setLatestVersion(ver);
          }
        } catch {
          // Non-fatal — banner simply won't appear without a latest version
        }
      }
    } catch (err) {
      console.warn('[App] check_for_update failed:', err);
      // Non-fatal — proceed normally if the command errors
      setUpdateInfo({ update_required: false });
    }
  }, []);

  const handleReady = useCallback(() => {
    // Auth was restored from store — go straight to app (AuthContext will pick it up)
    setPhase('app');
    runVersionCheck();
    // Auto-start grid worker after login (5s delay for app to settle)
    setTimeout(async () => {
      try {
        const { invoke: inv } = await import('@tauri-apps/api/core');
        const ws = await inv('grid_worker_status').catch(() => null);
        if (ws && !ws.running) {
          await inv('start_grid_worker').catch(() => {});
          console.log('[App] Grid worker auto-started after login');
        }
      } catch {}
    }, 5000);
  }, [runVersionCheck]);

  const handleNeedsLogin = useCallback(() => {
    // No saved auth — show explorer mode first (first-run experience)
    // User sees instant demo backtest results before being asked to sign in
    setPhase('explorer');
    runVersionCheck();
  }, [runVersionCheck]);

  if (phase === 'startup') {
    return <StartupPage onReady={handleReady} onNeedsLogin={handleNeedsLogin} />;
  }

  if (phase === 'explorer') {
    return <ExplorerPage onSignIn={() => setPhase('app')} />;
  }

  // Block the entire app if the running version is below the enforced minimum
  if (updateInfo?.update_required) {
    return (
      <ForceUpdateModal
        currentVersion={updateInfo.current_version}
        minimumVersion={updateInfo.minimum_version}
        downloadUrl={updateInfo.update_endpoint}
      />
    );
  }

  // Determine whether the soft-update banner should be shown:
  // latestVersion must be known and greater than current_version, and not dismissed
  const showBanner =
    !bannerDismissed &&
    latestVersion != null &&
    updateInfo != null &&
    latestVersion !== updateInfo.current_version;

  return (
    <>
      {showBanner && (
        <UpdateAvailableBanner
          latestVersion={latestVersion}
          currentVersion={updateInfo?.current_version}
          downloadUrl={updateInfo?.update_endpoint}
          onDismiss={() => setBannerDismissed(true)}
        />
      )}
      <ConfigProvider>
        <AuthProvider>
          <PreferencesProvider>
            <AuthGate />
          </PreferencesProvider>
        </AuthProvider>
      </ConfigProvider>
    </>
  );
}

export default function App() {
  return (
    <ErrorBoundary>
      <AppWithStartup />
    </ErrorBoundary>
  );
}
