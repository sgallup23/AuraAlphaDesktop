// Default docking layout — 3-column split
// rc-dock requires: dockbox wrapper, each tab panel needs tabs array with id+title
export const DEFAULT_LAYOUT = {
  dockbox: {
    mode: 'horizontal',
    children: [
      {
        mode: 'vertical',
        size: 300,
        children: [
          { tabs: [{ id: 'bot-command', title: 'Bot Command Center' }] },
          { tabs: [{ id: 'positions', title: 'Positions' }, { id: 'grid-compute', title: 'Grid Compute' }] },
        ],
      },
      {
        mode: 'vertical',
        size: 600,
        children: [
          { tabs: [{ id: 'chart', title: 'Chart' }, { id: 'intelligence', title: 'Intelligence' }, { id: 'backtest', title: 'Backtests' }] },
          { tabs: [{ id: 'portfolio-brain', title: 'Portfolio Brain' }, { id: 'regimes', title: 'Regimes' }, { id: 'meta-allocator', title: 'Meta Allocator' }] },
        ],
      },
      {
        mode: 'vertical',
        size: 300,
        children: [
          { tabs: [{ id: 'scanner', title: 'Scanner' }, { id: 'watchlist', title: 'Watchlist' }, { id: 'strategies', title: 'Strategies' }] },
          { tabs: [{ id: 'alerts', title: 'Alerts' }] },
        ],
      },
    ],
  },
};
