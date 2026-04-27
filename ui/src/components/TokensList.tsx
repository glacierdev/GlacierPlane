import { useEffect, useState } from 'react';
import { Link } from 'react-router-dom';
import { fetchTokens } from '../api';
import { AgentToken } from '../types';

function formatDate(dateString: string): string {
  return new Date(dateString).toLocaleDateString();
}

function getStatusSummary(token: AgentToken): { text: string; className: string } {
  if (token.running_count > 0) {
    return { text: `${token.running_count} running`, className: 'status-running' };
  }
  if (token.connected_count > 0) {
    return { text: `${token.connected_count} connected`, className: 'status-connected' };
  }
  if (token.agents_count > 0) {
    return { text: 'All idle', className: 'status-idle' };
  }
  return { text: 'No agents', className: 'status-disconnected' };
}

export function TokensList() {
  const [tokens, setTokens] = useState<AgentToken[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    async function loadTokens() {
      try {
        const data = await fetchTokens();
        setTokens(data);
        setError(null);
      } catch (err) {
        setError(err instanceof Error ? err.message : 'Failed to load tokens');
      } finally {
        setLoading(false);
      }
    }

    loadTokens();

    const interval = setInterval(loadTokens, 5000);
    return () => clearInterval(interval);
  }, []);

  if (loading) {
    return (
      <div className="loading">
        <div className="spinner"></div>
        Loading agent tokens...
      </div>
    );
  }

  if (error) {
    return (
      <div className="error">
        <p>⚠️ {error}</p>
        <p style={{ marginTop: '0.5rem', fontSize: '0.875rem', opacity: 0.8 }}>
          Make sure the control plane is running and accessible.
        </p>
      </div>
    );
  }

  if (tokens.length === 0) {
    return (
      <div className="empty-state">
        <div className="empty-icon">🤖</div>
        <p>No agent tokens created yet.</p>
        <p style={{ fontSize: '0.875rem', marginTop: '0.5rem' }}>
          Create a registration token in the database to get started.
        </p>
      </div>
    );
  }

  return (
    <div className="tokens-grid">
      {tokens.map((token) => {
        const status = getStatusSummary(token);

        return (
          <Link to={`/tokens/${token.id}`} className="token-card" key={token.id}>
            <div className="token-header">
              <div>
                <div className="token-name">{token.name || 'Unnamed Token'}</div>
                <div className="token-preview">{token.token_preview}</div>
              </div>
              <span className={`status-badge ${status.className}`}>
                {status.text}
              </span>
            </div>

            {token.description && (
              <div className="token-description">{token.description}</div>
            )}

            <div className="token-stats">
              <div className="stat-item">
                <span className="stat-value">{token.agents_count}</span>
                <span className="stat-label">Agents</span>
              </div>
              <div className="stat-item">
                <span className="stat-value connected">{token.connected_count}</span>
                <span className="stat-label">Connected</span>
              </div>
              <div className="stat-item">
                <span className="stat-value running">{token.running_count}</span>
                <span className="stat-label">Running</span>
              </div>
            </div>

            <div className="token-meta">
              <div className="meta-item">
                <span className="meta-label">Created</span>
                <span className="meta-value">{formatDate(token.created_at)}</span>
              </div>
              {token.expires_at && (
                <div className="meta-item">
                  <span className="meta-label">Expires</span>
                  <span className="meta-value">{formatDate(token.expires_at)}</span>
                </div>
              )}
            </div>
          </Link>
        );
      })}
    </div>
  );
}

