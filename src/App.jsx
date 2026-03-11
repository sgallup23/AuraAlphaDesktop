import { BrowserRouter, Routes, Route } from 'react-router-dom';
import NavBar from './components/NavBar';
import DashboardPage from './pages/DashboardPage';
import BrokerSetupPage from './pages/BrokerSetupPage';
import BotManagerPage from './pages/BotManagerPage';

export default function App() {
  return (
    <BrowserRouter>
      <div style={{ minHeight: '100vh', background: '#0D1117' }}>
        <NavBar />
        <Routes>
          <Route path="/" element={<DashboardPage />} />
          <Route path="/brokers" element={<BrokerSetupPage />} />
          <Route path="/bots" element={<BotManagerPage />} />
        </Routes>
      </div>
    </BrowserRouter>
  );
}
