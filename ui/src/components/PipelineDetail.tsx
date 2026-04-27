import { useEffect, useState, useMemo, useCallback, useRef } from 'react';
import { useParams, Link, useNavigate } from 'react-router-dom';
import { fetchPipelineDetail, deletePipeline, fetchJobLog, fetchPipelineBuilds, createBuild } from '../api';
import { PipelineDetailResponse, BuildWithJobs, BuildFilterOptions, BuildCreateData, JobSummary } from '../types';
import { AnsiUp } from 'ansi_up';

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

function formatDuration(startedAt: string | null, finishedAt: string | null): string {
  if (!startedAt || !finishedAt) return '-';
  const start = new Date(startedAt).getTime();
  const end = new Date(finishedAt).getTime();
  const diffSecs = Math.floor((end - start) / 1000);
  
  if (diffSecs < 60) return `${diffSecs}s`;
  if (diffSecs < 3600) return `${Math.floor(diffSecs / 60)}m ${diffSecs % 60}s`;
  return `${Math.floor(diffSecs / 3600)}h ${Math.floor((diffSecs % 3600) / 60)}m`;
}

function getBuildStatusClass(state: string): string {
  switch (state) {
    case 'passed': return 'status-connected';
    case 'failed': return 'status-disconnected';
    case 'running': return 'status-running';
    default: return 'status-idle';
  }
}

function getJobStatusClass(state: string, exitStatus: string | null): string {
  if (state === 'finished' && exitStatus === '0') return 'job-status-finished';
  if (state === 'failed' || (exitStatus && exitStatus !== '0')) return 'job-status-failed';
  if (state === 'running') return 'job-status-running';
  return 'job-status-scheduled';
}

function stripTimestamps(text: string): string {
  let result = text;
  result = result.replace(/\x1b_bk;t=\d+\x07/g, '');
  result = result.replace(/\d{4}-\d{2}-\d{2}\s+\d{2}:\d{2}:\d{2}\s+/g, '');
  return result;
}

type JobLogsState = {
  loading: boolean;
  loaded: boolean;
  logs: string;
  error: string | null;
};

type BuildLogsState = {
  loading: boolean;
  loaded: boolean;
  error: string | null;
};

const EMPTY_JOB_LOGS: JobLogsState = {
  loading: false,
  loaded: false,
  logs: '',
  error: null,
};

function JobRow({
  job,
  logsState,
}: {
  job: JobSummary;
  logsState: JobLogsState;
}) {
  const [expanded, setExpanded] = useState(false);
  const statusClass = getJobStatusClass(job.state, job.exit_status);

  const logsHtml = useMemo(() => {
    if (!logsState.logs) return '';
    const ansiUp = new AnsiUp();
    ansiUp.use_classes = true;
    return ansiUp.ansi_to_html(stripTimestamps(logsState.logs));
  }, [logsState.logs]);

  return (
    <div className="job-card" style={{ marginBottom: '0.75rem' }}>
      <div className="job-header" onClick={() => setExpanded(!expanded)}>
        <div className="job-info">
          <span className={`job-status-badge-sm ${statusClass}`}>
            {job.state}{job.exit_status ? ` (${job.exit_status})` : ''}
          </span>
          <div>
            <div className="job-label">{job.label || 'Unnamed Step'}</div>
            <div className="job-id">{job.id}</div>
          </div>
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: '0.75rem' }}>
          <span className="job-row-duration">
            {formatDuration(job.started_at, job.finished_at)}
          </span>
          <span className={`expand-icon ${expanded ? 'expanded' : ''}`}>▼</span>
        </div>
      </div>

      {expanded && (
        <div className="logs-container">
          <div className="logs-header">
            <span className="logs-label">Log</span>
            <span style={{ fontSize: '0.75rem', color: 'var(--text-subtle)' }}>
              {logsState.loaded && logsState.logs.length > 0 ? `${logsState.logs.length} bytes` : ''}
            </span>
          </div>
          <div className="logs-content">
            {logsState.loading ? (
              <p className="logs-empty">Loading logs...</p>
            ) : logsState.error ? (
              <p className="logs-empty">{logsState.error}</p>
            ) : logsState.loaded && logsState.logs.length > 0 ? (
              <pre className="logs-pre" dangerouslySetInnerHTML={{ __html: logsHtml }} />
            ) : logsState.loaded ? (
              <p className="logs-empty">No log output available for this job.</p>
            ) : (
              <p className="logs-empty">Expand the build to load logs.</p>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

function BuildCard({
  build,
  jobLogsById,
  buildLogsState,
  onExpandBuild,
}: {
  build: BuildWithJobs;
  jobLogsById: Record<string, JobLogsState>;
  buildLogsState: BuildLogsState | undefined;
  onExpandBuild: (build: BuildWithJobs) => void;
}) {
  const [expanded, setExpanded] = useState(false);
  const statusClass = getBuildStatusClass(build.state);
  const timeAgo = formatTimeAgo(build.created_at);
  const duration = formatDuration(build.started_at, build.finished_at);

  const handleToggle = () => {
    const nextExpanded = !expanded;
    setExpanded(nextExpanded);
    if (nextExpanded) {
      onExpandBuild(build);
    }
  };

  return (
    <div className="build-card">
      <div className="build-header" onClick={handleToggle}>
        <div className="build-info">
          <span className={`status-badge ${statusClass}`}>
            {build.state}
          </span>
          <div className="build-details">
            <div className="build-number">#{build.number}</div>
            <div className="build-commit">
              <span className="commit-sha">{build.commit.substring(0, 7)}</span>
              <span className="build-message">{build.message || 'No message'}</span>
            </div>
          </div>
        </div>
        <div className="build-meta-right">
          {build.source && build.source !== 'webhook' && (
            <span className="build-source-badge">{build.source}</span>
          )}
          <div className="build-branch">
            <span className="branch-icon">⌥</span>
            {build.branch}
          </div>
          <div className="build-timing">
            <span className={timeAgo.className}>{timeAgo.text}</span>
            {duration !== '-' && <span className="build-duration">{duration}</span>}
          </div>
          {build.author_name && (
            <span className="build-author">{build.author_name}</span>
          )}
          <span className={`expand-icon ${expanded ? 'expanded' : ''}`}>▼</span>
        </div>
      </div>
      
      {expanded && build.jobs.length > 0 && (
        <div className="build-jobs">
          <div className="build-jobs-header">
            <span>Jobs ({build.jobs.length})</span>
            {buildLogsState?.loading && (
              <span style={{ fontSize: '0.75rem', color: 'var(--text-subtle)' }}>Loading logs...</span>
            )}
            {buildLogsState?.error && (
              <span style={{ fontSize: '0.75rem', color: 'var(--accent-red)' }}>{buildLogsState.error}</span>
            )}
          </div>
          <div className="build-jobs-list">
            {build.jobs.map((job) => (
              <JobRow
                key={job.id}
                job={job}
                logsState={jobLogsById[job.id] ?? EMPTY_JOB_LOGS}
              />
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

const BUILD_STATES = ['', 'running', 'scheduled', 'passed', 'failed', 'canceled', 'finished'] as const;

export function PipelineDetail() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const [data, setData] = useState<PipelineDetailResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [deleting, setDeleting] = useState(false);
  const [jobLogsById, setJobLogsById] = useState<Record<string, JobLogsState>>({});
  const [buildLogsState, setBuildLogsState] = useState<Record<string, BuildLogsState>>({});
  const fetchedBuildsRef = useRef<Set<string>>(new Set());

  const [filterState, setFilterState] = useState('');
  const [filterBranch, setFilterBranch] = useState('');
  const [filterCreator, setFilterCreator] = useState('');
  const [filteredBuilds, setFilteredBuilds] = useState<BuildWithJobs[] | null>(null);
  const [filterLoading, setFilterLoading] = useState(false);

  const [showNewBuildModal, setShowNewBuildModal] = useState(false);
  const [newBuildCommit, setNewBuildCommit] = useState('HEAD');
  const [newBuildBranch, setNewBuildBranch] = useState('');
  const [newBuildMessage, setNewBuildMessage] = useState('');
  const [creatingBuild, setCreatingBuild] = useState(false);
  const [createBuildError, setCreateBuildError] = useState<string | null>(null);

  const hasActiveFilters = filterState !== '' || filterBranch !== '' || filterCreator !== '';

  useEffect(() => {
    setJobLogsById({});
    setBuildLogsState({});
    fetchedBuildsRef.current = new Set();
    setFilterState('');
    setFilterBranch('');
    setFilterCreator('');
    setFilteredBuilds(null);
  }, [id]);

  useEffect(() => {
    async function loadPipelineDetails() {
      if (!id) return;
      
      try {
        const response = await fetchPipelineDetail(id);
        setData(response);
        setError(null);
      } catch (err) {
        if (err instanceof Error && err.message === 'Not authenticated') {
          navigate('/login');
          return;
        }
        setError(err instanceof Error ? err.message : 'Failed to load pipeline details');
      } finally {
        setLoading(false);
      }
    }

    loadPipelineDetails();
    
    const interval = setInterval(loadPipelineDetails, 10000);
    return () => clearInterval(interval);
  }, [id, navigate]);

  const applyFilters = useCallback(async () => {
    if (!id) return;
    if (!hasActiveFilters) {
      setFilteredBuilds(null);
      return;
    }

    setFilterLoading(true);
    try {
      const filters: BuildFilterOptions = {};
      if (filterState) filters.state = filterState;
      if (filterBranch) filters.branch = filterBranch;
      if (filterCreator) filters.creator = filterCreator;
      const builds = await fetchPipelineBuilds(id, filters);
      setFilteredBuilds(builds);
    } catch (err) {
      if (err instanceof Error && err.message === 'Not authenticated') {
        navigate('/login');
        return;
      }
    } finally {
      setFilterLoading(false);
    }
  }, [id, filterState, filterBranch, filterCreator, hasActiveFilters, navigate]);

  const clearFilters = useCallback(() => {
    setFilterState('');
    setFilterBranch('');
    setFilterCreator('');
    setFilteredBuilds(null);
  }, []);

  const handleDelete = async () => {
    if (!id || !data) return;
    
    const confirmed = window.confirm(
      `Are you sure you want to delete the pipeline "${data.pipeline.name}"? This action cannot be undone.`
    );
    
    if (!confirmed) return;
    
    setDeleting(true);
    try {
      await deletePipeline(id);
      navigate('/pipelines');
    } catch (err) {
      alert(err instanceof Error ? err.message : 'Failed to delete pipeline');
    } finally {
      setDeleting(false);
    }
  };

  const loadBuildLogs = useCallback(async (build: BuildWithJobs) => {
    if (!id) return;
    if (fetchedBuildsRef.current.has(build.id)) return;
    fetchedBuildsRef.current.add(build.id);

    setBuildLogsState((prev) => ({
      ...prev,
      [build.id]: { loading: true, loaded: false, error: null },
    }));

    try {
      const results = await Promise.all(
        build.jobs.map(async (job) => {
          try {
            const logResponse = await fetchJobLog(id, build.number, job.id);
            return { jobId: job.id, logs: logResponse.content, error: null };
          } catch (err) {
            return { jobId: job.id, logs: '', error: err instanceof Error ? err.message : 'Failed to load log' };
          }
        })
      );

      const newJobLogs: Record<string, JobLogsState> = {};
      for (const result of results) {
        newJobLogs[result.jobId] = {
          loading: false,
          loaded: true,
          logs: result.logs,
          error: result.error,
        };
      }

      setJobLogsById((prev) => ({ ...prev, ...newJobLogs }));
      setBuildLogsState((prev) => ({
        ...prev,
        [build.id]: { loading: false, loaded: true, error: null },
      }));
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : 'Failed to load build logs';
      fetchedBuildsRef.current.delete(build.id);
      setBuildLogsState((prev) => ({
        ...prev,
        [build.id]: { loading: false, loaded: false, error: errorMsg },
      }));
    }
  }, [id]);

  const handleOpenNewBuild = useCallback(() => {
    setNewBuildBranch(data?.pipeline.default_branch || 'main');
    setNewBuildCommit('HEAD');
    setNewBuildMessage('');
    setCreateBuildError(null);
    setShowNewBuildModal(true);
  }, [data]);

  const handleCreateBuild = useCallback(async () => {
    if (!id) return;
    if (!newBuildCommit.trim() || !newBuildBranch.trim()) {
      setCreateBuildError('Commit and branch are required');
      return;
    }

    setCreatingBuild(true);
    setCreateBuildError(null);

    try {
      const payload: BuildCreateData = {
        commit: newBuildCommit.trim(),
        branch: newBuildBranch.trim(),
      };
      if (newBuildMessage.trim()) {
        payload.message = newBuildMessage.trim();
      }

      await createBuild(id, payload);
      setShowNewBuildModal(false);

      const response = await fetchPipelineDetail(id);
      setData(response);
    } catch (err) {
      if (err instanceof Error && err.message === 'Not authenticated') {
        navigate('/login');
        return;
      }
      setCreateBuildError(err instanceof Error ? err.message : 'Failed to create build');
    } finally {
      setCreatingBuild(false);
    }
  }, [id, newBuildCommit, newBuildBranch, newBuildMessage, navigate]);

  if (loading) {
    return (
      <div className="loading">
        <div className="spinner"></div>
        Loading pipeline details...
      </div>
    );
  }

  if (error || !data) {
    return (
      <div>
        <Link to="/pipelines" className="back-link">← Back to Pipelines</Link>
        <div className="error">
          <p>⚠️ {error || 'Pipeline not found'}</p>
        </div>
      </div>
    );
  }

  const { pipeline, stats, builds } = data;
  const displayBuilds = filteredBuilds !== null ? filteredBuilds : builds;
  const reliability = stats.total_builds > 0 
    ? Math.round((stats.passed_builds / stats.total_builds) * 100) 
    : 0;

  return (
    <div>
      <Link to="/pipelines" className="back-link">← Back to Pipelines</Link>

      <div className="pipeline-detail-header">
        <div className="pipeline-detail-title">
          <div>
            <div className="pipeline-detail-name">{pipeline.name}</div>
            <div className="pipeline-slug">{pipeline.slug}</div>
          </div>
          <div className="pipeline-actions">
            <button onClick={handleOpenNewBuild} className="btn-primary">
              New Build
            </button>
            <Link to={`/pipelines/${id}/edit`} className="btn-secondary">
              Edit
            </Link>
            <button 
              onClick={handleDelete} 
              className="btn-danger"
              disabled={deleting}
            >
              {deleting ? 'Deleting...' : 'Delete'}
            </button>
          </div>
        </div>

        {pipeline.description && (
          <div className="pipeline-detail-description">{pipeline.description}</div>
        )}

        <div className="pipeline-detail-info">
          <div className="pipeline-repo">
            <span className="repo-icon">📁</span>
            <a href={pipeline.repository_url} target="_blank" rel="noopener noreferrer">
              {pipeline.repository_url}
            </a>
          </div>
          <div className="pipeline-branch">
            <span className="branch-icon">⌥</span>
            {pipeline.default_branch || 'main'}
          </div>
        </div>

        <div className="pipeline-detail-stats">
          <div className="stat-box">
            <span className="stat-box-value">{stats.total_builds}</span>
            <span className="stat-box-label">Total Builds</span>
          </div>
          <div className="stat-box connected">
            <span className="stat-box-value">{stats.passed_builds}</span>
            <span className="stat-box-label">Passed</span>
          </div>
          <div className="stat-box" style={{ '--stat-color': 'var(--accent-red)' } as React.CSSProperties}>
            <span className="stat-box-value" style={{ color: 'var(--accent-red)' }}>{stats.failed_builds}</span>
            <span className="stat-box-label">Failed</span>
          </div>
          <div className="stat-box running">
            <span className="stat-box-value">{stats.running_builds}</span>
            <span className="stat-box-label">Running</span>
          </div>
          <div className="stat-box">
            <span className="stat-box-value">{reliability}%</span>
            <span className="stat-box-label">Reliability</span>
          </div>
        </div>
      </div>

      <div className="section">
        <h2 className="section-title">
          Builds
          <span className="section-count">{displayBuilds.length}</span>
        </h2>

        <div className="build-filters">
          <div className="build-filters-row">
            <select
              className="filter-select"
              value={filterState}
              onChange={(e) => setFilterState(e.target.value)}
            >
              <option value="">All states</option>
              {BUILD_STATES.filter(s => s !== '').map((s) => (
                <option key={s} value={s}>{s.charAt(0).toUpperCase() + s.slice(1)}</option>
              ))}
            </select>

            <input
              className="filter-input"
              type="text"
              placeholder="Branch (* for wildcard)"
              value={filterBranch}
              onChange={(e) => setFilterBranch(e.target.value)}
              onKeyDown={(e) => { if (e.key === 'Enter') applyFilters(); }}
            />

            <input
              className="filter-input"
              type="text"
              placeholder="Author"
              value={filterCreator}
              onChange={(e) => setFilterCreator(e.target.value)}
              onKeyDown={(e) => { if (e.key === 'Enter') applyFilters(); }}
            />

            <button className="btn-filter" onClick={applyFilters} disabled={filterLoading}>
              {filterLoading ? 'Filtering...' : 'Filter'}
            </button>

            {hasActiveFilters && (
              <button className="btn-filter-clear" onClick={clearFilters}>
                Clear
              </button>
            )}
          </div>
        </div>

        {displayBuilds.length === 0 ? (
          <div className="empty-state-sm">
            <p>{hasActiveFilters ? 'No builds match the current filters.' : 'No builds yet. Push to your repository to trigger a build.'}</p>
          </div>
        ) : (
          <div className="builds-list">
            {displayBuilds.map((build) => (
              <BuildCard
                key={build.id}
                build={build}
                jobLogsById={jobLogsById}
                buildLogsState={buildLogsState[build.id]}
                onExpandBuild={loadBuildLogs}
              />
            ))}
          </div>
        )}
      </div>

      {showNewBuildModal && (
        <div className="modal-overlay" onClick={() => setShowNewBuildModal(false)}>
          <div className="modal-content" onClick={(e) => e.stopPropagation()}>
            <h3 className="modal-title">New Build</h3>
            <p className="modal-subtitle">
              Trigger a new build for <strong>{pipeline.name}</strong>
            </p>

            {createBuildError && (
              <div className="modal-error">{createBuildError}</div>
            )}

            <div className="form-group">
              <label className="form-label">Commit</label>
              <input
                className="form-input"
                type="text"
                value={newBuildCommit}
                onChange={(e) => setNewBuildCommit(e.target.value)}
                placeholder="HEAD or full commit SHA"
              />
            </div>

            <div className="form-group">
              <label className="form-label">Branch</label>
              <input
                className="form-input"
                type="text"
                value={newBuildBranch}
                onChange={(e) => setNewBuildBranch(e.target.value)}
                placeholder="main"
              />
            </div>

            <div className="form-group">
              <label className="form-label">Message (optional)</label>
              <input
                className="form-input"
                type="text"
                value={newBuildMessage}
                onChange={(e) => setNewBuildMessage(e.target.value)}
                placeholder="Build triggered via API"
              />
            </div>

            <div className="modal-actions">
              <button
                className="btn-secondary"
                onClick={() => setShowNewBuildModal(false)}
                disabled={creatingBuild}
              >
                Cancel
              </button>
              <button
                className="btn-primary"
                onClick={handleCreateBuild}
                disabled={creatingBuild}
              >
                {creatingBuild ? 'Creating...' : 'Create Build'}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
