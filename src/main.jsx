import { createRoot } from 'react-dom/client';
import './index.css';
import App from './App';

// Remove the HTML-level preload indicator once React mounts
const preload = document.getElementById('preload');
if (preload) preload.remove();

// Show the window now that web content is ready (window starts hidden to avoid black flash)
import('@tauri-apps/api/window')
  .then(({ getCurrentWindow }) => getCurrentWindow().show())
  .catch(() => {}); // silently fail in dev/non-Tauri context

const root = createRoot(document.getElementById('root'));
root.render(<App />);
