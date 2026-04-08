import { useState, useMemo, useCallback, memo } from 'react';

/**
 * DataTable — Shared sortable table component.
 *
 * Props:
 *   columns  — [{ key, label, render?, align?, mono? }]
 *   data     — array of row objects
 *   defaultSort — { key, dir: 'asc'|'desc' }
 *   emptyText — string shown when no data
 *   onRowClick — optional (row) => void
 */

const DataTableRow = memo(function DataTableRow({ row, columns, onRowClick, clickable }) {
  const handleClick = useCallback(() => onRowClick?.(row), [onRowClick, row]);
  return (
    <tr
      className={`border-b border-aura-border/50 hover:bg-aura-surface2/50 ${clickable ? 'cursor-pointer' : ''}`}
      onClick={clickable ? handleClick : undefined}
    >
      {columns.map(col => (
        <td
          key={col.key}
          className={`px-2 py-1.5 ${col.mono ? 'font-mono' : ''}`}
          style={col.align ? { textAlign: col.align } : undefined}
        >
          {col.render ? col.render(row[col.key], row) : row[col.key]}
        </td>
      ))}
    </tr>
  );
});

function DataTable({ columns, data, defaultSort, emptyText = 'No data', onRowClick }) {
  const [sortKey, setSortKey] = useState(defaultSort?.key || columns[0]?.key);
  const [sortDir, setSortDir] = useState(defaultSort?.dir || 'asc');

  const toggleSort = useCallback((key) => {
    setSortKey(prev => {
      if (prev === key) {
        setSortDir(d => d === 'asc' ? 'desc' : 'asc');
        return prev;
      }
      setSortDir('asc');
      return key;
    });
  }, []);

  const sorted = useMemo(() => {
    return [...data].sort((a, b) => {
      const av = a[sortKey], bv = b[sortKey];
      const cmp = typeof av === 'number' ? av - bv : String(av || '').localeCompare(String(bv || ''));
      return sortDir === 'asc' ? cmp : -cmp;
    });
  }, [data, sortKey, sortDir]);

  const clickable = !!onRowClick;

  return (
    <div className="overflow-auto">
      <table className="w-full text-xs">
        <thead>
          <tr className="border-b border-aura-border">
            {columns.map(col => (
              <th
                key={col.key}
                className="text-left text-xs text-aura-muted font-medium px-2 py-1.5 cursor-pointer hover:text-aura-text select-none"
                onClick={() => toggleSort(col.key)}
              >
                {col.label} {sortKey === col.key ? (sortDir === 'asc' ? '\u25B2' : '\u25BC') : ''}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {sorted.map((row, i) => (
            <DataTableRow
              key={row.id || row.symbol || i}
              row={row}
              columns={columns}
              onRowClick={onRowClick}
              clickable={clickable}
            />
          ))}
          {sorted.length === 0 && (
            <tr><td colSpan={columns.length} className="text-center py-6 text-aura-muted">{emptyText}</td></tr>
          )}
        </tbody>
      </table>
    </div>
  );
}

export default memo(DataTable);
