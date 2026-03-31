// Default FlexLayout model — 3-column Bloomberg-style trading desk
// Uses flexlayout-react JSON model format
// https://github.com/caplin/FlexLayout

export const DEFAULT_LAYOUT = {
  global: {
    tabEnableClose: true,
    tabEnableFloat: true,
    tabSetEnableMaximize: true,
    tabSetMinWidth: 200,
    tabSetMinHeight: 150,
    borderBarSize: 28,
    tabSetHeaderHeight: 32,
    tabSetTabStripHeight: 32,
  },
  layout: {
    type: 'row',
    weight: 100,
    children: [
      // Left column: bots + grid
      {
        type: 'row',
        weight: 25,
        children: [
          {
            type: 'tabset',
            weight: 60,
            children: [
              { type: 'tab', name: 'Bot Command Center', component: 'bot-command' },
              { type: 'tab', name: 'Positions', component: 'positions' },
            ],
          },
          {
            type: 'tabset',
            weight: 40,
            children: [
              { type: 'tab', name: 'Grid Compute', component: 'grid-compute' },
              { type: 'tab', name: 'Portfolio Brain', component: 'portfolio-brain' },
            ],
          },
        ],
      },
      // Center column: intelligence + chart
      {
        type: 'row',
        weight: 50,
        children: [
          {
            type: 'tabset',
            weight: 50,
            children: [
              { type: 'tab', name: 'Chart', component: 'chart' },
              { type: 'tab', name: 'Intelligence', component: 'intelligence' },
              { type: 'tab', name: 'Backtests', component: 'backtest' },
            ],
          },
          {
            type: 'tabset',
            weight: 50,
            children: [
              { type: 'tab', name: 'Regimes', component: 'regimes' },
              { type: 'tab', name: 'Meta Allocator', component: 'meta-allocator' },
            ],
          },
        ],
      },
      // Right column: discovery tools
      {
        type: 'row',
        weight: 25,
        children: [
          {
            type: 'tabset',
            weight: 60,
            children: [
              { type: 'tab', name: 'Scanner', component: 'scanner' },
              { type: 'tab', name: 'Watchlist', component: 'watchlist' },
              { type: 'tab', name: 'Strategies', component: 'strategies' },
            ],
          },
          {
            type: 'tabset',
            weight: 40,
            children: [
              { type: 'tab', name: 'Alerts', component: 'alerts' },
            ],
          },
        ],
      },
    ],
  },
};
