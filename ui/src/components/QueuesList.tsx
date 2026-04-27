import { useState, useEffect } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { fetchQueues } from '../api';
import { QueueWithStats } from '../types';

function formatDate(dateString: string): string {
  const date = new Date(dateString);
  return date.toLocaleDateString('en-US', {
    month: 'short',
    day: 'numeric',
    year: 'numeric',
  });
}

function getQueueStatusInfo(queue: QueueWithStats): { text: string; className: string } {
  if (queue.running_count > 0) {
    return { text: `${queue.running_count} running`, className: 'status-running' };
  }
  if (queue.connected_count > 0) {
    return { text: `${queue.connected_count} connected`, className: 'status-connected' };
  }
  if (queue.agents_count > 0) {
    return { text: 'Idle', className: 'status-idle' };
  }
  return { text: 'No agents', className: 'status-disconnected' };
}

export function QueuesList() {
  const [queues, setQueues] = useState<QueueWithStats[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const navigate = useNavigate();

  const loadQueues = async () => {
    try {
      const data = await fetchQueues();
      setQueues(data);
      setError(null);
    } catch (err) {
      if (err instanceof Error && err.message === 'Not authenticated') {
        navigate('/login');
        return;
      }
      setError(err instanceof Error ? err.message : 'Failed to load queues');
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadQueues();
    const interval = setInterval(loadQueues, 10000);
    return () => clearInterval(interval);
  }, [navigate]);

  if (loading) {
    return (
      <div className="loading">
        <div className="spinner"></div>
        Loading queues...
      </div>
    );
  }

  if (error) {
    return (
      <div className="error">
        <p>Error: {error}</p>
      </div>
    );
  }

  const totalAgents = queues.reduce((sum, q) => sum + q.agents_count, 0);
  const totalConnected = queues.reduce((sum, q) => sum + q.connected_count, 0);
  const totalRunning = queues.reduce((sum, q) => sum + q.running_count, 0);

  return (
    <div>
      <div className="pipelines-header">
        <h1 className="page-title" style={{ marginBottom: 0 }}>
          <span className="page-title-icon">🔗</span>
          Queues
        </h1>
        <Link to="/queues/new" className="btn-primary">
          + New Queue
        </Link>
      </div>

      <div className="stats-grid" style={{ marginBottom: '1.5rem' }}>
        <div className="stat-box">
          <div className="stat-box-value">{queues.length}</div>
          <div className="stat-box-label">Queues</div>
        </div>
        <div className="stat-box">
          <div className="stat-box-value">{totalAgents}</div>
          <div className="stat-box-label">Total Agents</div>
        </div>
        <div className="stat-box">
          <div className="stat-box-value connected">{totalConnected}</div>
          <div className="stat-box-label">Connected</div>
        </div>
        <div className="stat-box">
          <div className="stat-box-value running">{totalRunning}</div>
          <div className="stat-box-label">Running</div>
        </div>
      </div>

      {queues.length === 0 ? (
        <div className="empty-state">
          <div className="empty-icon">🔗</div>
          <p>No queues yet.</p>
          <p style={{ fontSize: '0.875rem', marginTop: '0.5rem' }}>
            Queues help organize your agents. Create a queue to get started.
          </p>
          <Link to="/queues/new" className="btn-primary" style={{ marginTop: '1rem', display: 'inline-block' }}>
            Create Queue
          </Link>
        </div>
      ) : (
        <div className="pipelines-grid">
          {queues.map((queue) => {
            const status = getQueueStatusInfo(queue);
            return (
              <Link key={queue.id} to={`/queues/${queue.id}`} className="pipeline-card">
                <div className="pipeline-header">
                  <div>
                    <div className="pipeline-name">{queue.name}</div>
                    <div className="pipeline-slug">{queue.key}</div>
                  </div>
                  <span className={`status-badge ${status.className}`}>
                    {status.text}
                  </span>
                </div>

                {queue.description && (
                  <div className="pipeline-description">{queue.description}</div>
                )}

                {queue.pipeline_name && (
                  <div style={{ 
                    fontSize: '0.75rem', 
                    color: 'var(--text-muted)', 
                    marginBottom: '0.5rem',
                  }}>
                    Pipeline: <strong>{queue.pipeline_name}</strong>
                  </div>
                )}

                {queue.is_default && (
                  <div style={{ 
                    fontSize: '0.75rem', 
                    color: 'var(--accent-teal)', 
                    marginBottom: '0.75rem',
                    display: 'flex',
                    alignItems: 'center',
                    gap: '0.25rem'
                  }}>
                    <span>★</span> Default queue
                  </div>
                )}

                <div className="pipeline-stats">
                  <div className="stat-item">
                    <span className="stat-value">{queue.agents_count}</span>
                    <span className="stat-label">Agents</span>
                  </div>
                  <div className="stat-item">
                    <span className="stat-value connected">{queue.connected_count}</span>
                    <span className="stat-label">Connected</span>
                  </div>
                  <div className="stat-item">
                    <span className="stat-value running">{queue.running_count}</span>
                    <span className="stat-label">Running</span>
                  </div>
                </div>

                <div className="pipeline-meta">
                  <div className="meta-item">
                    <span className="meta-label">Created</span>
                    <span className="meta-value">{formatDate(queue.created_at)}</span>
                  </div>
                </div>
              </Link>
            );
          })}
        </div>
      )}
    </div>
  );
}
