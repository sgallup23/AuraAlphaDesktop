import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';

/**
 * WorkspaceManager — Save/load/list named workspace layouts via Tauri store.
 *
 * Usage:
 *   const wm = useWorkspaceManager(dockRef);
 *   <select value={wm.active} onChange={e => wm.load(e.target.value)}> ...
 *   <button onClick={wm.save}>Save</button>
 */
export function useWorkspaceManager(dockRef, defaultLayout) {
  const [active, setActive] = useState('default');
  const [workspaces, setWorkspaces] = useState([]);

  useEffect(() => {
    invoke('list_workspaces').then(setWorkspaces).catch(() => {});
  }, []);

  const load = useCallback(async (name) => {
    setActive(name);
    if (name === 'default') {
      dockRef.current?.loadLayout(defaultLayout);
      return;
    }
    try {
      const json = await invoke('load_workspace', { name });
      const layout = JSON.parse(json);
      dockRef.current?.loadLayout(layout);
    } catch (e) {
      console.warn('Failed to load workspace:', e);
    }
  }, [dockRef, defaultLayout]);

  const save = useCallback(async (name) => {
    const layout = dockRef.current?.saveLayout();
    if (!layout) return;
    const saveName = name || prompt('Workspace name:', active === 'default' ? '' : active);
    if (!saveName) return;
    try {
      await invoke('save_workspace', { name: saveName, layoutJson: JSON.stringify(layout) });
      setActive(saveName);
      const list = await invoke('list_workspaces');
      setWorkspaces(list);
    } catch (e) {
      console.warn('Failed to save workspace:', e);
    }
  }, [dockRef, active]);

  const remove = useCallback(async (name) => {
    // Note: requires a delete_workspace IPC command if needed
    console.warn('Delete workspace not yet implemented');
  }, []);

  return { active, workspaces, load, save, remove };
}

/**
 * WorkspaceSelector — Dropdown UI for workspace selection.
 */
export default function WorkspaceManager({ active, workspaces, onLoad, onSave }) {
  return (
    <div className="flex items-center gap-2">
      <select
        value={active || 'default'}
        onChange={(e) => onLoad?.(e.target.value)}
        className="text-xs bg-aura-surface2 border border-aura-border rounded px-2 py-1 text-aura-text outline-none"
      >
        <option value="default">Default Layout</option>
        {workspaces?.map(w => <option key={w} value={w}>{w}</option>)}
      </select>
      <button
        onClick={() => onSave?.()}
        className="text-xs text-aura-muted hover:text-aura-blue transition-colors"
      >
        Save
      </button>
    </div>
  );
}
