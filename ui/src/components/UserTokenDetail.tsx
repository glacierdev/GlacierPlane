import { useEffect, useState } from 'react';
import { useParams, Link, useNavigate } from 'react-router-dom';
import { fetchUserAgentTokenDetail } from '../api';
import { AgentTokenDetailResponse, Agent } from '../types';

function formatDate(dateString: string | null): string {
  if (!dateString) return '-';
  return new Date(dateString).toLocaleString();
}

function formatTimeAgo(dateString: string | null): { text: string; className: string } {
  if (!dateString) return { text: 'Never', className: 'time-relative' };
  
  const date = new Date(dateString);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffSecs = Math.floor(diffMs / 1000);
  const diffMins = Math.floor(diffSecs / 60);
  const diffHours = Math.floor(diffMins / 60);
  const diffDays = Math.floor(diffHours / 24);

  let text: string;
  let className: string;

  if (diffSecs < 60) {
    text = `${diffSecs}s ago`;
    className = 'time-ago-recent';
  } else if (diffMins < 60) {
    text = `${diffMins}m ago`;
    className = diffMins < 5 ? 'time-ago-recent' : 'time-ago-stale';
  } else if (diffHours < 24) {
    text = `${diffHours}h ago`;
    className = 'time-ago-stale';
  } else {
    text = `${diffDays}d ago`;
    className = 'time-ago-old';
  }

  return { text, className };
}

function getAgentStatusClass(status: string, currentJobId: string | null): string {
  if (currentJobId) return 'status-running';
  switch (status.toLowerCase()) {
    case 'connected': return 'status-connected';
    case 'disconnected': return 'status-disconnected';
    case 'lost': return 'status-lost';
    case 'registered': return 'status-registered';
    default: return 'status-idle';
  }
}

interface AgentCardProps {
  agent: Agent;
}

function AgentCard({ agent }: AgentCardProps) {
  const lastSeen = formatTimeAgo(agent.last_seen);
  const statusClass = getAgentStatusClass(agent.connection_state, agent.current_job_id);
  const displayStatus = agent.current_job_id ? 'Running Job' : agent.connection_state;

  const queueName = agent.queue_name || agent.meta_data?.find(t => t.startsWith('queue='))?.replace('queue=', '') || null;

  return (
    <div className="agent-instance-card">
      <div className="agent-instance-header">
        <div>
          <div className="agent-instance-name">{agent.name}</div>
          <div className="agent-instance-hostname">{agent.hostname}</div>
        </div>
        <span className={`status-badge-sm ${statusClass}`}>{displayStatus}</span>
      </div>
      <div className="agent-instance-meta">
        <span>{agent.os} / {agent.arch}</span>
        <span>v{agent.version}</span>
        <span className={lastSeen.className}>Seen {lastSeen.text}</span>
      </div>

      {(agent.priority != null || queueName) && (
        <div style={{ marginTop: '0.75rem', fontSize: '0.8rem', color: 'var(--text-muted)', display: 'flex', gap: '1rem', flexWrap: 'wrap' }}>
          {agent.priority != null && (
            <span>Priority: <strong style={{ color: 'var(--text)' }}>{agent.priority}</strong></span>
          )}
          {queueName && (
            <span>Queue: <span style={{ color: 'var(--accent-teal)', fontFamily: "'JetBrains Mono', monospace" }}>{queueName}</span></span>
          )}
        </div>
      )}

      {agent.meta_data && agent.meta_data.length > 0 && (
        <div className="agent-tags">
          {agent.meta_data.map((tag, idx) => (
            <span key={idx} className="tag">{tag}</span>
          ))}
        </div>
      )}
    </div>
  );
}

export function UserTokenDetail() {
  const { tokenId } = useParams<{ tokenId: string }>();
  const navigate = useNavigate();
  const [data, setData] = useState<AgentTokenDetailResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const loadData = async () => {
    if (!tokenId) return;
    
    try {
      const tokenResponse = await fetchUserAgentTokenDetail(tokenId);
      setData(tokenResponse);
      setError(null);
    } catch (err) {
      if (err instanceof Error && err.message === 'Not authenticated') {
        navigate('/login');
        return;
      }
      setError(err instanceof Error ? err.message : 'Failed to load token details');
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadData();

    const interval = setInterval(loadData, 10000);
    return () => clearInterval(interval);
  }, [tokenId, navigate]);

  if (loading) {
    return (
      <div className="loading">
        <div className="spinner"></div>
        Loading token details...
      </div>
    );
  }

  if (error || !data) {
    return (
      <div>
        <Link to="/agents" className="back-link">← Back to Registration Tokens</Link>
        <div className="error">
          <p>⚠️ {error || 'Token not found'}</p>
        </div>
      </div>
    );
  }

  const { token, agents } = data;

  return (
    <div>
      <Link to="/agents" className="back-link">← Back to Registration Tokens</Link>

      <div className="token-detail-header">
        <div className="token-detail-title">
          <div>
            <div className="token-detail-name">{token.name || 'Unnamed Token'}</div>
            <div className="token-preview">{token.token_preview}</div>
          </div>
          <div className="token-detail-stats">
            <div className="stat-box">
              <span className="stat-box-value">{token.agents_count || 0}</span>
              <span className="stat-box-label">Total Agents</span>
            </div>
            <div className="stat-box connected">
              <span className="stat-box-value">{token.connected_count || 0}</span>
              <span className="stat-box-label">Connected</span>
            </div>
            <div className="stat-box running">
              <span className="stat-box-value">{token.running_count || 0}</span>
              <span className="stat-box-label">Running Jobs</span>
            </div>
          </div>
        </div>

        {token.description && (
          <div className="token-detail-description">{token.description}</div>
        )}

        <div className="token-detail-meta">
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
      </div>

      <div className="section">
        <h2 className="section-title">
          Registered Agent Instances
          <span className="section-count">{agents.length}</span>
        </h2>

        {agents.length === 0 ? (
          <div className="empty-state-sm">
            <p>No agents have registered with this token yet.</p>
            <p style={{ fontSize: '0.875rem', marginTop: '0.5rem', color: 'var(--text-muted)' }}>
              Use this token when starting agents: <code style={{ fontSize: '0.875rem' }}>buildkite-agent start --token &lt;token&gt;</code>
            </p>
          </div>
        ) : (
          <div className="agents-instances-grid">
            {agents.map((agent) => (
              <AgentCard key={agent.id} agent={agent} />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
