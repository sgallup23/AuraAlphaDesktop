import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';

/**
 * Telemetry consent dialog — shown once on first launch.
 *
 * GDPR/CCPA requires explicit consent before collecting machine
 * identifiers (hostname, CPU cores, RAM) via the grid worker.
 * The choice is persisted via the Tauri store plugin and checked
 * by the Rust grid worker before registration/heartbeat.
 */
export default function TelemetryConsentDialog({ onDecided }) {
  const [visible, setVisible] = useState(false);

  useEffect(() => {
    (async () => {
      try {
        const consent = await invoke('get_telemetry_consent');
        if (!consent.decided) {
          setVisible(true);
        } else if (onDecided) {
          onDecided(consent.consented);
        }
      } catch (err) {
        if (import.meta.env.DEV) console.warn('[TelemetryConsent] Failed to check consent:', err);
        // If the command fails (e.g. not in Tauri), don't show dialog
      }
    })();
  }, []);

  const handleChoice = async (consented) => {
    try {
      await invoke('set_telemetry_consent', { consented });
    } catch (err) {
      if (import.meta.env.DEV) console.warn('[TelemetryConsent] Failed to save consent:', err);
    }
    setVisible(false);
    if (onDecided) onDecided(consented);
  };

  if (!visible) return null;

  return (
    <div style={{
      position: 'fixed',
      inset: 0,
      zIndex: 99999,
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      background: 'rgba(0, 0, 0, 0.7)',
      backdropFilter: 'blur(4px)',
    }}>
      <div style={{
        background: '#161B22',
        border: '1px solid #30363D',
        borderRadius: 12,
        padding: 32,
        maxWidth: 440,
        width: '90%',
        boxShadow: '0 8px 32px rgba(0,0,0,0.5)',
      }}>
        <div style={{
          display: 'flex',
          alignItems: 'center',
          gap: 10,
          marginBottom: 16,
        }}>
          <div style={{
            width: 36,
            height: 36,
            borderRadius: 8,
            background: 'linear-gradient(135deg, #58A6FF, #BC8CFF)',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            fontSize: 18,
            fontWeight: 700,
            color: '#fff',
            flexShrink: 0,
          }}>A</div>
          <h3 style={{
            margin: 0,
            fontSize: 16,
            fontWeight: 600,
            color: '#E6EDF3',
          }}>Research Grid Data Sharing</h3>
        </div>

        <p style={{
          color: '#8B949E',
          fontSize: 13,
          lineHeight: 1.6,
          margin: '0 0 12px 0',
        }}>
          Help improve Aura Alpha by sharing anonymous hardware data with the
          research grid? This includes your hostname, CPU core count, and RAM
          size, which helps optimize job distribution across contributors.
        </p>

        <p style={{
          color: '#6E7681',
          fontSize: 12,
          lineHeight: 1.5,
          margin: '0 0 24px 0',
        }}>
          You can change this at any time in Settings. No personal data, trading
          activity, or credentials are ever shared.
        </p>

        <div style={{
          display: 'flex',
          gap: 12,
          justifyContent: 'flex-end',
        }}>
          <button
            onClick={() => handleChoice(false)}
            style={{
              padding: '8px 20px',
              background: 'transparent',
              color: '#8B949E',
              border: '1px solid #30363D',
              borderRadius: 8,
              cursor: 'pointer',
              fontSize: 13,
              fontWeight: 500,
            }}
          >
            Decline
          </button>
          <button
            onClick={() => handleChoice(true)}
            style={{
              padding: '8px 20px',
              background: '#238636',
              color: '#fff',
              border: '1px solid #2EA043',
              borderRadius: 8,
              cursor: 'pointer',
              fontSize: 13,
              fontWeight: 500,
            }}
          >
            Accept
          </button>
        </div>
      </div>
    </div>
  );
}
