// Panel registry — maps panel IDs to components
// Components are lazy-loaded to keep initial bundle small
import { lazy } from 'react';

const BotCommandPanel = lazy(() => import('../panels/BotCommandPanel'));
const PositionsPanel = lazy(() => import('../panels/PositionsPanel'));
const WatchlistPanel = lazy(() => import('../panels/WatchlistPanel'));
const ChartPanel = lazy(() => import('../panels/ChartPanel'));
const ScannerPanel = lazy(() => import('../panels/ScannerPanel'));
const AlertsPanel = lazy(() => import('../panels/AlertsPanel'));
const StrategyManagerPanel = lazy(() => import('../panels/StrategyManagerPanel'));
const BrokerSetupPanel = lazy(() => import('../panels/BrokerSetupPanel'));
const BotManagerPanel = lazy(() => import('../panels/BotManagerPanel'));

export const PANELS = {
  'bot-command':    { id: 'bot-command',    title: 'Bot Command Center', component: BotCommandPanel, closable: true, group: 'main' },
  'positions':      { id: 'positions',      title: 'Positions',          component: PositionsPanel,  closable: true, group: 'main' },
  'watchlist':      { id: 'watchlist',      title: 'Watchlist',          component: WatchlistPanel,  closable: true, group: 'main' },
  'chart':          { id: 'chart',          title: 'Chart',              component: ChartPanel,      closable: true, group: 'main' },
  'scanner':        { id: 'scanner',        title: 'Scanner',            component: ScannerPanel,    closable: true, group: 'main' },
  'alerts':         { id: 'alerts',         title: 'Alerts',             component: AlertsPanel,     closable: true, group: 'main' },
  'strategies':     { id: 'strategies',     title: 'Strategies',         component: StrategyManagerPanel, closable: true, group: 'main' },
  'broker-setup':   { id: 'broker-setup',   title: 'Broker Setup',       component: BrokerSetupPanel, closable: true, group: 'settings' },
  'bot-manager':    { id: 'bot-manager',    title: 'Bot Manager',        component: BotManagerPanel,  closable: true, group: 'settings' },
};

export function getPanelComponent(panelId) {
  return PANELS[panelId]?.component || null;
}

export function getPanelTitle(panelId) {
  return PANELS[panelId]?.title || panelId;
}
