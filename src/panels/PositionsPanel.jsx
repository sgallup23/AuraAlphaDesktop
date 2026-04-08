import { useState, memo, useCallback } from 'react';
import usePositions from '../hooks/usePositions';
import DataTable from '../components/DataTable';
import { formatCurrency, formatPnl, pnlColor } from '../utils/formatters';

const POS_COLUMNS = [
  { key: 'symbol', label: 'Symbol', mono: true, render: (v) => <span className="font-medium text-aura-text">{v}</span> },
  { key: 'qty', label: 'Qty', mono: true },
  { key: 'avg_cost', label: 'Entry', mono: true, render: (v, row) => formatCurrency(v || row.avg_price || row.entryPrice || 0) },
  { key: 'market_value', label: 'Value', mono: true, render: (v, row) => formatCurrency(v || row.marketValue || 0) },
  {
    key: 'unrealized_pnl', label: 'P&L', mono: true,
    render: (v, row) => {
      const pnl = v || row.unrealizedPnl || 0;
      return <span className="font-medium" style={{ color: pnlColor(pnl) }}>{formatPnl(pnl)}</span>;
    },
  },
];

const ORDER_COLUMNS = [
  { key: 'symbol', label: 'Symbol', mono: true, render: (v) => <span className="font-medium text-aura-text">{v}</span> },
  { key: 'side', label: 'Side', render: (v) => <span className={v === 'BUY' ? 'text-aura-green' : 'text-aura-red'}>{v}</span> },
  { key: 'qty', label: 'Qty', mono: true, render: (v, row) => v || row.quantity },
  { key: 'status', label: 'Status', render: (v) => <span className="text-aura-muted">{v}</span> },
  { key: 'order_type', label: 'Type', render: (v, row) => <span className="text-aura-muted">{v || row.type}</span> },
];

const SYMBOL_ASC_SORT = { key: 'symbol', dir: 'asc' };

function PositionsPanel() {
  const { positions, orders, loading } = usePositions();
  const [tab, setTab] = useState('positions');

  const showPositions = useCallback(() => setTab('positions'), []);
  const showOrders = useCallback(() => setTab('orders'), []);

  if (loading) return <div className="p-4 text-aura-muted animate-pulse">Loading positions...</div>;

  return (
    <div className="space-y-2">
      <div className="flex gap-2">
        <button onClick={showPositions} className={`text-xs px-3 py-1 rounded ${tab === 'positions' ? 'bg-aura-blue text-white' : 'text-aura-muted hover:text-aura-text'}`}>
          Positions ({positions.length})
        </button>
        <button onClick={showOrders} className={`text-xs px-3 py-1 rounded ${tab === 'orders' ? 'bg-aura-blue text-white' : 'text-aura-muted hover:text-aura-text'}`}>
          Orders ({orders.length})
        </button>
      </div>

      {tab === 'positions' ? (
        <DataTable
          columns={POS_COLUMNS}
          data={positions}
          defaultSort={SYMBOL_ASC_SORT}
          emptyText="No open positions"
        />
      ) : (
        <DataTable
          columns={ORDER_COLUMNS}
          data={orders}
          defaultSort={SYMBOL_ASC_SORT}
          emptyText="No orders"
        />
      )}
    </div>
  );
}

export default memo(PositionsPanel);
