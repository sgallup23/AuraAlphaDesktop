import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';

const SECTIONS = [
  {
    title: 'Connection',
    settings: [
      { key: 'api_url', label: 'API Server', type: 'text', default: 'http://127.0.0.1:8020', desc: 'Backend server URL' },
    ],
  },
  {
    title: 'Trading',
    settings: [
      { key: 'trading_mode', label: 'Trading Mode', type: 'select', options: ['paper', 'live'], default: 'paper', desc: 'Paper mode uses simulated funds' },
      { key: 'max_daily_loss', label: 'Max Daily Loss %', type: 'number', default: 5, desc: 'Stop trading if daily loss exceeds this' },
      { key: 'kill_switch_enabled', label: 'Kill Switch', type: 'toggle', default: true, desc: 'Emergency stop for all trading' },
    ],
  },
  {
    title: 'Notifications',
    settings: [
      { key: 'notify_trades', label: 'Trade Notifications', type: 'toggle', default: true, desc: 'Alert on trade executions' },
      { key: 'notify_achievements', label: 'Achievement Alerts', type: 'toggle', default: true, desc: 'Show achievement popups' },
      { key: 'notify_rank', label: 'Rank Change Alerts', type: 'toggle', default: true, desc: 'Alert on leaderboard movement' },
    ],
  },
  {
    title: 'Appearance',
    settings: [
      { key: 'theme', label: 'Theme', type: 'select', options: ['dark', 'midnight', 'terminal'], default: 'dark', desc: 'UI color scheme' },
      { key: 'compact_mode', label: 'Compact Mode', type: 'toggle', default: false, desc: 'Reduce padding for more data density' },
    ],
  },
];

export default function SettingsPanel() {
  const [values, setValues] = useState({});
  const [saved, setSaved] = useState(false);
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    (async () => {
      try {
        // Load all preferences at once using the bulk loader
        const allPrefs = await invoke('load_preferences');
        const merged = {};
        for (const section of SECTIONS) {
          for (const s of section.settings) {
            merged[s.key] = (allPrefs && allPrefs[s.key] !== undefined) ? allPrefs[s.key] : s.default;
          }
        }
        setValues(merged);
      } catch {
        // Fall back to defaults
        const defaults = {};
        for (const section of SECTIONS) {
          for (const s of section.settings) {
            defaults[s.key] = s.default;
          }
        }
        setValues(defaults);
      }
      setLoaded(true);
    })();
  }, []);

  const update = async (key, value) => {
    setValues(prev => ({ ...prev, [key]: value }));
    setSaved(false);
    try {
      await invoke('save_preference', { key, value: JSON.parse(JSON.stringify(value)) });
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    } catch (err) {
      if (import.meta.env.DEV) console.warn('[Settings] save failed:', err);
    }
  };

  const inputStyle = {
    padding: '8px 12px', background: '#0D1117', border: '1px solid #30363D',
    borderRadius: 6, color: '#E6EDF3', fontSize: 14, outline: 'none',
  };

  if (!loaded) {
    return (
      <div style={{ padding: 24, color: '#8B949E', fontSize: 14 }}>Loading settings...</div>
    );
  }

  return (
    <div style={{ padding: 24, maxWidth: 600, color: '#E6EDF3' }}>
      <h2 style={{ fontSize: 20, fontWeight: 700, marginBottom: 4, marginTop: 0 }}>Settings</h2>
      <p style={{ fontSize: 13, color: '#8B949E', marginBottom: 24 }}>Configure your trading environment.</p>

      {saved && (
        <div style={{
          padding: '8px 12px', background: '#3FB95011', border: '1px solid #3FB95044',
          borderRadius: 6, color: '#3FB950', fontSize: 13, marginBottom: 16,
        }}>
          Settings saved
        </div>
      )}

      {SECTIONS.map(section => (
        <div key={section.title} style={{ marginBottom: 24 }}>
          <h3 style={{
            fontSize: 14, fontWeight: 600, color: '#8B949E', textTransform: 'uppercase',
            letterSpacing: 0.5, marginBottom: 12, marginTop: 0,
          }}>
            {section.title}
          </h3>
          {section.settings.map(s => (
            <div key={s.key} style={{
              display: 'flex', justifyContent: 'space-between', alignItems: 'center',
              padding: '12px 0', borderBottom: '1px solid #21262D',
            }}>
              <div style={{ flex: 1, marginRight: 16 }}>
                <div style={{ fontSize: 14, fontWeight: 500 }}>{s.label}</div>
                <div style={{ fontSize: 12, color: '#8B949E', marginTop: 2 }}>{s.desc}</div>
              </div>
              {s.type === 'toggle' ? (
                <button
                  onClick={() => update(s.key, !values[s.key])}
                  style={{
                    width: 44, height: 24, borderRadius: 12, border: 'none', cursor: 'pointer',
                    background: values[s.key] ? '#3FB950' : '#30363D', position: 'relative',
                    transition: 'background 0.2s', flexShrink: 0,
                  }}
                >
                  <div style={{
                    width: 18, height: 18, borderRadius: '50%', background: '#fff',
                    position: 'absolute', top: 3, left: values[s.key] ? 23 : 3,
                    transition: 'left 0.2s',
                  }} />
                </button>
              ) : s.type === 'select' ? (
                <select
                  value={values[s.key] ?? s.default}
                  onChange={e => update(s.key, e.target.value)}
                  style={{ ...inputStyle, minWidth: 120 }}
                >
                  {s.options.map(o => <option key={o} value={o}>{o}</option>)}
                </select>
              ) : s.type === 'number' ? (
                <input
                  type="number"
                  value={values[s.key] ?? s.default}
                  onChange={e => update(s.key, parseFloat(e.target.value) || 0)}
                  style={{ ...inputStyle, width: 80, textAlign: 'right' }}
                />
              ) : (
                <input
                  type="text"
                  value={values[s.key] ?? s.default}
                  onChange={e => update(s.key, e.target.value)}
                  style={{ ...inputStyle, width: 200 }}
                />
              )}
            </div>
          ))}
        </div>
      ))}

      <div style={{
        marginTop: 16, padding: '12px 16px', background: '#161B22',
        borderRadius: 8, border: '1px solid #21262D',
      }}>
        <div style={{ fontSize: 12, color: '#8B949E' }}>
          Settings are saved automatically and persist between sessions.
        </div>
      </div>
    </div>
  );
}
