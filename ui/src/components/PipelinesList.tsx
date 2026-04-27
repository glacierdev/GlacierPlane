import { useEffect, useState } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { fetchPipelines } from '../api';
import { PipelineWithStats, BuildSummary } from '../types';

function formatDate(dateString: string): string {
  return new Date(dateString).toLocaleDateString();
}

function formatDuration(seconds: number): string {
  if (seconds < 60) {
    return `${Math.round(seconds)}s`;
  }
  if (seconds < 3600) {
    return `${Math.round(seconds / 60)}m`;
  }
  return `${Math.round(seconds / 3600)}h ${Math.round((seconds % 3600) / 60)}m`;
}

function getReliability(pipeline: PipelineWithStats): number {
  if (pipeline.total_builds === 0) return 0;
  return Math.round((pipeline.passed_builds / pipeline.total_builds) * 100);
}

function getLastBuildStatus(pipeline: PipelineWithStats): { text: string; className: string } {
  if (pipeline.running_builds > 0) {
    return { text: 'Running', className: 'status-running' };
  }
  if (pipeline.recent_builds.length === 0) {
    return { text: 'No builds', className: 'status-idle' };
  }
  const lastBuild = pipeline.recent_builds[0];
  switch (lastBuild.state) {
    case 'passed':
      return { text: 'Passed', className: 'status-connected' };
    case 'failed':
      return { text: 'Failed', className: 'status-disconnected' };
    case 'running':
      return { text: 'Running', className: 'status-running' };
    default:
      return { text: lastBuild.state, className: 'status-idle' };
  }
}

function getBuildStateColor(state: string): string {
  switch (state) {
    case 'passed':
      return 'var(--accent-green)';
    case 'failed':
      return 'var(--accent-red)';
    case 'running':
      return 'var(--accent-blue)';
    default:
      return 'var(--text-subtle)';
  }
}

interface BuildHistoryBarProps {
  builds: BuildSummary[];
}

function BuildHistoryBar({ builds }: BuildHistoryBarProps) {
  const displayBuilds = builds.slice(0, 10).reverse();
  
  if (displayBuilds.length === 0) {
    return (
      <div className="build-history-bar">
        <div className="build-history-empty">No builds yet</div>
      </div>
    );
  }
  
  return (
    <div className="build-history-bar">
      {displayBuilds.reverse().map((build) => (
        <div
          key={build.id}
          className="build-history-item"
          style={{ backgroundColor: getBuildStateColor(build.state) }}
          title={`#${build.number}: ${build.state}`}
        />
      ))}
    </div>
  );
}

export function PipelinesList() {
  const [pipelines, setPipelines] = useState<PipelineWithStats[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const navigate = useNavigate();

  useEffect(() => {
    async function loadPipelines() {
      try {
        const data = await fetchPipelines();
        setPipelines(data);
        setError(null);
      } catch (err) {
        if (err instanceof Error && err.message === 'Not authenticated') {
          navigate('/login');
          return;
        }
        setError(err instanceof Error ? err.message : 'Failed to load pipelines');
      } finally {
        setLoading(false);
      }
    }

    loadPipelines();

    const interval = setInterval(loadPipelines, 10000);
    return () => clearInterval(interval);
  }, [navigate]);

  if (loading) {
    return (
      <div className="loading">
        <div className="spinner"></div>
        Loading pipelines...
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

  return (
    <div>
      <div className="pipelines-header">
        <h1 className="page-title">
          <span className="page-title-icon">📦</span>
          Pipelines
        </h1>
        <Link to="/pipelines/new" className="btn-primary">
          + New Pipeline
        </Link>
      </div>

      {pipelines.length === 0 ? (
        <div className="empty-state">
          <div className="empty-icon">📦</div>
          <p>No pipelines created yet.</p>
          <p style={{ fontSize: '0.875rem', marginTop: '0.5rem' }}>
            Create your first pipeline to get started.
          </p>
          <Link to="/pipelines/new" className="btn-primary" style={{ marginTop: '1rem', display: 'inline-block' }}>
            Create Pipeline
          </Link>
        </div>
      ) : (
        <div className="pipelines-grid">
          {pipelines.map((pipeline) => {
            const status = getLastBuildStatus(pipeline);
            const reliability = getReliability(pipeline);

            return (
              <Link to={`/pipelines/${pipeline.slug}`} className="pipeline-card" key={pipeline.id}>
                <div className="pipeline-header">
                  <div>
                    <div className="pipeline-name">{pipeline.name || pipeline.slug}</div>
                    <div className="pipeline-slug">{pipeline.slug}</div>
                  </div>
                  <span className={`status-badge ${status.className}`}>
                    {status.text}
                  </span>
                </div>

                {pipeline.description && (
                  <div className="pipeline-description">{pipeline.description}</div>
                )}

                <div className="pipeline-branch">
                  <span className="branch-icon">⌥</span>
                  {pipeline.default_branch || 'main'}
                </div>

                {pipeline.queues && pipeline.queues.length > 0 && (
                  <div style={{ 
                    fontSize: '0.75rem', 
                    color: 'var(--text-muted)', 
                    marginBottom: '0.5rem',
                    marginTop: '0.5rem',
                  }}>
                    Queue{pipeline.queues.length > 1 ? 's' : ''}: {pipeline.queues.map(q => q.name).join(', ')}
                  </div>
                )}

                <BuildHistoryBar builds={pipeline.recent_builds} />

                <div className="pipeline-stats">
                  <div className="stat-item">
                    <span className="stat-value">{pipeline.total_builds}</span>
                    <span className="stat-label">Builds</span>
                  </div>
                  <div className="stat-item">
                    <span className="stat-value connected">{reliability}%</span>
                    <span className="stat-label">Reliability</span>
                  </div>
                  <div className="stat-item">
                    <span className="stat-value">{formatDuration(pipeline.avg_duration_seconds)}</span>
                    <span className="stat-label">Avg Speed</span>
                  </div>
                </div>

                <div className="pipeline-meta">
                  <div className="meta-item">
                    <span className="meta-label">Created</span>
                    <span className="meta-value">{formatDate(pipeline.created_at)}</span>
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
