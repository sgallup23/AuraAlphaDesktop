import { useState } from 'react';
import { useAuth } from '../contexts/AuthContext';

export default function LoginPage() {
  const { login, enterStandaloneMode } = useAuth();
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [remember, setRemember] = useState(true);
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);

  const handleSubmit = async (e) => {
    e.preventDefault();
    setError('');
    setLoading(true);
    try {
      await login(username, password, remember);
    } catch (err) {
      setError(err.message);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="min-h-screen flex items-center justify-center bg-aura-bg p-4">
      <div className="w-full max-w-md">
        {/* Logo */}
        <div className="text-center mb-8">
          <div className="inline-flex items-center justify-center w-16 h-16 rounded-2xl mb-4"
               style={{ background: 'linear-gradient(135deg, #58A6FF, #BC8CFF, #3FB950)' }}>
            <span className="text-white text-3xl font-bold">A</span>
          </div>
          <h1 className="text-2xl font-bold text-aura-text">Aura Alpha</h1>
          <p className="text-aura-muted text-sm mt-1">Algorithmic Trading Desktop</p>
        </div>

        {/* Login Card */}
        <form onSubmit={handleSubmit} className="glass-panel p-6 space-y-4">
          {error && (
            <div className="p-3 rounded-lg bg-aura-red/10 border border-aura-red/20 text-aura-red text-sm">
              {error}
            </div>
          )}

          <div>
            <label className="block text-sm text-aura-muted mb-1.5 font-medium">Username</label>
            <input
              type="text"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              className="input-field"
              placeholder="Enter your username"
              autoFocus
              required
            />
          </div>

          <div>
            <label className="block text-sm text-aura-muted mb-1.5 font-medium">Password</label>
            <input
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              className="input-field"
              placeholder="Enter your password"
              required
            />
          </div>

          <div className="flex items-center gap-2">
            <input
              type="checkbox"
              id="remember"
              checked={remember}
              onChange={(e) => setRemember(e.target.checked)}
              className="w-4 h-4 rounded border-aura-border bg-aura-surface2 text-aura-blue focus:ring-aura-blue"
            />
            <label htmlFor="remember" className="text-sm text-aura-muted cursor-pointer">
              Remember me
            </label>
          </div>

          <button
            type="submit"
            disabled={loading || !username || !password}
            className="btn-primary w-full"
          >
            {loading ? 'Signing in...' : 'Sign In'}
          </button>

          {/* Divider */}
          <div className="flex items-center gap-3 py-1">
            <div className="flex-1 h-px bg-aura-border" />
            <span className="text-xs text-aura-muted">or</span>
            <div className="flex-1 h-px bg-aura-border" />
          </div>

          {/* Standalone / Explorer mode */}
          <button
            type="button"
            onClick={enterStandaloneMode}
            className="w-full py-2.5 px-4 rounded-lg border border-aura-border text-aura-muted
                       hover:text-aura-text hover:border-aura-blue/50 transition-colors text-sm font-medium"
          >
            Continue as Explorer (Offline)
          </button>

          <p className="text-center text-xs text-aura-muted mt-2">
            Explorer mode: paper trading, 10 strategies, no account required.
            <br />
            Sign in to unlock all features with your subscription.
          </p>
        </form>
      </div>
    </div>
  );
}
