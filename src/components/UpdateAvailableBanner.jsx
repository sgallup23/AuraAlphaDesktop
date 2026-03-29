import { useState } from 'react';

/**
 * UpdateAvailableBanner
 *
 * Non-blocking slim banner shown when a newer app version exists on the server
 * but is not yet required (update_required === false).
 *
 * Props:
 *   latestVersion   — the newest available version (e.g. "5.1.0")
 *   currentVersion  — the version currently running (e.g. "5.0.2")
 *   downloadUrl     — URL to open for the download / release notes
 *   onDismiss       — optional callback when the user dismisses the banner
 */

/**
 * Open a URL in the system default browser via Tauri v2 opener plugin,
 * with a fallback to window.open for dev/web context.
 */
async function openUrl(url) {
  try {
    if (window.__TAURI__?.opener?.openUrl) {
      await window.__TAURI__.opener.openUrl(url);
      return;
    }
    if (window.__TAURI_INTERNALS__) {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('plugin:opener|open_url', { url });
      return;
    }
  } catch {}
  window.open(url, '_blank', 'noopener,noreferrer');
}

export default function UpdateAvailableBanner({
  latestVersion,
  currentVersion,
  downloadUrl,
  onDismiss,
}) {
  const [dismissed, setDismissed] = useState(false);
  const [hoverBtn, setHoverBtn] = useState(false);
  const [hoverClose, setHoverClose] = useState(false);

  if (dismissed) return null;

  const handleUpdate = async () => {
    if (!downloadUrl) return;
    await openUrl(downloadUrl);
  };

  const handleDismiss = () => {
    setDismissed(true);
    onDismiss?.();
  };

  return (
    <div style={styles.banner}>
      {/* Left: icon + message */}
      <div style={styles.left}>
        <span style={styles.icon}>&#x2B06;</span>
        <span style={styles.message}>
          Version{' '}
          <strong style={styles.version}>{latestVersion}</strong>
          {' '}is available
          {currentVersion ? (
            <span style={styles.current}> &mdash; you&apos;re on {currentVersion}</span>
          ) : null}
        </span>
      </div>

      {/* Right: action + dismiss */}
      <div style={styles.right}>
        <button
          onClick={handleUpdate}
          style={{ ...styles.updateBtn, ...(hoverBtn ? styles.updateBtnHover : {}) }}
          onMouseEnter={() => setHoverBtn(true)}
          onMouseLeave={() => setHoverBtn(false)}
        >
          Update Now
        </button>
        <button
          onClick={handleDismiss}
          style={{ ...styles.closeBtn, ...(hoverClose ? styles.closeBtnHover : {}) }}
          onMouseEnter={() => setHoverClose(true)}
          onMouseLeave={() => setHoverClose(false)}
          aria-label="Dismiss update banner"
        >
          &#x2715;
        </button>
      </div>
    </div>
  );
}

// ─── Inline styles ────────────────────────────────────────────────────────────

const styles = {
  banner: {
    position: 'fixed',
    top: 0,
    left: 0,
    right: 0,
    zIndex: 1000,
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'space-between',
    padding: '8px 16px',
    background: 'linear-gradient(90deg, #0D2818 0%, #0F2A1A 100%)',
    borderBottom: '1px solid #2EA04344',
    fontFamily: "'Plus Jakarta Sans', system-ui, sans-serif",
    fontSize: 13,
    gap: 12,
  },

  left: {
    display: 'flex',
    alignItems: 'center',
    gap: 8,
    minWidth: 0,
    overflow: 'hidden',
  },

  icon: {
    fontSize: 14,
    color: '#3FB950',
    flexShrink: 0,
  },

  message: {
    color: '#C9D1D9',
    whiteSpace: 'nowrap',
    overflow: 'hidden',
    textOverflow: 'ellipsis',
  },

  version: {
    color: '#3FB950',
    fontWeight: 600,
  },

  current: {
    color: '#8B949E',
    fontWeight: 400,
  },

  right: {
    display: 'flex',
    alignItems: 'center',
    gap: 8,
    flexShrink: 0,
  },

  updateBtn: {
    padding: '4px 14px',
    background: '#2EA043',
    border: 'none',
    borderRadius: 6,
    color: '#fff',
    fontSize: 12,
    fontWeight: 600,
    cursor: 'pointer',
    transition: 'background 0.15s',
    letterSpacing: '0.02em',
  },

  updateBtnHover: {
    background: '#3FB950',
  },

  closeBtn: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    width: 22,
    height: 22,
    padding: 0,
    background: 'transparent',
    border: 'none',
    borderRadius: 4,
    color: '#8B949E',
    fontSize: 13,
    cursor: 'pointer',
    lineHeight: 1,
    transition: 'color 0.15s, background 0.15s',
  },

  closeBtnHover: {
    color: '#E6EDF3',
    background: '#21262D',
  },
};
