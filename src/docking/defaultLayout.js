// Default docking layout — 3-column split
export const DEFAULT_LAYOUT = {
  dpiScale: 1,
  mode: 'horizontal',
  children: [
    {
      mode: 'vertical',
      size: 300,
      children: [
        {
          tabs: [
            { id: 'bot-command', title: 'Bot Command Center', group: 'main' },
          ],
        },
        {
          tabs: [
            { id: 'positions', title: 'Positions', group: 'main' },
          ],
          size: 300,
        },
      ],
    },
    {
      tabs: [
        { id: 'chart', title: 'Chart', group: 'main' },
      ],
      size: 600,
    },
    {
      mode: 'vertical',
      size: 300,
      children: [
        {
          tabs: [
            { id: 'watchlist', title: 'Watchlist', group: 'main' },
            { id: 'scanner', title: 'Scanner', group: 'main' },
          ],
        },
        {
          tabs: [
            { id: 'alerts', title: 'Alerts', group: 'main' },
            { id: 'strategies', title: 'Strategies', group: 'main' },
          ],
          size: 300,
        },
      ],
    },
  ],
};
