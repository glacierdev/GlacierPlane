import { useEffect, useState } from 'react';
import { useParams, useNavigate, Link } from 'react-router-dom';
import { createPipeline, updatePipeline, fetchPipelineDetail } from '../api';

function generateSlug(name: string): string {
  return name
    .toLowerCase()
    .replace(/[^a-z0-9\s-]/g, '')
    .replace(/\s+/g, '-')
    .replace(/-+/g, '-')
    .trim();
}

export function PipelineForm() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const isEditMode = Boolean(id);

  const [name, setName] = useState('');
  const [slug, setSlug] = useState('');
  const [repositoryUrl, setRepositoryUrl] = useState('');
  const [description, setDescription] = useState('');
  const [defaultBranch, setDefaultBranch] = useState('main');
  
  const [loading, setLoading] = useState(isEditMode);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [slugManuallyEdited, setSlugManuallyEdited] = useState(false);

  useEffect(() => {
    if (!isEditMode) return;

    async function loadPipeline() {
      try {
        const data = await fetchPipelineDetail(id!);
        setName(data.pipeline.name);
        setSlug(data.pipeline.slug);
        setRepositoryUrl(data.pipeline.repository_url);
        setDescription(data.pipeline.description || '');
        setDefaultBranch(data.pipeline.default_branch || 'main');
        setSlugManuallyEdited(true);
      } catch (err) {
        if (err instanceof Error && err.message === 'Not authenticated') {
          navigate('/login');
          return;
        }
        setError(err instanceof Error ? err.message : 'Failed to load pipeline');
      } finally {
        setLoading(false);
      }
    }

    loadPipeline();
  }, [id, isEditMode, navigate]);

  const handleNameChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const newName = e.target.value;
    setName(newName);

    if (!slugManuallyEdited && !isEditMode) {
      setSlug(generateSlug(newName));
    }
  };

  const handleSlugChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    setSlug(e.target.value);
    setSlugManuallyEdited(true);
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);

    if (!name.trim()) {
      setError('Name is required');
      return;
    }
    if (!slug.trim()) {
      setError('Slug is required');
      return;
    }
    if (!/^[a-z0-9-]+$/.test(slug)) {
      setError('Slug must contain only lowercase letters, numbers, and hyphens');
      return;
    }
    if (!repositoryUrl.trim()) {
      setError('Repository URL is required');
      return;
    }

    setSaving(true);

    try {
      if (isEditMode) {
        await updatePipeline(id!, {
          name: name.trim(),
          description: description.trim() || undefined,
          repository_url: repositoryUrl.trim(),
          default_branch: defaultBranch.trim() || undefined,
        });
        navigate(`/pipelines/${id}`);
      } else {
        const pipeline = await createPipeline({
          name: name.trim(),
          slug: slug.trim(),
          repository_url: repositoryUrl.trim(),
          description: description.trim() || undefined,
          default_branch: defaultBranch.trim() || undefined,
        });
        navigate(`/pipelines/${pipeline.slug}`);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to save pipeline');
    } finally {
      setSaving(false);
    }
  };

  if (loading) {
    return (
      <div className="loading">
        <div className="spinner"></div>
        Loading pipeline...
      </div>
    );
  }

  return (
    <div className="pipeline-form-container">
      <Link to={isEditMode ? `/pipelines/${id}` : '/pipelines'} className="back-link">
        ← {isEditMode ? 'Back to Pipeline' : 'Back to Pipelines'}
      </Link>

      <div className="form-card">
        <h1 className="form-title">
          {isEditMode ? 'Edit Pipeline' : 'Create Pipeline'}
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
              placeholder="My Pipeline"
              required
            />
          </div>

          <div className="form-field">
            <label className="form-label" htmlFor="slug">
              Slug *
              {isEditMode && (
                <span className="form-hint">(cannot be changed)</span>
              )}
            </label>
            <input
              type="text"
              id="slug"
              className="form-input form-input-mono"
              value={slug}
              onChange={handleSlugChange}
              placeholder="my-pipeline"
              pattern="[a-z0-9-]+"
              title="Only lowercase letters, numbers, and hyphens"
              required
              disabled={isEditMode}
            />
            <span className="form-help">
              Used in URLs and webhook configurations
            </span>
          </div>

          <div className="form-field">
            <label className="form-label" htmlFor="repository_url">
              Repository URL *
            </label>
            <input
              type="text"
              id="repository_url"
              className="form-input form-input-mono"
              value={repositoryUrl}
              onChange={(e) => setRepositoryUrl(e.target.value)}
              placeholder="git@github.com:owner/repo.git"
              required
            />
            <span className="form-help">
              SSH (git@...) or HTTPS URL for the repository
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
              placeholder="Optional description for this pipeline"
              rows={3}
            />
          </div>

          <div className="form-field">
            <label className="form-label" htmlFor="default_branch">
              Default Branch
            </label>
            <input
              type="text"
              id="default_branch"
              className="form-input"
              value={defaultBranch}
              onChange={(e) => setDefaultBranch(e.target.value)}
              placeholder="main"
            />
          </div>

          <div className="form-actions">
            <Link 
              to={isEditMode ? `/pipelines/${id}` : '/pipelines'} 
              className="btn-secondary"
            >
              Cancel
            </Link>
            <button 
              type="submit" 
              className="btn-primary"
              disabled={saving}
            >
              {saving ? 'Saving...' : (isEditMode ? 'Save Changes' : 'Create Pipeline')}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
