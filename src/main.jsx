import { createRoot } from 'react-dom/client';
import './index.css';
import App from './App';

// Remove the HTML-level preload indicator once React mounts
const preload = document.getElementById('preload');
if (preload) preload.remove();

const root = createRoot(document.getElementById('root'));
root.render(<App />);
