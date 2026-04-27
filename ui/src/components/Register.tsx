import { useState, FormEvent } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { register } from '../api';

interface RegisterProps {
  onLogin: () => void;
}

export function Register({ onLogin }: RegisterProps) {
  const [name, setName] = useState('');
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [status, setStatus] = useState<string>('');
  const navigate = useNavigate();

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    setError(null);
    setStatus('Validating...');

    if (password.length < 6) {
      setError('Password must be at least 6 characters');
      setStatus('');
      return;
    }

    setLoading(true);
    setStatus('Sending request to server...');

    try {
      await register(email, name, password);
      setStatus('Registration successful! Redirecting...');
      onLogin();
      navigate('/');
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Registration failed';
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
              <h1 className="auth-title">Create your account</h1>
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
                <label className="form-label">Full name</label>
                <input
                  type="text"
                  className="form-input"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder="Butterscotch"
                  autoFocus
                  required
                />
              </div>

              <div className="form-field">
                <label className="form-label">Email address</label>
                <input
                  type="email"
                  className="form-input"
                  value={email}
                  onChange={(e) => setEmail(e.target.value)}
                  placeholder="dev@gg.com"
                  required
                />
              </div>

              <div className="form-field">
                <label className="form-label">Password</label>
                <input
                  type="password"
                  className="form-input"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  placeholder="At least 6 characters"
                  required
                  minLength={6}
                />
              </div>

              <button
                type="submit"
                className="auth-submit-btn"
                disabled={loading}
              >
                {loading ? 'Creating account...' : 'Sign up'}
              </button>
            </form>

            <div className="auth-hints">
              <p>
                Already have an account?{' '}
                <Link to="/login" className="auth-link">
                  Login
                </Link>
              </p>
            </div>
          </article>
        </section>
      </div>

      <footer className="auth-footer">
        <p>
          By signing up, you agree to use this control plane for authorized purposes only. And that Greenland for penguins
        </p>
      </footer>
    </div>
  );
}
