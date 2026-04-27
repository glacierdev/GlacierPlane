import { useEffect, useState, useMemo } from 'react';
import { useParams, Link } from 'react-router-dom';
import { fetchTokenDetails } from '../api';
import { TokenDetailResponse, Agent, JobWithLogs } from '../types';
import { AnsiUp } from 'ansi_up';

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

function getJobStatusClass(state: string, exitStatus: string | null): string {
  if (state === 'finished' && exitStatus === '0') return 'job-status-finished';
  if (state === 'failed' || (exitStatus && exitStatus !== '0')) return 'job-status-failed';
  if (state === 'running') return 'job-status-running';
  return 'job-status-scheduled';
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

function AgentCard({ agent }: { agent: Agent }) {
  const lastSeen = formatTimeAgo(agent.last_seen);
  const statusClass = getAgentStatusClass(agent.connection_state, agent.current_job_id);
  const displayStatus = agent.current_job_id ? 'Running Job' : agent.connection_state;

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

function stripTimestamps(text: string): string {
  let result = text;
  result = result.replace(/\x1b_bk;t=\d+\x07/g, '');
  result = result.replace(/\d{4}-\d{2}-\d{2}\s+\d{2}:\d{2}:\d{2}\s+/g, '');
  return result;
}

function JobCard({ jobWithLogs }: { jobWithLogs: JobWithLogs }) {
  const [expanded, setExpanded] = useState(false);
  const { job, logs } = jobWithLogs;
  const statusClass = getJobStatusClass(job.state, job.exit_status);

  const logsHtml = useMemo(() => {
    if (!logs || logs.length === 0) return '';
    const cleanedLogs = stripTimestamps(logs);
    const ansiUp = new AnsiUp();
    ansiUp.use_classes = true;
    return ansiUp.ansi_to_html(cleanedLogs);
  }, [logs]);

  return (
    <div className="job-card">
      <div className="job-header" onClick={() => setExpanded(!expanded)}>
        <div className="job-info">
          <span className={`job-status-badge ${statusClass}`}>
            {job.state}{job.exit_status ? ` (${job.exit_status})` : ''}
          </span>
          <div>
            <div className="job-label">{job.label || 'Unnamed Step'}</div>
            <div className="job-id">{job.id}</div>
          </div>
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: '1rem' }}>
          {job.agent_name && (
            <span className="job-agent-name">by {job.agent_name}</span>
          )}
          <div className="job-times">
            {job.finished_at && (
              <span>{formatDate(job.finished_at)}</span>
            )}
          </div>
          <span className={`expand-icon ${expanded ? 'expanded' : ''}`}>▼</span>
        </div>
      </div>
      
      {expanded && (
        <div className="logs-container">
          <div className="logs-header">
            <span className="logs-label">Job Output</span>
            <span style={{ fontSize: '0.75rem', color: 'var(--text-subtle)' }}>
              {logs.length > 0 ? `${logs.length} bytes` : 'No output'}
            </span>
          </div>
          <div className="logs-content">
            {logs.length > 0 ? (
              <pre 
                className="logs-pre"
                dangerouslySetInnerHTML={{ __html: logsHtml }}
              />
            ) : (
              <p className="logs-empty">No log output available for this job.</p>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

export function TokenDetail() {
  const { tokenId } = useParams<{ tokenId: string }>();
  const [data, setData] = useState<TokenDetailResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    async function loadTokenDetails() {
      if (!tokenId) return;
      
      try {
        const response = await fetchTokenDetails(tokenId);
        setData(response);
        setError(null);
      } catch (err) {
        setError(err instanceof Error ? err.message : 'Failed to load token details');
      } finally {
        setLoading(false);
      }
    }

    loadTokenDetails();

    const interval = setInterval(loadTokenDetails, 10000);
    return () => clearInterval(interval);
  }, [tokenId]);

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
        <Link to="/" className="back-link">← Back to Tokens</Link>
        <div className="error">
          <p>⚠️ {error || 'Token not found'}</p>
        </div>
      </div>
    );
  }

  const { token, agents, jobs } = data;

  return (
    <div>
      <Link to="/" className="back-link">← Back to Tokens</Link>

      <div className="token-detail-header">
        <div className="token-detail-title">
          <div>
            <div className="token-detail-name">{token.name || 'Unnamed Token'}</div>
            <div className="token-preview">{token.token_preview}</div>
          </div>
          <div className="token-detail-stats">
            <div className="stat-box">
              <span className="stat-box-value">{token.agents_count}</span>
              <span className="stat-box-label">Total Agents</span>
            </div>
            <div className="stat-box connected">
              <span className="stat-box-value">{token.connected_count}</span>
              <span className="stat-box-label">Connected</span>
            </div>
            <div className="stat-box running">
              <span className="stat-box-value">{token.running_count}</span>
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
          </div>
        ) : (
          <div className="agents-instances-grid">
            {agents.map((agent) => (
              <AgentCard key={agent.id} agent={agent} />
            ))}
          </div>
        )}
      </div>

      <div className="section">
        <h2 className="section-title">
          Recent Jobs
          <span className="section-count">{jobs.length}</span>
        </h2>

        {jobs.length === 0 ? (
          <div className="empty-state-sm">
            <p>No jobs have been run by agents using this token.</p>
          </div>
        ) : (
          jobs.map((jobWithLogs) => (
            <JobCard key={jobWithLogs.job.id} jobWithLogs={jobWithLogs} />
          ))
        )}
      </div>
    </div>
  );
}

