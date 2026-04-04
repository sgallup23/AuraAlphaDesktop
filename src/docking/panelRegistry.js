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
const BacktestPanel = lazy(() => import('../panels/BacktestPanel'));
const SettingsPanel = lazy(() => import('../panels/SettingsPanel'));
const IntelligenceDashboardPanel = lazy(() => import('../panels/IntelligenceDashboardPanel'));
const RegimePanel = lazy(() => import('../panels/RegimePanel'));
const PortfolioBrainPanel = lazy(() => import('../panels/PortfolioBrainPanel'));
const MetaAllocatorPanel = lazy(() => import('../panels/MetaAllocatorPanel'));
const GridComputePanel = lazy(() => import('../panels/GridComputePanel'));
const StrategyRoutingPanel = lazy(() => import('../panels/StrategyRoutingPanel'));

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
  'backtest':       { id: 'backtest',       title: 'Backtests',          component: BacktestPanel,    closable: true, group: 'main' },
  'settings':       { id: 'settings',       title: 'Settings',           component: SettingsPanel,    closable: true, group: 'settings' },
  'intelligence':   { id: 'intelligence',   title: 'Intelligence',       component: IntelligenceDashboardPanel, closable: true, group: 'main' },
  'regimes':        { id: 'regimes',        title: 'Regimes',            component: RegimePanel,      closable: true, group: 'main' },
  'portfolio-brain':{ id: 'portfolio-brain', title: 'Portfolio Brain',   component: PortfolioBrainPanel, closable: true, group: 'main' },
  'meta-allocator': { id: 'meta-allocator', title: 'Meta Allocator',    component: MetaAllocatorPanel, closable: true, group: 'main' },
  'grid-compute':   { id: 'grid-compute',   title: 'Grid Compute',      component: GridComputePanel,   closable: true, group: 'main' },
  'strategy-routing': { id: 'strategy-routing', title: 'Strategy Routing', component: StrategyRoutingPanel, closable: true, group: 'settings' },
};

export function getPanelComponent(panelId) {
  return PANELS[panelId]?.component || null;
}

export function getPanelTitle(panelId) {
  return PANELS[panelId]?.title || panelId;
}
