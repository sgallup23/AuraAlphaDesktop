import { useState, useEffect, useCallback, useRef, Suspense } from 'react';
import DockLayout from 'rc-dock';
import 'rc-dock/dist/rc-dock-dark.css';
import '../docking/dockTheme.css';
import { invoke } from '@tauri-apps/api/core';
import TopBar from './TopBar';
import { PANELS } from '../docking/panelRegistry';
import { DEFAULT_LAYOUT } from '../docking/defaultLayout';
import CommandPalette from '../components/CommandPalette';
import { useAuth } from '../contexts/AuthContext';
import { usePreferences } from '../contexts/PreferencesContext';

function PanelWrapper({ panelId }) {
  const panel = PANELS[panelId];
  if (!panel) return <div className="p-4 text-aura-muted">Unknown panel: {panelId}</div>;
  const Component = panel.component;
  return (
    <Suspense fallback={<div className="p-4 text-aura-muted animate-pulse">Loading {panel.title}...</div>}>
      <div className="panel-content">
        <Component />
      </div>
    </Suspense>
  );
}

export default function WorkspaceShell() {
  const dockRef = useRef(null);
  const [activeWorkspace, setActiveWorkspace] = useState('default');
  const [workspaces, setWorkspaces] = useState([]);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const { logout } = useAuth();
  const { prefs, setPref } = usePreferences();

  useEffect(() => {
    invoke('list_workspaces').then(setWorkspaces).catch(() => {});
  }, []);

  const loadTab = useCallback((tab) => {
    return {
      ...tab,
      content: <PanelWrapper panelId={tab.id} />,
      closable: PANELS[tab.id]?.closable ?? true,
      minWidth: 200,
      minHeight: 150,
    };
  }, []);

  const handleWorkspaceChange = useCallback(async (name) => {
    setActiveWorkspace(name);
    if (name === 'default') {
      dockRef.current?.loadLayout(DEFAULT_LAYOUT);
      return;
    }
    try {
      const json = await invoke('load_workspace', { name });
      const layout = JSON.parse(json);
      dockRef.current?.loadLayout(layout);
    } catch (e) {
      console.warn('Failed to load workspace, falling back to default:', e);
      dockRef.current?.loadLayout(DEFAULT_LAYOUT);
    }
  }, []);

  const handleAddPanel = useCallback((panelId) => {
    const panel = PANELS[panelId];
    if (!panel || !dockRef.current) return;
    dockRef.current.dockMove(
      { id: `${panelId}-${Date.now()}`, title: panel.title, content: <PanelWrapper panelId={panelId} />, closable: true, minWidth: 200, minHeight: 150 },
      null,
      'float'
    );
  }, []);

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
      // Ctrl+N or Cmd+N: Add new chart panel
      if ((e.ctrlKey || e.metaKey) && e.key === 'n') {
        e.preventDefault();
        handleAddPanel('chart');
      }
      // Ctrl+B or Cmd+B: Toggle bot command
      if ((e.ctrlKey || e.metaKey) && e.key === 'b') {
        e.preventDefault();
        handleAddPanel('bot-command');
      }
      // Ctrl+K or Cmd+K: Open command palette
      if ((e.ctrlKey || e.metaKey) && e.key === 'k') {
        e.preventDefault();
        setPaletteOpen(prev => !prev);
      }
      // Ctrl+Shift+P or Cmd+Shift+P: Open command palette
      if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === 'P') {
        e.preventDefault();
        setPaletteOpen(prev => !prev);
      }
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [handleAddPanel]);

  const handleSaveWorkspace = useCallback(async () => {
    const layout = dockRef.current?.saveLayout();
    if (!layout) return;
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
  }, [activeWorkspace]);

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
        <DockLayout
          ref={dockRef}
          defaultLayout={DEFAULT_LAYOUT}
          loadTab={loadTab}
          style={{ position: 'absolute', inset: 0 }}
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
