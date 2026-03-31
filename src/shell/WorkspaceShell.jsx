import { useState, useEffect, useCallback, useRef, Suspense, Component } from 'react';
import { Layout, Model, Actions } from 'flexlayout-react';
import 'flexlayout-react/style/dark.css';
import './flexTheme.css';
import { invoke } from '@tauri-apps/api/core';
import TopBar from './TopBar';
import { PANELS } from '../docking/panelRegistry';
import { DEFAULT_LAYOUT } from '../docking/defaultLayout';
import CommandPalette from '../components/CommandPalette';
import { useAuth } from '../contexts/AuthContext';
import { usePreferences } from '../contexts/PreferencesContext';

// Error boundary — catches panel render crashes without killing the whole workspace
class PanelErrorBoundary extends Component {
  state = { error: null };
  static getDerivedStateFromError(error) { return { error }; }
  componentDidCatch(err) { console.error('[Panel crash]', this.props.panelId, err); }
  render() {
    if (this.state.error) {
      return (
        <div style={{ padding: 16, color: '#F85149', fontFamily: 'system-ui', fontSize: 13 }}>
          <div style={{ fontWeight: 600, marginBottom: 4 }}>Panel Error</div>
          <div style={{ color: '#8B949E' }}>{this.state.error.message}</div>
          <button
            onClick={() => this.setState({ error: null })}
            style={{ marginTop: 8, padding: '4px 12px', background: '#30363D', color: '#E6EDF3', border: '1px solid #484F58', borderRadius: 6, cursor: 'pointer', fontSize: 12 }}
          >
            Retry
          </button>
        </div>
      );
    }
    return this.props.children;
  }
}

function PanelWrapper({ panelId }) {
  const panel = PANELS[panelId];
  if (!panel) return <div className="p-4 text-aura-muted">Unknown panel: {panelId}</div>;
  const PanelComponent = panel.component;
  return (
    <PanelErrorBoundary panelId={panelId}>
      <Suspense fallback={<div className="p-4 text-aura-muted animate-pulse">Loading {panel.title}...</div>}>
        <div className="panel-content" style={{ height: '100%', overflow: 'auto' }}>
          <PanelComponent />
        </div>
      </Suspense>
    </PanelErrorBoundary>
  );
}

export default function WorkspaceShell() {
  const [model, setModel] = useState(() => Model.fromJson(DEFAULT_LAYOUT));
  const layoutRef = useRef(null);
  const [activeWorkspace, setActiveWorkspace] = useState('default');
  const [workspaces, setWorkspaces] = useState([]);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const { logout } = useAuth();
  const { prefs, setPref } = usePreferences();

  useEffect(() => {
    invoke('list_workspaces').then(setWorkspaces).catch(() => {});
  }, []);

  // Render panel content for FlexLayout
  const factory = useCallback((node) => {
    const component = node.getComponent();
    return <PanelWrapper panelId={component} />;
  }, []);

  const handleWorkspaceChange = useCallback(async (name) => {
    setActiveWorkspace(name);
    if (name === 'default') {
      setModel(Model.fromJson(DEFAULT_LAYOUT));
      return;
    }
    try {
      const json = await invoke('load_workspace', { name });
      const layout = JSON.parse(json);
      setModel(Model.fromJson(layout));
    } catch (e) {
      console.warn('Failed to load workspace, falling back to default:', e);
      setModel(Model.fromJson(DEFAULT_LAYOUT));
    }
  }, []);

  const handleAddPanel = useCallback((panelId) => {
    const panel = PANELS[panelId];
    if (!panel || !model) return;
    model.doAction(Actions.addNode(
      { type: 'tab', name: panel.title, component: panelId },
      model.getRoot().getId(),
      0, // insert at first position
      false
    ));
  }, [model]);

  const handlePaletteAction = useCallback((actionId) => {
    switch (actionId) {
      case 'check-health':
        invoke('check_health').catch(e => console.warn('Health check failed:', e));
        break;
      case 'toggle-compact':
        setPref('compactMode', !prefs.compactMode);
        break;
      case 'refresh-data':
        window.dispatchEvent(new CustomEvent('aura:refresh'));
        break;
      case 'pop-out':
        invoke('create_panel_window').catch(e => console.warn('Pop-out failed:', e));
        break;
      case 'logout':
        logout();
        break;
      default:
        console.warn('Unknown action:', actionId);
    }
  }, [prefs.compactMode, setPref, logout]);

  useEffect(() => {
    const handler = (e) => {
      if ((e.ctrlKey || e.metaKey) && e.key === 'k') {
        e.preventDefault();
        setPaletteOpen(prev => !prev);
      }
      if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === 'P') {
        e.preventDefault();
        setPaletteOpen(prev => !prev);
      }
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, []);

  const handleSaveWorkspace = useCallback(async () => {
    if (!model) return;
    const layout = model.toJson();
    const name = prompt('Workspace name:', activeWorkspace === 'default' ? '' : activeWorkspace);
    if (!name) return;
    try {
      await invoke('save_workspace', { name, layoutJson: JSON.stringify(layout) });
      setActiveWorkspace(name);
      const list = await invoke('list_workspaces');
      setWorkspaces(list);
    } catch (e) {
      console.warn('Failed to save workspace:', e);
    }
  }, [model, activeWorkspace]);

  return (
    <div className="h-screen flex flex-col bg-aura-bg">
      <TopBar
        activeWorkspace={activeWorkspace}
        onWorkspaceChange={handleWorkspaceChange}
        workspaces={workspaces}
        onSaveWorkspace={handleSaveWorkspace}
        onAddPanel={handleAddPanel}
      />
      <div className="flex-1 relative">
        <Layout
          ref={layoutRef}
          model={model}
          factory={factory}
        />
      </div>
      <CommandPalette
        open={paletteOpen}
        onClose={() => setPaletteOpen(false)}
        onAddPanel={handleAddPanel}
        onAction={handlePaletteAction}
      />
    </div>
  );
}
