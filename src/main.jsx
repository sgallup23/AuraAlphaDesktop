import { createRoot } from 'react-dom/client';
import App from './App';

// Global reset styles
const globalCSS = `
  * { margin: 0; padding: 0; box-sizing: border-box; }
  body {
    background: #0D1117;
    color: #E6EDF3;
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', 'Plus Jakarta Sans', sans-serif;
    overflow-x: hidden;
  }
  ::-webkit-scrollbar { width: 8px; height: 8px; }
  ::-webkit-scrollbar-track { background: #0D1117; }
  ::-webkit-scrollbar-thumb { background: #30363D; border-radius: 4px; }
  ::-webkit-scrollbar-thumb:hover { background: #484F58; }
  input::placeholder { color: #484F58; }
  select option { background: #161B22; color: #E6EDF3; }
  a { color: #58A6FF; }
`;

// Inject global styles
const style = document.createElement('style');
style.textContent = globalCSS;
document.head.appendChild(style);

const root = createRoot(document.getElementById('root'));
root.render(<App />);
