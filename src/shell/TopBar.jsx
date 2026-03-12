import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useAuth } from '../contexts/AuthContext';
import PanelMenu from '../components/PanelMenu';

export default function TopBar({ activeWorkspace, onWorkspaceChange, workspaces, onSaveWorkspace, onAddPanel }) {
  const { user, logout } = useAuth();
  const [health, setHealth] = useState(null);
  const [showMenu, setShowMenu] = useState(false);

  useEffect(() => {
    const check = async () => {
      try {
        const h = await invoke('check_health');
        setHealth(h);
      } catch { setHealth(null); }
    };
    check();
    const iv = setInterval(check, 30000);
    return () => clearInterval(iv);
  }, []);

  return (
    <div className="h-12 flex items-center justify-between px-4 bg-aura-surface border-b border-aura-border select-none" style={{ WebkitAppRegion: 'drag' }}>
      {/* Left: Logo + status */}
      <div className="flex items-center gap-3" style={{ WebkitAppRegion: 'no-drag' }}>
        <div className="flex items-center gap-2">
          <div className="w-7 h-7 rounded-lg flex items-center justify-center text-sm font-bold text-white"
               style={{ background: 'linear-gradient(135deg, #58A6FF, #BC8CFF, #3FB950)' }}>A</div>
          <span className="text-sm font-semibold text-aura-text">Aura Alpha</span>
        </div>
        <div className="flex items-center gap-1.5 ml-2">
          <span className={`status-dot ${health?.api_up ? 'bg-aura-green' : 'bg-aura-red'}`} />
          <span className="text-xs text-aura-muted">
            {health ? `${health.bots_active} bots | ${health.total_positions} pos` : 'Connecting...'}
          </span>
        </div>
      </div>

      {/* Center: Workspace selector */}
      <div className="flex items-center gap-2" style={{ WebkitAppRegion: 'no-drag' }}>
        <select
          value={activeWorkspace || 'default'}
          onChange={(e) => onWorkspaceChange?.(e.target.value)}
          className="text-xs bg-aura-surface2 border border-aura-border rounded px-2 py-1 text-aura-text outline-none"
        >
          <option value="default">Default Layout</option>
          {workspaces?.map(w => <option key={w} value={w}>{w}</option>)}
        </select>
        <button onClick={onSaveWorkspace} className="text-xs text-aura-muted hover:text-aura-blue transition-colors">
          Save
        </button>
        <PanelMenu onAddPanel={onAddPanel} />
      </div>

      {/* Right: User + logout */}
      <div className="flex items-center gap-3" style={{ WebkitAppRegion: 'no-drag' }}>
        <span className="text-xs text-aura-muted">{user?.username || 'User'}</span>
        <button onClick={logout} className="text-xs text-aura-muted hover:text-aura-red transition-colors">
          Logout
        </button>
      </div>
    </div>
  );
}
