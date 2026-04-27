import { useState, useEffect } from 'react';
import { useParams, useNavigate, Link } from 'react-router-dom';
import { createQueue, updateQueue, fetchQueueDetail, fetchPipelines } from '../api';
import { PipelineWithStats } from '../types';

function generateKey(name: string): string {
  return name
    .toLowerCase()
    .replace(/[^a-z0-9\s-]/g, '')
    .replace(/\s+/g, '-')
    .replace(/-+/g, '-')
    .trim();
}

export function QueueForm() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const isEditMode = Boolean(id);

  const [name, setName] = useState('');
  const [key, setKey] = useState('');
  const [description, setDescription] = useState('');
  const [pipelineId, setPipelineId] = useState<string>('');
  const [pipelines, setPipelines] = useState<PipelineWithStats[]>([]);
  
  const [loading, setLoading] = useState(isEditMode);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [keyManuallyEdited, setKeyManuallyEdited] = useState(false);
  const [isDefault, setIsDefault] = useState(false);

  useEffect(() => {
    fetchPipelines()
      .then(setPipelines)
      .catch(console.error);

    if (!isEditMode) return;

    async function loadQueue() {
      try {
        const data = await fetchQueueDetail(id!);
        setName(data.queue.name);
        setKey(data.queue.key);
        setDescription(data.queue.description || '');
        setPipelineId(data.queue.pipeline_id || '');
        setIsDefault(data.queue.is_default);
        setKeyManuallyEdited(true);
      } catch (err) {
        if (err instanceof Error && err.message === 'Not authenticated') {
          navigate('/login');
          return;
        }
        setError(err instanceof Error ? err.message : 'Failed to load queue');
      } finally {
        setLoading(false);
      }
    }

    loadQueue();
  }, [id, isEditMode, navigate]);

  const handleNameChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const newName = e.target.value;
    setName(newName);
    
    if (!keyManuallyEdited && !isEditMode) {
      setKey(generateKey(newName));
    }
  };

  const handleKeyChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    setKey(e.target.value);
    setKeyManuallyEdited(true);
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);

    if (!name.trim()) {
      setError('Name is required');
      return;
    }
    if (!key.trim()) {
      setError('Key is required');
      return;
    }
    if (!/^[a-z0-9-]+$/.test(key)) {
      setError('Key must contain only lowercase letters, numbers, and hyphens');
      return;
    }

    setSaving(true);

    try {
      if (isEditMode) {
        await updateQueue(id!, {
          name: name.trim(),
          description: description.trim() || undefined,
          pipeline_id: pipelineId || null,
        });
        navigate(`/queues/${id}`);
      } else {
        const queue = await createQueue({
          name: name.trim(),
          key: key.trim(),
          description: description.trim() || undefined,
          pipeline_id: pipelineId || undefined,
        });
        navigate(`/queues/${queue.id}`);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to save queue');
    } finally {
      setSaving(false);
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

  return (
    <div className="pipeline-form-container">
      <Link to={isEditMode ? `/queues/${id}` : '/queues'} className="back-link">
        ← {isEditMode ? 'Back to Queue' : 'Back to Queues'}
      </Link>

      <div className="form-card">
        <h1 className="form-title">
          {isEditMode ? 'Edit Queue' : 'Create Queue'}
        </h1>

        <form onSubmit={handleSubmit} className="pipeline-form">
          {error && (
            <div className="auth-error">{error}</div>
          )}

          <div className="form-field">
            <label className="form-label" htmlFor="name">
              Name *
            </label>
            <input
              type="text"
              id="name"
              className="form-input"
              value={name}
              onChange={handleNameChange}
              placeholder="Linux Agents"
              required
            />
          </div>

          <div className="form-field">
            <label className="form-label" htmlFor="key">
              Key *
              {isEditMode && (
                <span className="form-hint">(cannot be changed)</span>
              )}
            </label>
            <input
              type="text"
              id="key"
              className="form-input form-input-mono"
              value={key}
              onChange={handleKeyChange}
              placeholder="linux-agents"
              pattern="[a-z0-9-]+"
              title="Only lowercase letters, numbers, and hyphens"
              required
              disabled={isEditMode}
            />
            <span className="form-help">
              Used in pipeline config: <code>agents: {'{'} queue: "{key || 'your-key'}" {'}'}</code>
            </span>
          </div>

          <div className="form-field">
            <label className="form-label" htmlFor="description">
              Description
            </label>
            <textarea
              id="description"
              className="form-input form-textarea"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder="Optional description for this queue"
              rows={3}
            />
          </div>

          <div className="form-field">
            <label className="form-label" htmlFor="pipeline">
              Link to Pipeline (Optional)
            </label>
            <select
              id="pipeline"
              className="form-input"
              value={pipelineId}
              onChange={(e) => setPipelineId(e.target.value)}
              disabled={isDefault}
            >
              <option value="">No pipeline (standalone queue)</option>
              {pipelines.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name} ({p.slug})
                </option>
              ))}
            </select>
            {isDefault && (
              <span className="form-help" style={{ color: 'var(--accent-yellow)' }}>
                Default queues cannot change their pipeline association
              </span>
            )}
            <span className="form-help">
              Linked queues become the default target for pipeline jobs
            </span>
          </div>

          <div className="form-actions">
            <Link 
              to={isEditMode ? `/queues/${id}` : '/queues'} 
              className="btn-secondary"
            >
              Cancel
            </Link>
            <button 
              type="submit" 
              className="btn-primary"
              disabled={saving}
            >
              {saving ? 'Saving...' : (isEditMode ? 'Save Changes' : 'Create Queue')}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
