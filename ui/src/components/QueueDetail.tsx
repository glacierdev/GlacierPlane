import { useState, useEffect } from 'react';
import { useParams, Link, useNavigate } from 'react-router-dom';
import { fetchQueueDetail, deleteQueue } from '../api';
import { QueueDetailResponse, QueueAgentToken } from '../types';

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

function getAgentTokenStatusInfo(agent: QueueAgentToken): { text: string; className: string } {
  if (agent.running_count > 0) {
    return { text: `${agent.running_count} running`, className: 'status-running' };
  }
  if (agent.connected_count > 0) {
    return { text: `${agent.connected_count} connected`, className: 'status-connected' };
  }
  if (agent.registrations_count > 0) {
    return { text: 'Idle', className: 'status-idle' };
  }
  return { text: 'No registrations', className: 'status-disconnected' };
}

function AgentTokenCard({ agent }: { agent: QueueAgentToken }) {
  const status = getAgentTokenStatusInfo(agent);
  const createdAt = formatTimeAgo(agent.created_at);

  return (
    <Link to={`/agents/tokens/${agent.id}`} className="agent-card" style={{ cursor: 'pointer', textDecoration: 'none' }}>
      <div className="agent-header">
        <div>
          <div className="agent-name">{agent.name || 'Unnamed Agent'}</div>
          <div className="agent-hostname" style={{ fontFamily: "'JetBrains Mono', monospace", fontSize: '0.8rem' }}>{agent.token_preview}</div>
        </div>
        <span className={`status-badge ${status.className}`}>
          {status.text}
        </span>
      </div>

      {agent.description && (
        <div style={{ fontSize: '0.85rem', color: 'var(--text-muted)', marginBottom: '0.75rem' }}>
          {agent.description}
        </div>
      )}
      
      <div className="agent-details">
        <div className="detail-item">
          <span className="detail-label">Registrations</span>
          <span className="detail-value">{agent.registrations_count}</span>
        </div>
        <div className="detail-item">
          <span className="detail-label">Connected</span>
          <span className="detail-value connected">{agent.connected_count}</span>
        </div>
        <div className="detail-item">
          <span className="detail-label">Running</span>
          <span className="detail-value running">{agent.running_count}</span>
        </div>
        <div className="detail-item">
          <span className="detail-label">Created</span>
          <span className={`detail-value ${createdAt.className}`}>{createdAt.text}</span>
        </div>
      </div>
    </Link>
  );
}

export function QueueDetail() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const [data, setData] = useState<QueueDetailResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [deleting, setDeleting] = useState(false);

  const loadData = async () => {
    if (!id) return;
    
    try {
      const result = await fetchQueueDetail(id);
      setData(result);
      setError(null);
    } catch (err) {
      if (err instanceof Error && err.message === 'Not authenticated') {
        navigate('/login');
        return;
      }
      setError(err instanceof Error ? err.message : 'Failed to load queue');
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadData();
    const interval = setInterval(loadData, 5000);
    return () => clearInterval(interval);
  }, [id, navigate]);

  const handleDelete = async () => {
    if (!id || !data) return;
    
    if (data.queue.is_default) {
      alert('Cannot delete the default queue for a pipeline');
      return;
    }
    
    if (!confirm(`Delete queue "${data.queue.name}"? This will unassign all agents from this queue.`)) {
      return;
    }
    
    setDeleting(true);
    try {
      await deleteQueue(id);
      navigate('/queues');
    } catch (err) {
      alert(err instanceof Error ? err.message : 'Failed to delete queue');
    } finally {
      setDeleting(false);
    }
  };

  if (loading) {
    return (
      <div className="loading">
        <div className="spinner"></div>
        Loading queue...
      </div>
    );
  }

  if (error || !data) {
    return (
      <div className="error">
        <p>Error: {error || 'Queue not found'}</p>
        <Link to="/queues" className="btn-secondary" style={{ marginTop: '1rem', display: 'inline-block' }}>
          Back to Queues
        </Link>
      </div>
    );
  }

  const { queue, agents, pipeline_name } = data;
  const connectedCount = agents.reduce((sum, a) => sum + a.connected_count, 0);
  const runningCount = agents.reduce((sum, a) => sum + a.running_count, 0);

  return (
    <div>
      <Link to="/queues" className="back-link">
        ← Back to Queues
      </Link>

      <div className="pipeline-detail-header">
        <div className="pipeline-detail-title">
          <div>
            <div className="pipeline-detail-name">{queue.name}</div>
            <div className="pipeline-slug" style={{ marginTop: '0.5rem' }}>{queue.key}</div>
            {queue.is_default && (
              <div style={{ 
                fontSize: '0.8rem', 
                color: 'var(--accent-teal)', 
                marginTop: '0.5rem',
                display: 'flex',
                alignItems: 'center',
                gap: '0.25rem'
              }}>
                <span>★</span> Default queue
              </div>
            )}
          </div>
          <div className="pipeline-actions">
            <Link to={`/queues/${id}/edit`} className="btn-secondary">
              Edit
            </Link>
            <button 
              className="btn-danger" 
              onClick={handleDelete}
              disabled={deleting || queue.is_default}
              title={queue.is_default ? 'Cannot delete default queue' : undefined}
            >
              {deleting ? 'Deleting...' : 'Delete'}
            </button>
          </div>
        </div>

        {queue.description && (
          <div className="pipeline-detail-description">{queue.description}</div>
        )}

        {pipeline_name && (
          <div style={{ color: 'var(--text-muted)', fontSize: '0.9rem', marginBottom: '1rem' }}>
            Linked to pipeline: <strong>{pipeline_name}</strong>
          </div>
        )}

        <div className="pipeline-detail-stats">
          <div className="stat-box">
            <span className="stat-box-value">{agents.length}</span>
            <span className="stat-box-label">Agents</span>
          </div>
          <div className="stat-box">
            <span className="stat-box-value connected">{connectedCount}</span>
            <span className="stat-box-label">Connected</span>
          </div>
          <div className="stat-box">
            <span className="stat-box-value running">{runningCount}</span>
            <span className="stat-box-label">Running</span>
          </div>
        </div>
      </div>

      <div className="section">
        <h2 className="section-title">
          Agents in Queue
          <span className="section-count">{agents.length}</span>
        </h2>

        {agents.length === 0 ? (
          <div className="empty-state-sm">
            <p>No agents assigned to this queue yet.</p>
            <p style={{ fontSize: '0.875rem', marginTop: '0.5rem', color: 'var(--text-subtle)' }}>
              Go to the Agents page to create tokens and register agents for this queue.
            </p>
            <Link to="/agents" className="btn-primary" style={{ marginTop: '1rem', display: 'inline-block' }}>
              Manage Agents
            </Link>
          </div>
        ) : (
          <div className="agents-grid">
            {agents.map((agent) => (
              <AgentTokenCard key={agent.id} agent={agent} />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
