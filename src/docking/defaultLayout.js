// Default docking layout — 3-column split
// rc-dock LayoutData requires a dockbox wrapper (BoxData directly at top level is invalid)
//
// Layout philosophy: Lead with what's unique.
//   Left  — Live bots + research activity (what's happening right now)
//   Center — Intelligence engine first, chart second (the jaw-drop moment)
//   Right  — Scanner/strategies + alerts (power tools discoverable on the right)
export const DEFAULT_LAYOUT = {
  dockbox: {
    mode: 'horizontal',
    children: [
      // Left column: live bots on top, grid compute on bottom
      {
        mode: 'vertical',
        size: 290,
        children: [
          {
            tabs: [
              { id: 'bot-command', title: 'Bot Command Center', group: 'main' },
              { id: 'positions', title: 'Positions', group: 'main' },
            ],
          },
          {
            tabs: [
              { id: 'grid-compute', title: 'Grid Compute', group: 'main' },
              { id: 'portfolio-brain', title: 'Portfolio Brain', group: 'main' },
            ],
            size: 280,
          },
        ],
      },
      // Center column: intelligence dashboard first (the differentiator), chart second
      {
        mode: 'vertical',
        size: 620,
        children: [
          {
            tabs: [
              { id: 'intelligence', title: 'Intelligence', group: 'main' },
              { id: 'regimes', title: 'Regimes', group: 'main' },
              { id: 'meta-allocator', title: 'Meta Allocator', group: 'main' },
            ],
            size: 320,
          },
          {
            tabs: [
              { id: 'chart', title: 'Chart', group: 'main' },
              { id: 'backtest', title: 'Backtests', group: 'main' },
            ],
          },
        ],
      },
      // Right column: discovery tools
      {
        mode: 'vertical',
        size: 300,
        children: [
          {
            tabs: [
              { id: 'scanner', title: 'Scanner', group: 'main' },
              { id: 'watchlist', title: 'Watchlist', group: 'main' },
              { id: 'strategies', title: 'Strategies', group: 'main' },
            ],
          },
          {
            tabs: [
              { id: 'alerts', title: 'Alerts', group: 'main' },
            ],
            size: 260,
          },
        ],
      },
    ],
  },
};
