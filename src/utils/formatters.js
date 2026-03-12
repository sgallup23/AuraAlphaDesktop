export function formatCurrency(val, decimals = 2) {
  if (val == null || isNaN(val)) return '$0.00';
  return '$' + Number(val).toLocaleString('en-US', { minimumFractionDigits: decimals, maximumFractionDigits: decimals });
}

export function formatPnl(val) {
  if (val == null || isNaN(val)) return '$0.00';
  const prefix = val >= 0 ? '+' : '';
  return prefix + formatCurrency(val);
}

export function formatPercent(val, decimals = 1) {
  if (val == null || isNaN(val)) return '0.0%';
  const prefix = val >= 0 ? '+' : '';
  return prefix + Number(val).toFixed(decimals) + '%';
}

export function formatVolume(val) {
  if (val == null || isNaN(val)) return '0';
  if (val >= 1e9) return (val / 1e9).toFixed(1) + 'B';
  if (val >= 1e6) return (val / 1e6).toFixed(1) + 'M';
  if (val >= 1e3) return (val / 1e3).toFixed(1) + 'K';
  return val.toLocaleString();
}

export function timeAgo(ts) {
  if (!ts) return '';
  const now = Date.now();
  const t = ts > 1e12 ? ts : ts * 1000;
  const diff = now - t;
  if (diff < 60000) return Math.floor(diff / 1000) + 's ago';
  if (diff < 3600000) return Math.floor(diff / 60000) + 'm ago';
  if (diff < 86400000) return Math.floor(diff / 3600000) + 'h ago';
  return Math.floor(diff / 86400000) + 'd ago';
}

export function pnlColor(val) {
  if (val > 0) return '#3FB950';
  if (val < 0) return '#F85149';
  return '#8B949E';
}
