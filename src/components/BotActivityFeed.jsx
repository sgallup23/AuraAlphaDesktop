import { useState, useEffect, useRef } from 'react'
import { api } from '../utils/api'

const COLORS = {
  bg: '#0B0F14',
  surface: '#0F1620',
  border: '#1F2A37',
  text: '#E5E7DB',
  muted: '#90A3AF',
  green: '#22C55E',
  red: '#EF4444',
  blue: '#60A5FA',
  purple: '#A78BFA',
}

// Assign a stable color to each bot name dynamically
const PALETTE = [COLORS.green, COLORS.blue, COLORS.purple, '#ec4899', '#f59e0b', '#10b981']
const botColorCache = {}
function getBotColor(name) {
  if (!name) return COLORS.muted
  const key = String(name).toUpperCase()
  if (!botColorCache[key]) {
    const idx = Object.keys(botColorCache).length % PALETTE.length
    botColorCache[key] = PALETTE[idx]
  }
  return botColorCache[key]
}

function timeAgo(date) {
  const secs = Math.floor((Date.now() - date.getTime()) / 1000)
  if (secs < 60) return `${secs}s ago`
  const mins = Math.floor(secs / 60)
  if (mins < 60) return `${mins}m ago`
  const hrs = Math.floor(mins / 60)
  return `${hrs}h ago`
}

export default function BotActivityFeed() {
  const [trades, setTrades] = useState([])
  const [loading, setLoading] = useState(true)
  const [activeFilter, setActiveFilter] = useState('ALL')
  const [tick, setTick] = useState(0)
  const intervalRef = useRef(null)

  const fetchTrades = async () => {
    try {
      const data = await api('/telemetry/latest')
      if (!data) throw new Error('API error')
      let arr = Array.isArray(data.trades) ? data.trades : Array.isArray(data) ? data : []
      // Collect from per-bot payloads if no top-level trades
      if (arr.length === 0 && data.bots && typeof data.bots === 'object') {
        Object.entries(data.bots).forEach(([botKey, b]) => {
          const botTrades = b?.payload?.trades || []
          if (Array.isArray(botTrades)) {
            arr = arr.concat(botTrades.map(t => ({
              ...t,
              bot: String(b?.payload?.bot_name || t?.bot || botKey || '').toUpperCase(),
              symbol: String(t?.symbol || ''),
              timestamp: t.timestamp
                ? new Date(typeof t.timestamp === 'number' && t.timestamp < 1e12 ? t.timestamp * 1000 : t.timestamp)
                : new Date(),
            })))
          }
        })
      }
      setTrades(arr)
    } catch {
      setTrades([])
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    fetchTrades()
    intervalRef.current = setInterval(() => {
      setTick((t) => t + 1)
      fetchTrades()
    }, 10000)
    return () => clearInterval(intervalRef.current)
  }, [])

  // tick forces re-render of timeAgo labels even without new data
  useEffect(() => {}, [tick])

  const safeTrades = Array.isArray(trades) ? trades : []

  // Build filter list dynamically from actual bot names in trades
  const botNames = [...new Set(safeTrades.map(t => String(t.bot || '').toUpperCase()).filter(Boolean))]
  const filters = ['ALL', ...botNames.sort()]

  const filtered = activeFilter === 'ALL'
    ? safeTrades
    : safeTrades.filter((t) => String(t.bot || '').toUpperCase() === activeFilter)

  const styles = {
    card: {
      background: COLORS.surface,
      border: `1px solid ${COLORS.border}`,
      borderRadius: 14,
      padding: '20px 0',
      fontFamily: "'JetBrains Mono', 'Fira Code', 'Courier New', monospace",
      color: COLORS.text,
      maxWidth: 480,
      width: '100%',
    },
    header: {
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'space-between',
      padding: '0 20px',
      marginBottom: 16,
    },
    titleRow: {
      display: 'flex',
      alignItems: 'center',
      gap: 8,
    },
    title: {
      fontSize: 14,
      fontWeight: 700,
      letterSpacing: '0.04em',
    },
    liveDot: {
      width: 7,
      height: 7,
      borderRadius: '50%',
      background: COLORS.green,
      animation: 'feed-pulse 1.5s ease-in-out infinite',
    },
    liveLabel: {
      fontSize: 10,
      color: COLORS.green,
      letterSpacing: '0.08em',
    },
    filterRow: {
      display: 'flex',
      gap: 8,
      padding: '0 20px',
      marginBottom: 12,
      flexWrap: 'wrap',
    },
    filterPill: (active, bot) => {
      const color = bot === 'ALL' ? COLORS.blue : getBotColor(bot)
      return {
        background: active ? `${color}22` : 'transparent',
        border: `1px solid ${active ? color : COLORS.border}`,
        borderRadius: 18,
        padding: '3px 12px',
        fontSize: 11,
        fontFamily: 'inherit',
        color: active ? color : COLORS.muted,
        cursor: 'pointer',
        transition: 'all 0.25s',
        letterSpacing: '0.05em',
        fontWeight: active ? 700 : 400,
      }
    },
    scrollList: {
      maxHeight: 340,
      overflowY: 'auto',
      scrollbarWidth: 'thin',
      scrollbarColor: `${COLORS.border} transparent`,
    },
    tradeRow: {
      display: 'flex',
      alignItems: 'center',
      gap: 12,
      padding: '9px 20px',
      borderTop: `1px solid ${COLORS.border}`,
      transition: 'background 0.15s',
    },
    botDot: (bot) => ({
      width: 8,
      height: 8,
      borderRadius: '50%',
      background: getBotColor(bot),
      flexShrink: 0,
    }),
    botName: (bot) => ({
      fontSize: 11,
      fontWeight: 700,
      color: getBotColor(bot),
      width: 52,
      flexShrink: 0,
      overflow: 'hidden',
      textOverflow: 'ellipsis',
      whiteSpace: 'nowrap',
    }),
    sideBadge: (side) => ({
      fontSize: 10,
      fontWeight: 700,
      letterSpacing: '0.06em',
      padding: '2px 7px',
      borderRadius: 6,
      background: side === 'BUY' ? `${COLORS.green}22` : `${COLORS.red}22`,
      border: `1px solid ${side === 'BUY' ? COLORS.green : COLORS.red}55`,
      color: side === 'BUY' ? COLORS.green : COLORS.red,
      flexShrink: 0,
    }),
    symbol: {
      fontSize: 13,
      fontWeight: 700,
      color: COLORS.text,
      flex: 1,
    },
    price: {
      fontSize: 13,
      color: COLORS.muted,
      flexShrink: 0,
    },
    timeAgo: {
      fontSize: 10,
      color: COLORS.muted,
      width: 44,
      textAlign: 'right',
      flexShrink: 0,
    },
    emptyState: {
      padding: '32px 20px',
      textAlign: 'center',
      color: COLORS.muted,
      fontSize: 13,
    },
    skeletonRow: {
      padding: '10px 20px',
      borderTop: `1px solid ${COLORS.border}`,
      display: 'flex',
      gap: 12,
      alignItems: 'center',
    },
    skeletonBlock: (w, h = 14) => ({
      width: w,
      height: h,
      borderRadius: 6,
      background: `linear-gradient(90deg, ${COLORS.border} 25%, #1E2D3D 50%, ${COLORS.border} 75%)`,
      backgroundSize: '200% 100%',
      animation: 'feed-shimmer 1.4s infinite',
      flexShrink: 0,
    }),
    footer: {
      padding: '10px 20px 0',
      borderTop: `1px solid ${COLORS.border}`,
      marginTop: 4,
      display: 'flex',
      justifyContent: 'space-between',
      alignItems: 'center',
      fontSize: 10,
      color: COLORS.muted,
    },
  }

  return (
    <div style={styles.card}>
      <style>{`
        @keyframes feed-pulse {
          0%, 100% { opacity: 1; }
          50% { opacity: 0.3; }
        }
        @keyframes feed-shimmer {
          0% { background-position: -200% 0; }
          100% { background-position: 200% 0; }
        }
        .trade-row:hover {
          background: rgba(255,255,255,0.02) !important;
        }
      `}</style>

      <div style={styles.header}>
        <div style={styles.titleRow}>
          <span style={{ fontSize: 16 }}>⚡</span>
          <span style={styles.title}>BOT ACTIVITY FEED</span>
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <div style={styles.liveDot} />
          <span style={styles.liveLabel}>LIVE</span>
        </div>
      </div>

      {filters.length > 1 && (
        <div style={styles.filterRow}>
          {filters.map((f) => (
            <button
              key={f}
              style={styles.filterPill(activeFilter === f, f)}
              onClick={() => setActiveFilter(f)}
            >
              {f}
            </button>
          ))}
        </div>
      )}

      <div style={styles.scrollList}>
        {loading ? (
          Array.from({ length: 8 }).map((_, i) => (
            <div key={i} style={styles.skeletonRow}>
              <div style={styles.skeletonBlock(8, 8)} />
              <div style={styles.skeletonBlock(44)} />
              <div style={styles.skeletonBlock(36, 20)} />
              <div style={styles.skeletonBlock(50)} />
              <div style={{ flex: 1 }} />
              <div style={styles.skeletonBlock(60)} />
              <div style={styles.skeletonBlock(36, 10)} />
            </div>
          ))
        ) : filtered.length === 0 ? (
          <div style={styles.emptyState}>
            {activeFilter === 'ALL'
              ? 'No trades yet today'
              : `No trades for ${activeFilter}`}
          </div>
        ) : (
          filtered.map((trade) => {
            const price = Number(trade.price ?? trade.fill_price ?? trade.avg_fill ?? trade.limit_price) || 0
            const ts = trade.timestamp instanceof Date ? trade.timestamp
              : typeof trade.timestamp === 'number' ? new Date(trade.timestamp < 1e12 ? trade.timestamp * 1000 : trade.timestamp)
              : new Date()
            const side = (trade.side || trade.action || '').toUpperCase()
            const botName = String(trade.bot || '—').toUpperCase()
            return (
              <div key={trade.id || `${trade.symbol}-${trade.bot}-${ts.getTime()}`} className="trade-row" style={styles.tradeRow}>
                <div style={styles.botDot(botName)} />
                <span style={styles.botName(botName)}>{botName}</span>
                <span style={styles.sideBadge(side)}>{side}</span>
                <span style={styles.symbol}>{String(trade.symbol || '—')}</span>
                <span style={styles.price}>
                  {price > 0 ? `$${price >= 1000
                    ? price.toLocaleString('en-US', { minimumFractionDigits: 0 })
                    : price.toFixed(2)}` : '—'}
                </span>
                <span style={styles.timeAgo}>{timeAgo(ts)}</span>
              </div>
            )
          })
        )}
      </div>

      <div style={styles.footer}>
        <span>{filtered.length} trades shown</span>
        <span>Auto-refreshes every 10s</span>
      </div>
    </div>
  )
}
