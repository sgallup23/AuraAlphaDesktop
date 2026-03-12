/**
 * StatusDot — Colored indicator dot.
 *
 * Props:
 *   color — 'green' | 'red' | 'amber' | 'blue' | string (hex)
 *   size  — 'sm' (6px) | 'md' (8px, default) | 'lg' (10px)
 *   pulse — boolean, adds pulse animation
 */
const COLOR_MAP = {
  green: 'bg-aura-green',
  red: 'bg-aura-red',
  amber: 'bg-aura-amber',
  blue: 'bg-aura-blue',
  muted: 'bg-aura-muted',
};

const SIZE_MAP = {
  sm: 'w-1.5 h-1.5',
  md: 'w-2 h-2',
  lg: 'w-2.5 h-2.5',
};

export default function StatusDot({ color = 'green', size = 'md', pulse = false, style }) {
  const colorClass = COLOR_MAP[color];
  const sizeClass = SIZE_MAP[size] || SIZE_MAP.md;

  if (colorClass) {
    return (
      <span
        className={`rounded-full inline-block flex-shrink-0 ${sizeClass} ${colorClass} ${pulse ? 'animate-pulse' : ''}`}
        style={style}
      />
    );
  }

  // Custom hex color
  return (
    <span
      className={`rounded-full inline-block flex-shrink-0 ${sizeClass} ${pulse ? 'animate-pulse' : ''}`}
      style={{ background: color, ...style }}
    />
  );
}
