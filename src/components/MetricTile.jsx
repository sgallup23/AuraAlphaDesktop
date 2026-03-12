/**
 * MetricTile — Glass-panel metric display.
 *
 * Props:
 *   label — string (top label)
 *   value — string|number (main value)
 *   color — optional hex color for value
 *   size  — 'sm' | 'md' (default) | 'lg'
 *   mono  — boolean (monospace value)
 */
const SIZE_CLASSES = {
  sm: { label: 'text-[10px]', value: 'text-sm' },
  md: { label: 'text-xs', value: 'text-lg' },
  lg: { label: 'text-sm', value: 'text-2xl' },
};

export default function MetricTile({ label, value, color, size = 'md', mono = true }) {
  const s = SIZE_CLASSES[size] || SIZE_CLASSES.md;

  return (
    <div className="metric-tile">
      <div className={`${s.label} text-aura-muted mb-1`}>{label}</div>
      <div
        className={`${s.value} font-bold ${mono ? 'font-mono' : ''} ${color ? '' : 'text-aura-text'}`}
        style={color ? { color } : undefined}
      >
        {value}
      </div>
    </div>
  );
}
