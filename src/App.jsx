import { AuthProvider, useAuth } from './contexts/AuthContext';
import { PreferencesProvider } from './contexts/PreferencesContext';
import LoginPage from './pages/LoginPage';
import WorkspaceShell from './shell/WorkspaceShell';
import { Suspense } from 'react';
import { PANELS } from './docking/panelRegistry';

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
    <AuthProvider>
      <PreferencesProvider>
        <AuthGate />
      </PreferencesProvider>
    </AuthProvider>
  );
}
