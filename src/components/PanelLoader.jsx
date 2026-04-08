/**
 * PanelLoader — Skeleton placeholder shown while a lazy-loaded panel chunk is fetched.
 * Used as the Suspense fallback for every docking panel.
 */
export default function PanelLoader({ title }) {
  return (
    <div
      className="flex flex-col items-center justify-center gap-3 h-full w-full select-none"
      style={{ minHeight: 120, background: 'var(--aura-bg, #0D1117)' }}
    >
      {/* Spinner ring */}
      <div
        style={{
          width: 28,
          height: 28,
          border: '3px solid rgba(88,166,255,0.15)',
          borderTop: '3px solid #58A6FF',
          borderRadius: '50%',
          animation: 'panel-spin 0.8s linear infinite',
        }}
      />
      {title && (
        <span style={{ color: '#8B949E', fontSize: 12, fontFamily: 'system-ui, sans-serif' }}>
          Loading {title}...
        </span>
      )}
      {/* Skeleton bars */}
      <div style={{ display: 'flex', flexDirection: 'column', gap: 6, width: '60%', maxWidth: 220, marginTop: 4 }}>
        <div style={{ height: 8, borderRadius: 4, background: 'rgba(88,166,255,0.08)', animation: 'panel-pulse 1.5s ease-in-out infinite' }} />
        <div style={{ height: 8, borderRadius: 4, background: 'rgba(88,166,255,0.06)', animation: 'panel-pulse 1.5s ease-in-out 0.2s infinite', width: '75%' }} />
        <div style={{ height: 8, borderRadius: 4, background: 'rgba(88,166,255,0.04)', animation: 'panel-pulse 1.5s ease-in-out 0.4s infinite', width: '50%' }} />
      </div>
      <style>{`
        @keyframes panel-spin {
          to { transform: rotate(360deg); }
        }
        @keyframes panel-pulse {
          0%, 100% { opacity: 0.4; }
          50% { opacity: 1; }
        }
      `}</style>
    </div>
  );
}
