import { useEffect, useState } from 'react';
import { useNavigate, Link } from 'react-router-dom';
import { 
  fetchUserAgentTokens,
  createAgentToken,
  deleteAgentToken
} from '../api';
import { AgentToken, AgentTokenCreateData } from '../types';

function formatTimeAgo(dateString: string | null): { text: string; className: string } {
  if (!dateString) {
    return { text: 'Never', className: 'time-ago-old' };
  }
  
  const date = new Date(dateString);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffSec = Math.floor(diffMs / 1000);
  const diffMin = Math.floor(diffSec / 60);
  const diffHour = Math.floor(diffMin / 60);
  const diffDay = Math.floor(diffHour / 24);
  
  if (diffSec < 60) {
    return { text: `${diffSec}s ago`, className: 'time-ago-recent' };
  }
  if (diffMin < 5) {
    return { text: `${diffMin}m ago`, className: 'time-ago-recent' };
  }
  if (diffMin < 60) {
    return { text: `${diffMin}m ago`, className: 'time-ago-medium' };
  }
  if (diffHour < 24) {
    return { text: `${diffHour}h ago`, className: 'time-ago-medium' };
  }
  return { text: `${diffDay}d ago`, className: 'time-ago-old' };
}

interface TokenCardProps {
  token: AgentToken;
  onDelete: (id: string) => void;
  deleting: boolean;
}

function getAgentStatus(token: AgentToken): { text: string; className: string } {
  if ((token.running_count ?? 0) > 0) {
    return { text: 'Running job', className: 'status-running' };
  }
  if ((token.connected_count ?? 0) > 0) {
    return { text: 'Connected', className: 'status-connected' };
  }
  return { text: 'Disconnected', className: 'status-disconnected' };
}

function TokenCard({ token, onDelete, deleting }: TokenCardProps) {
  const tokenPreview = token.token_preview || (token.token ? token.token.substring(0, 8) + '...' : '********');
  const createdAt = formatTimeAgo(token.created_at);
  const status = getAgentStatus(token);

  return (
    <Link to={`/agents/tokens/${token.id}`} className="token-card" style={{ cursor: 'pointer', textDecoration: 'none' }}>
      <div className="token-header">
        <div>
          <div className="token-name">{token.name || 'Unnamed Token'}</div>
          <div className="token-preview">{tokenPreview}</div>
        </div>
        <div style={{ display: 'flex', gap: '0.5rem', alignItems: 'center' }}>
          <span className={`status-badge ${status.className}`} style={{ fontSize: '0.75rem' }}>
            {status.text}
          </span>
        </div>
      </div>

      {token.description && (
        <div className="token-description">{token.description}</div>
      )}

      <div className="token-meta">
        <div className="meta-item">
          <span className="meta-label">Created</span>
          <span className="meta-value">{createdAt.text}</span>
        </div>
        {token.agents_count !== undefined && (
          <div className="meta-item">
            <span className="meta-label">Registrations</span>
            <span className="meta-value">{token.agents_count}</span>
          </div>
        )}
      </div>

      <div className="token-actions" onClick={(e) => e.stopPropagation()}>
        <button
          className="btn-danger btn-small"
          onClick={(e) => {
            e.preventDefault();
            onDelete(token.id);
          }}
          disabled={deleting}
        >
          {deleting ? 'Deleting...' : 'Delete Token'}
        </button>
      </div>
    </Link>
  );
}

interface CreateTokenModalProps {
  onClose: () => void;
  onCreated: (token: string) => void;
}

function CreateTokenModal({ onClose, onCreated }: CreateTokenModalProps) {
  const [formData, setFormData] = useState<AgentTokenCreateData>({
    name: '',
    description: '',
  });
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!formData.name.trim()) {
      setError('Name is required');
      return;
    }

    setSubmitting(true);
    setError(null);

    try {
      const result = await createAgentToken(formData);
      if (result.token) {
        onCreated(result.token);
      } else {
        onClose();
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to create token');
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal-content" onClick={(e) => e.stopPropagation()}>
        <h2>Create Registration Token</h2>
        <p className="modal-description">
          Create a token to register new agents. The token will only be shown once.
        </p>

        {error && <div className="form-error">{error}</div>}

        <form onSubmit={handleSubmit}>
          <div className="form-group">
            <label>Name *</label>
            <input
              type="text"
              value={formData.name}
              onChange={(e) => setFormData({ ...formData, name: e.target.value })}
              placeholder="e.g., Production Agents"
              disabled={submitting}
            />
          </div>

          <div className="form-group">
            <label>Description</label>
            <textarea
              value={formData.description || ''}
              onChange={(e) => setFormData({ ...formData, description: e.target.value })}
              placeholder="Optional description"
              disabled={submitting}
              rows={3}
            />
          </div>

          <div className="modal-actions">
            <button type="button" className="btn-secondary" onClick={onClose} disabled={submitting}>
              Cancel
            </button>
            <button type="submit" className="btn-primary" disabled={submitting}>
              {submitting ? 'Creating...' : 'Create Token'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}

interface TokenCreatedModalProps {
  token: string;
  onClose: () => void;
}

function TokenCreatedModal({ token, onClose }: TokenCreatedModalProps) {
  const [copied, setCopied] = useState(false);

  const copyToClipboard = async () => {
    try {
      await navigator.clipboard.writeText(token);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      const textArea = document.createElement('textarea');
      textArea.value = token;
      document.body.appendChild(textArea);
      textArea.select();
      document.execCommand('copy');
      document.body.removeChild(textArea);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  };

  return (
    <div className="modal-overlay">
      <div className="modal-content">
        <h2>Token Created</h2>
        <div className="modal-warning">
          Copy this token now - you won't be able to see it again!
        </div>

        <div className="token-display">
          <code>{token}</code>
          <button className="btn-secondary btn-small" onClick={copyToClipboard}>
            {copied ? 'Copied!' : 'Copy'}
          </button>
        </div>

        <p style={{ fontSize: '0.875rem', color: 'var(--text-muted)', marginBottom: '1rem' }}>
          Use this token when starting agents:
        </p>
        <div style={{ 
          background: 'var(--bg-overlay)', 
          padding: '0.75rem 1rem', 
          borderRadius: '8px',
          fontFamily: "'JetBrains Mono', monospace",
          fontSize: '0.8rem',
          color: 'var(--accent-teal)',
          wordBreak: 'break-all'
        }}>
          buildkite-agent start --token {token.substring(0, 20)}...
        </div>

        <div className="modal-actions">
          <button className="btn-primary" onClick={onClose}>
            Done
          </button>
        </div>
      </div>
    </div>
  );
}

export function AgentsList() {
  const [tokens, setTokens] = useState<AgentToken[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [showCreateToken, setShowCreateToken] = useState(false);
  const [createdToken, setCreatedToken] = useState<string | null>(null);
  const [deletingToken, setDeletingToken] = useState<string | null>(null);
  const navigate = useNavigate();

  const loadData = async () => {
    try {
      const tokensData = await fetchUserAgentTokens();
      setTokens(tokensData);
      setError(null);
    } catch (err) {
      if (err instanceof Error && err.message === 'Not authenticated') {
        navigate('/login');
        return;
      }
      setError(err instanceof Error ? err.message : 'Failed to load data');
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadData();
    const interval = setInterval(loadData, 5000);
    return () => clearInterval(interval);
  }, [navigate]);

  const handleTokenCreated = (token: string) => {
    setShowCreateToken(false);
    setCreatedToken(token);
    loadData();
  };

  const handleDeleteToken = async (id: string) => {
    if (!confirm('Delete this token? Agents using it will no longer be able to connect.')) {
      return;
    }

    setDeletingToken(id);
    try {
      await deleteAgentToken(id);
      await loadData();
    } catch (err) {
      alert(err instanceof Error ? err.message : 'Failed to delete token');
    } finally {
      setDeletingToken(null);
    }
  };

  if (loading) {
    return (
      <div className="loading">
        <div className="spinner"></div>
        Loading agents...
      </div>
    );
  }

  if (error) {
    return (
      <div className="error">
        <p>Warning: {error}</p>
        <p style={{ marginTop: '0.5rem', fontSize: '0.875rem', opacity: 0.8 }}>
          Make sure the control plane is running and accessible.
        </p>
      </div>
    );
  }

  const connectedCount = tokens.reduce((sum, t) => sum + (t.connected_count || 0), 0);
  const runningCount = tokens.reduce((sum, t) => sum + (t.running_count || 0), 0);

  return (
    <div>
      <div className="pipelines-header">
        <h1 className="page-title" style={{ marginBottom: 0 }}>
          <span className="page-title-icon">🤖</span>
          Agents
        </h1>
        <button className="btn-primary" onClick={() => setShowCreateToken(true)}>
          + New Token
        </button>
      </div>

      <div className="stats-grid" style={{ marginBottom: '1.5rem' }}>
        <div className="stat-box">
          <div className="stat-box-value">{tokens.length}</div>
          <div className="stat-box-label">Total Tokens</div>
        </div>
        <div className="stat-box">
          <div className="stat-box-value connected">{connectedCount}</div>
          <div className="stat-box-label">Connected</div>
        </div>
        <div className="stat-box">
          <div className="stat-box-value running">{runningCount}</div>
          <div className="stat-box-label">Running Jobs</div>
        </div>
      </div>

      {tokens.length === 0 ? (
        <div className="empty-state">
          <div className="empty-icon">🤖</div>
          <p>No registration tokens yet.</p>
          <p style={{ fontSize: '0.875rem', marginTop: '0.5rem' }}>
            Create a token to register new agents.
          </p>
          <button
            className="btn-primary"
            style={{ marginTop: '1rem' }}
            onClick={() => setShowCreateToken(true)}
          >
            Create Token
          </button>
        </div>
      ) : (
        <div className="tokens-grid">
          {tokens.map((token) => (
            <TokenCard
              key={token.id}
              token={token}
              onDelete={handleDeleteToken}
              deleting={deletingToken === token.id}
            />
          ))}
        </div>
      )}

      {showCreateToken && (
        <CreateTokenModal
          onClose={() => setShowCreateToken(false)}
          onCreated={handleTokenCreated}
        />
      )}

      {createdToken && (
        <TokenCreatedModal
          token={createdToken}
          onClose={() => setCreatedToken(null)}
        />
      )}
    </div>
  );
}
