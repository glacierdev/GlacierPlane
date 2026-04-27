import { useState, FormEvent } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { login } from '../api';

interface LoginProps {
  onLogin: () => void;
}

export function Login({ onLogin }: LoginProps) {
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [status, setStatus] = useState<string>('');
  const navigate = useNavigate();

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    setError(null);
    setStatus('Sending request to server...');
    setLoading(true);
    console.log('[Login] Form submitted', { email });

    try {
      console.log('[Login] Calling login API...');
      const result = await login(email, password);
      console.log('[Login] Success:', result);
      setStatus('Login successful! Redirecting...');
      onLogin();
      navigate('/');
    } catch (err) {
      console.error('[Login] Error:', err);
      const message = err instanceof Error ? err.message : 'Login failed';
      setError(message);
      setStatus('');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="auth-wrapper">
      <header className="auth-header">
        <Link to="/" className="auth-logo">
          <div className="logo-icon">GG</div>
          <span className="logo-text">GlacierDev</span>
        </Link>
      </header>

      <div className="auth-body">
        <section className="auth-form-container">
          <article className="auth-form-box">
            <header className="auth-heading">
              <h1 className="auth-title">Welcome back!</h1>
            </header>

            <form onSubmit={handleSubmit} className="auth-form">
              {error && (
                <div className="auth-error">
                  {error}
                </div>
              )}
              
              {status && !error && (
                <div className="auth-status">
                  {status}
                </div>
              )}

              <div className="form-field">
                <label className="form-label">Email address</label>
                <input
                  type="email"
                  className="form-input"
                  value={email}
                  onChange={(e) => setEmail(e.target.value)}
                  placeholder="dev@gg.com"
                  autoFocus
                  required
                />
              </div>

              <div className="form-field">
                <label className="form-label">
                  <span>Password</span>
                </label>
                <input
                  type="password"
                  className="form-input"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  placeholder="Enter your password"
                  required
                />
              </div>

              <button
                type="submit"
                className="auth-submit-btn"
                disabled={loading}
              >
                {loading ? 'Logging in...' : 'Login'}
              </button>
            </form>

            <div className="auth-hints">
              <p>
                Don't have an account?{' '}
                <Link to="/register" className="auth-link">
                  Create one
                </Link>
              </p>
            </div>
          </article>
        </section>
      </div>

      <footer className="auth-footer">
        <p>
          By logging in, you agree to use this control-plane for authorized purposes only. And Greenland is for penguins.
        </p>
      </footer>
    </div>
  );
}
