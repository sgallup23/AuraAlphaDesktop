/**
 * ForceUpdateModal
 *
 * Full-screen blocking overlay shown when the running app version is below
 * the server-enforced minimum_version.  Cannot be dismissed — the user must
 * download and install the new release.
 *
 * Props:
 *   currentVersion  — semver string from check_for_update (e.g. "4.1.0")
 *   minimumVersion  — semver string from check_for_update (e.g. "5.0.0")
 *   downloadUrl     — update_endpoint URL from check_for_update
 */

/**
 * Open a URL in the system default browser.
 * Tauri v2 opener plugin is available on `window.__TAURI__.opener` when the
 * plugin is registered in lib.rs (tauri_plugin_opener::init()).
 * Falls back to window.open for dev/web context.
 */
async function openUrl(url) {
  try {
    // Tauri v2 opener plugin surface
    if (window.__TAURI__?.opener?.openUrl) {
      await window.__TAURI__.opener.openUrl(url);
      return;
    }
    // Tauri v2 internals direct invoke
    if (window.__TAURI_INTERNALS__) {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('plugin:opener|open_url', { url });
      return;
    }
  } catch {}
  // Fallback: standard browser open
  window.open(url, '_blank', 'noopener,noreferrer');
}

export default function ForceUpdateModal({ currentVersion, minimumVersion, downloadUrl }) {
  const handleUpdate = async () => {
    if (!downloadUrl) return;
    await openUrl(downloadUrl);
  };

  return (
    <div style={styles.overlay}>
      <div style={styles.card}>
        {/* Logo */}
        <div style={styles.logo}>
          <span style={styles.logoText}>A</span>
        </div>

        {/* Brand */}
        <div style={styles.brand}>Aura Alpha</div>

        {/* Divider */}
        <div style={styles.divider} />

        {/* Heading */}
        <div style={styles.badge}>Update Required</div>
        <h1 style={styles.heading}>Your app is out of date</h1>

        {/* Version info */}
        <p style={styles.body}>
          Version{' '}
          <span style={styles.versionBad}>{currentVersion}</span>
          {' '}is below the minimum required{' '}
          <span style={styles.versionGood}>{minimumVersion}</span>.
        </p>
        <p style={styles.subtext}>
          This version is no longer supported. Please update to continue
          using Aura Alpha Desktop.
        </p>

        {/* CTA */}
        <button style={styles.button} onClick={handleUpdate}>
          Update Now
        </button>

        {/* Footer note */}
        <p style={styles.footnote}>
          After installing the update, relaunch the application.
        </p>
      </div>
    </div>
  );
}

// ─── Inline styles ────────────────────────────────────────────────────────────

const styles = {
  overlay: {
    position: 'fixed',
    inset: 0,
    zIndex: 9999,
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    background: 'rgba(0, 0, 0, 0.95)',
    fontFamily: "'Plus Jakarta Sans', system-ui, sans-serif",
  },

  card: {
    display: 'flex',
    flexDirection: 'column',
    alignItems: 'center',
    textAlign: 'center',
    background: '#161B22',
    border: '1px solid #30363D',
    borderRadius: 16,
    padding: '48px 56px',
    maxWidth: 440,
    width: '90%',
    boxShadow: '0 24px 64px rgba(0,0,0,0.6)',
  },

  logo: {
    display: 'inline-flex',
    alignItems: 'center',
    justifyContent: 'center',
    width: 56,
    height: 56,
    borderRadius: 16,
    background: 'linear-gradient(135deg, #58A6FF, #BC8CFF, #3FB950)',
    marginBottom: 12,
  },
  logoText: {
    color: '#fff',
    fontSize: 28,
    fontWeight: 700,
    lineHeight: 1,
  },

  brand: {
    fontSize: 20,
    fontWeight: 700,
    color: '#E6EDF3',
    marginBottom: 20,
    letterSpacing: '-0.01em',
  },

  divider: {
    width: '100%',
    height: 1,
    background: '#21262D',
    marginBottom: 24,
  },

  badge: {
    display: 'inline-block',
    padding: '4px 12px',
    background: 'rgba(248, 81, 73, 0.15)',
    border: '1px solid rgba(248, 81, 73, 0.35)',
    borderRadius: 20,
    fontSize: 12,
    fontWeight: 600,
    color: '#F85149',
    letterSpacing: '0.04em',
    textTransform: 'uppercase',
    marginBottom: 16,
  },

  heading: {
    fontSize: 22,
    fontWeight: 700,
    color: '#E6EDF3',
    margin: '0 0 16px',
    letterSpacing: '-0.02em',
  },

  body: {
    fontSize: 15,
    color: '#C9D1D9',
    lineHeight: 1.6,
    margin: '0 0 8px',
  },

  versionBad: {
    fontFamily: "'SF Mono', 'Fira Code', monospace",
    fontSize: 14,
    fontWeight: 600,
    color: '#F85149',
    background: 'rgba(248,81,73,0.1)',
    padding: '1px 6px',
    borderRadius: 4,
  },

  versionGood: {
    fontFamily: "'SF Mono', 'Fira Code', monospace",
    fontSize: 14,
    fontWeight: 600,
    color: '#3FB950',
    background: 'rgba(63,185,80,0.1)',
    padding: '1px 6px',
    borderRadius: 4,
  },

  subtext: {
    fontSize: 13,
    color: '#8B949E',
    lineHeight: 1.6,
    margin: '0 0 32px',
  },

  button: {
    display: 'block',
    width: '100%',
    padding: '14px 0',
    background: 'linear-gradient(135deg, #2EA043, #3FB950)',
    border: 'none',
    borderRadius: 10,
    color: '#fff',
    fontSize: 15,
    fontWeight: 700,
    cursor: 'pointer',
    letterSpacing: '0.01em',
    transition: 'opacity 0.15s, transform 0.1s',
    marginBottom: 16,
  },

  footnote: {
    fontSize: 12,
    color: '#484F58',
    margin: 0,
  },
};
