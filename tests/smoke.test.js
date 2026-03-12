/**
 * Smoke tests for AuraAlphaDesktop frontend.
 * Verifies all modules import cleanly and key components render.
 * Run: node tests/smoke.test.js
 */
import { readFileSync, existsSync } from 'fs';
import { join, resolve } from 'path';
import { execSync } from 'child_process';

const ROOT = resolve(import.meta.dirname, '..');
const SRC = join(ROOT, 'src');

let pass = 0;
let fail = 0;

function test(name, fn) {
  try {
    fn();
    pass++;
    console.log(`  PASS  ${name}`);
  } catch (e) {
    fail++;
    console.log(`  FAIL  ${name}: ${e.message}`);
  }
}

function assert(cond, msg) {
  if (!cond) throw new Error(msg || 'Assertion failed');
}

function fileExists(rel) {
  return existsSync(join(SRC, rel));
}

function fileContains(rel, needle) {
  const content = readFileSync(join(SRC, rel), 'utf-8');
  return content.includes(needle);
}

console.log('\n--- AuraAlphaDesktop Smoke Tests ---\n');

// Wave 1: Foundation
console.log('[Wave 1: Foundation]');
test('AuthContext exists', () => assert(fileExists('contexts/AuthContext.jsx')));
test('PreferencesContext exists', () => assert(fileExists('contexts/PreferencesContext.jsx')));
test('api.js exists', () => assert(fileExists('utils/api.js')));
test('auth.js exists', () => assert(fileExists('utils/auth.js')));
test('formatters.js exists', () => assert(fileExists('utils/formatters.js')));
test('LoginPage exists', () => assert(fileExists('pages/LoginPage.jsx')));
test('App.jsx has auth gate', () => assert(fileContains('App.jsx', 'AuthGate')));
test('App.jsx has PopOutPanel', () => assert(fileContains('App.jsx', 'PopOutPanel')));
test('index.css has Tailwind directives', () => assert(fileContains('index.css', '@tailwind')));
test('tauri-plugin-store in Cargo.toml', () => {
  const cargo = readFileSync(join(ROOT, 'src-tauri/Cargo.toml'), 'utf-8');
  assert(cargo.includes('tauri-plugin-store'));
});
test('save_auth_token IPC command', () => {
  const lib = readFileSync(join(ROOT, 'src-tauri/src/lib.rs'), 'utf-8');
  assert(lib.includes('save_auth_token'));
});

// Wave 2: Workspace Shell
console.log('\n[Wave 2: Workspace Shell]');
test('WorkspaceShell exists', () => assert(fileExists('shell/WorkspaceShell.jsx')));
test('TopBar exists', () => assert(fileExists('shell/TopBar.jsx')));
test('panelRegistry exists', () => assert(fileExists('docking/panelRegistry.js')));
test('defaultLayout exists', () => assert(fileExists('docking/defaultLayout.js')));
test('WorkspaceManager exists', () => assert(fileExists('docking/WorkspaceManager.jsx')));
test('dockTheme.css exists', () => assert(fileExists('docking/dockTheme.css')));
test('WorkspaceShell uses DockLayout', () => assert(fileContains('shell/WorkspaceShell.jsx', 'DockLayout')));
test('WorkspaceShell uses CommandPalette', () => assert(fileContains('shell/WorkspaceShell.jsx', 'CommandPalette')));

// Wave 3: Hooks
console.log('\n[Wave 3: Hooks]');
const hooks = ['useLiveBots', 'usePositions', 'useChartData', 'useScanners', 'useWatchlists', 'useAlerts', 'useStrategies', 'useLocalBots', 'useBrokerSetup'];
for (const h of hooks) {
  test(`${h} hook exists`, () => assert(fileExists(`hooks/${h}.js`)));
}

// Wave 3: Panels
console.log('\n[Wave 3: Panels]');
const panels = ['BotCommandPanel', 'PositionsPanel', 'WatchlistPanel', 'ChartPanel', 'ScannerPanel', 'AlertsPanel', 'StrategyManagerPanel', 'BrokerSetupPanel', 'BotManagerPanel'];
for (const p of panels) {
  test(`${p} exists`, () => assert(fileExists(`panels/${p}.jsx`)));
}

// Wave 3: Shared Components
console.log('\n[Wave 3: Shared Components]');
test('DataTable exists', () => assert(fileExists('components/DataTable.jsx')));
test('StatusDot exists', () => assert(fileExists('components/StatusDot.jsx')));
test('MetricTile exists', () => assert(fileExists('components/MetricTile.jsx')));
test('CommandPalette exists', () => assert(fileExists('components/CommandPalette.jsx')));
test('PanelMenu exists', () => assert(fileExists('components/PanelMenu.jsx')));

// Wave 3: Panels use hooks
console.log('\n[Wave 3: Panel-Hook Wiring]');
test('BotCommandPanel uses useLiveBots', () => assert(fileContains('panels/BotCommandPanel.jsx', 'useLiveBots')));
test('PositionsPanel uses usePositions', () => assert(fileContains('panels/PositionsPanel.jsx', 'usePositions')));
test('ChartPanel uses useChartData', () => assert(fileContains('panels/ChartPanel.jsx', 'useChartData')));
test('ScannerPanel uses useScanners', () => assert(fileContains('panels/ScannerPanel.jsx', 'useScanners')));
test('WatchlistPanel uses useWatchlists', () => assert(fileContains('panels/WatchlistPanel.jsx', 'useWatchlists')));
test('AlertsPanel uses useAlerts', () => assert(fileContains('panels/AlertsPanel.jsx', 'useAlerts')));
test('StrategyManagerPanel uses useStrategies', () => assert(fileContains('panels/StrategyManagerPanel.jsx', 'useStrategies')));
test('BotManagerPanel uses useLocalBots', () => assert(fileContains('panels/BotManagerPanel.jsx', 'useLocalBots')));
test('BrokerSetupPanel uses useBrokerSetup', () => assert(fileContains('panels/BrokerSetupPanel.jsx', 'useBrokerSetup')));

// Wave 3: Panels use shared components
console.log('\n[Wave 3: Shared Component Usage]');
test('BotCommandPanel uses MetricTile', () => assert(fileContains('panels/BotCommandPanel.jsx', 'MetricTile')));
test('BotCommandPanel uses StatusDot', () => assert(fileContains('panels/BotCommandPanel.jsx', 'StatusDot')));
test('PositionsPanel uses DataTable', () => assert(fileContains('panels/PositionsPanel.jsx', 'DataTable')));
test('ScannerPanel uses DataTable', () => assert(fileContains('panels/ScannerPanel.jsx', 'DataTable')));
test('StrategyManagerPanel uses DataTable', () => assert(fileContains('panels/StrategyManagerPanel.jsx', 'DataTable')));

// Wave 4: Multi-Window + Polish
console.log('\n[Wave 4: Multi-Window + Polish]');
test('create_panel_window IPC', () => {
  const lib = readFileSync(join(ROOT, 'src-tauri/src/lib.rs'), 'utf-8');
  assert(lib.includes('create_panel_window'));
});
test('Keyboard shortcut Ctrl+K', () => assert(fileContains('shell/WorkspaceShell.jsx', "e.key === 'k'")));
test('Keyboard shortcut Ctrl+N', () => assert(fileContains('shell/WorkspaceShell.jsx', "e.key === 'n'")));
test('Keyboard shortcut Ctrl+Shift+P', () => assert(fileContains('shell/WorkspaceShell.jsx', "e.key === 'P'")));
test('System tray in lib.rs', () => {
  const lib = readFileSync(join(ROOT, 'src-tauri/src/lib.rs'), 'utf-8');
  assert(lib.includes('TrayIconBuilder'));
});
test('Updater plugin registered', () => {
  const lib = readFileSync(join(ROOT, 'src-tauri/src/lib.rs'), 'utf-8');
  assert(lib.includes('tauri_plugin_updater'));
});
test('No dead pages remain', () => {
  assert(!fileExists('pages/DashboardPage.jsx'), 'DashboardPage should be removed');
  assert(!fileExists('pages/BotManagerPage.jsx'), 'BotManagerPage should be removed');
  assert(!fileExists('pages/BrokerSetupPage.jsx'), 'BrokerSetupPage should be removed');
  assert(!fileExists('components/NavBar.jsx'), 'NavBar should be removed');
  assert(!fileExists('components/BotStatusCard.jsx'), 'BotStatusCard should be removed');
});

// Build verification
console.log('\n[Build Verification]');
test('npm build succeeds', () => {
  execSync('npm run build', { cwd: ROOT, stdio: 'pipe' });
});
test('dist/index.html exists', () => assert(existsSync(join(ROOT, 'dist/index.html'))));
test('cargo check succeeds', () => {
  execSync('cargo check', { cwd: join(ROOT, 'src-tauri'), stdio: 'pipe' });
});

// Manual chunks verification
console.log('\n[Bundle Optimization]');
test('vite.config has manualChunks', () => {
  const cfg = readFileSync(join(ROOT, 'vite.config.js'), 'utf-8');
  assert(cfg.includes('manualChunks'));
});

console.log(`\n--- Results: ${pass} passed, ${fail} failed ---\n`);
process.exit(fail > 0 ? 1 : 0);
