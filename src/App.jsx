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
      return (
        <div style={{ minHeight: '100vh', display: 'flex', alignItems: 'center', justifyContent: 'center', background: '#0D1117', color: '#E6EDF3', fontFamily: 'system-ui, sans-serif' }}>
          <div style={{ textAlign: 'center', maxWidth: 480, padding: 32 }}>
            <div style={{ fontSize: 48, marginBottom: 16 }}>⚠</div>
            <h2 style={{ fontSize: 18, marginBottom: 8 }}>Something went wrong</h2>
            <p style={{ color: '#8B949E', fontSize: 13, marginBottom: 16 }}>{String(this.state.error)}</p>
            <button onClick={() => { this.setState({ error: null }); window.location.reload(); }}
              style={{ padding: '8px 20px', background: '#58A6FF', color: '#fff', border: 'none', borderRadius: 8, cursor: 'pointer', fontSize: 13 }}>
              Reload
            </button>
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
