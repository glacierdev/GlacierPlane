import { BrowserRouter, Routes, Route, Link, Navigate, NavLink, useNavigate, useParams } from 'react-router-dom';
import { useState, useEffect, useRef } from 'react';
import { TokensList } from './components/TokensList';
import { TokenDetail } from './components/TokenDetail';
import { PipelinesList } from './components/PipelinesList';
import { PipelineDetail } from './components/PipelineDetail';
import { PipelineForm } from './components/PipelineForm';
import { QueuesList } from './components/QueuesList';
import { QueueDetail } from './components/QueueDetail';
import { QueueForm } from './components/QueueForm';
import { AgentsList } from './components/AgentsList';
import { UserTokenDetail } from './components/UserTokenDetail';
import { OrgSettings } from './components/OrgSettings';
import { Login } from './components/Login';
import { Register } from './components/Register';
import {
  getCurrentUser, logout, getAuthToken,
  fetchOrganizations, createOrganization, joinOrganization,
  getSelectedOrgId, setSelectedOrgId, clearSelectedOrgId,
  setSelectedOrgSlug, setSelectedOrgRole,
} from './api';
import { User, Organization, OrganizationCreateData } from './types';

interface OrgSelectorProps {
  orgs: Organization[];
  selectedOrgId: string | null;
  onSelect: (org: Organization) => void;
  onCreateOrg: () => void;
  loading: boolean;
}

function OrgSelector({ orgs, selectedOrgId, onSelect, onCreateOrg, loading }: OrgSelectorProps) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  const selectedOrg = orgs.find(o => o.id === selectedOrgId);

  return (
    <div className="org-selector" ref={ref}>
      <button className="org-selector-btn" onClick={() => setOpen(!open)}>
        <span className="org-selector-icon">
          {selectedOrg ? selectedOrg.name.charAt(0).toUpperCase() : '?'}
        </span>
        <span className="org-selector-name">
          {loading ? 'Loading...' : selectedOrg ? selectedOrg.name : 'Select Organization'}
        </span>
        <span className="org-selector-arrow">{open ? '\u25B2' : '\u25BC'}</span>
      </button>
      {open && (
        <div className="org-selector-dropdown">
          {orgs.length > 0 && orgs.map((org) => (
            <button
              key={org.id}
              className={`org-selector-item ${org.id === selectedOrgId ? 'active' : ''}`}
              onClick={() => {
                onSelect(org);
                setOpen(false);
              }}
            >
              <span className="org-selector-item-icon">
                {org.name.charAt(0).toUpperCase()}
              </span>
              <div className="org-selector-item-info">
                <span className="org-selector-item-name">{org.name}</span>
                <span className="org-selector-item-role">{org.role}</span>
              </div>
              {org.id === selectedOrgId && <span className="org-selector-check">&#10003;</span>}
            </button>
          ))}
          <div className="org-selector-divider" />
          <button className="org-selector-item org-selector-create" onClick={() => { onCreateOrg(); setOpen(false); }}>
            <span className="org-selector-item-icon">+</span>
            <span className="org-selector-item-name">Create Organization</span>
          </button>
        </div>
      )}
    </div>
  );
}

interface CreateOrgModalProps {
  onClose: () => void;
  onCreated: (org: Organization) => void;
}

function CreateOrgModal({ onClose, onCreated }: CreateOrgModalProps) {
  const [formData, setFormData] = useState<OrganizationCreateData>({ name: '', slug: '' });
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const generateSlug = (name: string) => {
    return name.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '');
  };

  const handleNameChange = (name: string) => {
    setFormData({ name, slug: generateSlug(name) });
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!formData.name.trim() || !formData.slug.trim()) {
      setError('Name and slug are required');
      return;
    }
    setSubmitting(true);
    setError(null);
    try {
      const org = await createOrganization(formData);
      onCreated(org);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to create organization');
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal-content" onClick={(e) => e.stopPropagation()}>
        <h2>Create Organization</h2>
        <p className="modal-description">
          Create a new organization to collaborate with your team.
        </p>
        {error && <div className="form-error">{error}</div>}
        <form onSubmit={handleSubmit}>
          <div className="form-group">
            <label>Organization Name *</label>
            <input
              type="text"
              value={formData.name}
              onChange={(e) => handleNameChange(e.target.value)}
              placeholder="e.g., My Team"
              disabled={submitting}
            />
          </div>
          <div className="form-group">
            <label>Slug *</label>
            <input
              type="text"
              value={formData.slug}
              onChange={(e) => setFormData({ ...formData, slug: e.target.value })}
              placeholder="e.g., my-team"
              disabled={submitting}
            />
          </div>
          <div className="modal-actions">
            <button type="button" className="btn-secondary" onClick={onClose} disabled={submitting}>Cancel</button>
            <button type="submit" className="btn-primary" disabled={submitting}>
              {submitting ? 'Creating...' : 'Create Organization'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}

function JoinOrganization() {
  const { token } = useParams<{ token: string }>();
  const navigate = useNavigate();
  const [status, setStatus] = useState<'loading' | 'success' | 'error'>('loading');
  const [message, setMessage] = useState('');
  const [joinedOrgId, setJoinedOrgId] = useState<string | null>(null);
  const joinAttempted = useRef(false);

  useEffect(() => {
    if (!token || joinAttempted.current) return;
    joinAttempted.current = true;

    joinOrganization(token)
      .then((org) => {
        setStatus('success');
        setMessage(`You have successfully joined "${org.name}"!`);
        setSelectedOrgId(org.id);
        setSelectedOrgSlug(org.slug);
        setJoinedOrgId(org.id);
      })
      .catch((err) => {
        setStatus('error');
        setMessage(err instanceof Error ? err.message : 'Failed to join organization');
      });
  }, [token]);

  const handleOk = () => {
    if (joinedOrgId) {
      setSelectedOrgId(joinedOrgId);
    }
    navigate('/pipelines');
    window.location.reload();
  };

  return (
    <div className="app">
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', height: '100vh', flexDirection: 'column', gap: '1rem' }}>
        {status === 'loading' && (
          <>
            <div className="spinner"></div>
            <span>Joining organization...</span>
          </>
        )}
        {status === 'success' && (
          <>
            <div style={{ fontSize: '3rem', color: 'var(--accent-green)' }}>&#10003;</div>
            <h2>{message}</h2>
            <button
              className="btn-primary"
              style={{ marginTop: '1rem', padding: '0.75rem 2rem', borderRadius: '8px', fontSize: '1rem', cursor: 'pointer' }}
              onClick={handleOk}
            >
              Ok
            </button>
          </>
        )}
        {status === 'error' && (
          <>
            <div style={{ fontSize: '3rem', color: 'var(--accent-red)' }}>&#10007;</div>
            <h2>Failed to join</h2>
            <p>{message}</p>
            <Link to="/pipelines" className="btn-primary" style={{ marginTop: '1rem', textDecoration: 'none', padding: '0.75rem 1.5rem', borderRadius: '8px' }}>Go to Dashboard</Link>
          </>
        )}
      </div>
    </div>
  );
}

function App() {
  const [user, setUser] = useState<User | null>(null);
  const [loading, setLoading] = useState(true);
  const [orgs, setOrgs] = useState<Organization[]>([]);
  const [orgsLoading, setOrgsLoading] = useState(false);
  const [selectedOrgId, setSelectedOrgIdState] = useState<string | null>(getSelectedOrgId());
  const [showCreateOrg, setShowCreateOrg] = useState(false);

  const checkAuth = async () => {
    const token = getAuthToken();
    if (!token) {
      setLoading(false);
      return;
    }
    try {
      const currentUser = await getCurrentUser();
      setUser(currentUser);
    } catch {
      setUser(null);
    } finally {
      setLoading(false);
    }
  };

  const loadOrgs = async () => {
    setOrgsLoading(true);
    try {
      const orgsData = await fetchOrganizations();
      setOrgs(orgsData);
      if (!getSelectedOrgId() && orgsData.length > 0) {
        setSelectedOrgId(orgsData[0].id);
        setSelectedOrgSlug(orgsData[0].slug);
        setSelectedOrgRole(orgsData[0].role);
        setSelectedOrgIdState(orgsData[0].id);
      }
      const currentId = getSelectedOrgId();
      if (currentId && !orgsData.find(o => o.id === currentId)) {
        if (orgsData.length > 0) {
          setSelectedOrgId(orgsData[0].id);
          setSelectedOrgSlug(orgsData[0].slug);
          setSelectedOrgRole(orgsData[0].role);
          setSelectedOrgIdState(orgsData[0].id);
        } else {
          clearSelectedOrgId();
          setSelectedOrgIdState(null);
        }
      }
      const sel = orgsData.find(o => o.id === getSelectedOrgId());
      if (sel) {
        setSelectedOrgRole(sel.role);
      }
    } catch {
      /* noop */
    } finally {
      setOrgsLoading(false);
    }
  };

  useEffect(() => {
    checkAuth();
  }, []);

  useEffect(() => {
    if (user) {
      loadOrgs();
    }
  }, [user]);

  const handleLogin = () => {
    checkAuth();
  };

  const handleLogout = async () => {
    await logout();
    clearSelectedOrgId();
    setSelectedOrgIdState(null);
    setUser(null);
    setOrgs([]);
  };

  const handleSelectOrg = (org: Organization) => {
    setSelectedOrgId(org.id);
    setSelectedOrgSlug(org.slug);
    setSelectedOrgRole(org.role);
    setSelectedOrgIdState(org.id);
  };

  const handleOrgCreated = (org: Organization) => {
    setShowCreateOrg(false);
    setSelectedOrgId(org.id);
    setSelectedOrgSlug(org.slug);
    setSelectedOrgRole(org.role);
    setSelectedOrgIdState(org.id);
    loadOrgs();
  };

  const selectedOrg = orgs.find(o => o.id === selectedOrgId);
  const isAdminOrOwner = selectedOrg && (selectedOrg.role === 'owner' || selectedOrg.role === 'admin');

  if (loading) {
    return (
      <div className="app">
        <div className="loading">
          <div className="spinner"></div>
          <span>Loading...</span>
        </div>
      </div>
    );
  }

  return (
    <BrowserRouter>
      <Routes>
        <Route
          path="/login"
          element={
            user ? <Navigate to="/" replace /> : <Login onLogin={handleLogin} />
          }
        />
        <Route
          path="/register"
          element={
            user ? <Navigate to="/" replace /> : <Register onLogin={handleLogin} />
          }
        />

        <Route
          path="/join/:token"
          element={
            user ? <JoinOrganization /> : <Navigate to="/login" replace />
          }
        />

        <Route
          path="/*"
          element={
            user ? (
              <div className="app">
                <header className="header">
                  <div className="header-content">
                    <OrgSelector
                      orgs={orgs}
                      selectedOrgId={selectedOrgId}
                      onSelect={handleSelectOrg}
                      onCreateOrg={() => setShowCreateOrg(true)}
                      loading={orgsLoading}
                    />
                    <Link to="/pipelines" className="logo">
                      <div className="logo-icon">GG</div>
                      <span className="logo-text">GlacierDev Control Plane</span>
                    </Link>
                    <nav className="header-nav">
                      <NavLink 
                        to="/pipelines" 
                        className={({ isActive }) => `nav-link ${isActive ? 'active' : ''}`}
                      >
                        Pipelines
                      </NavLink>
                      <NavLink 
                        to="/queues" 
                        className={({ isActive }) => `nav-link ${isActive ? 'active' : ''}`}
                      >
                        Queues
                      </NavLink>
                      <NavLink 
                        to="/agents" 
                        className={({ isActive }) => `nav-link ${isActive ? 'active' : ''}`}
                      >
                        Agents
                      </NavLink>
                      {isAdminOrOwner && (
                        <NavLink 
                          to="/settings" 
                          className={({ isActive }) => `nav-link ${isActive ? 'active' : ''}`}
                        >
                          Settings
                        </NavLink>
                      )}
                    </nav>
                    <div className="header-user">
                      <span className="user-name">{user.name}</span>
                      <button onClick={handleLogout} className="logout-btn">
                        Logout
                      </button>
                    </div>
                  </div>
                </header>

                <main className="main-content">
                  {!selectedOrgId ? (
                    <div className="no-org-prompt">
                      <div className="no-org-icon">🏢</div>
                      <h2>No Organization Selected</h2>
                      <p>You need to create or join an organization to manage pipelines, queues, and agents.</p>
                      {orgs.length > 0 ? (
                        <p className="no-org-hint">Select an organization from the dropdown above to get started.</p>
                      ) : (
                        <button className="btn-primary" onClick={() => setShowCreateOrg(true)}>
                          Create Organization
                        </button>
                      )}
                    </div>
                  ) : (
                  <Routes>
                    <Route path="/" element={<Navigate to="/pipelines" replace />} />

                    <Route path="/pipelines" element={<PipelinesList />} />
                    <Route path="/pipelines/new" element={<PipelineForm />} />
                    <Route path="/pipelines/:id" element={<PipelineDetail />} />
                    <Route path="/pipelines/:id/edit" element={<PipelineForm />} />

                    <Route path="/queues" element={<QueuesList />} />
                    <Route path="/queues/new" element={<QueueForm />} />
                    <Route path="/queues/:id" element={<QueueDetail />} />
                    <Route path="/queues/:id/edit" element={<QueueForm />} />

                    <Route path="/agents" element={<AgentsList />} />
                    <Route path="/agents/tokens/:tokenId" element={<UserTokenDetail />} />

                    <Route path="/settings" element={
                      isAdminOrOwner ? <OrgSettings /> : <Navigate to="/pipelines" replace />
                    } />

                    <Route
                      path="/tokens"
                      element={
                        <>
                          <h1 className="page-title">
                            <span className="page-title-icon">All Agent Tokens (Admin)</span>
                          </h1>
                          <TokensList />
                        </>
                      }
                    />
                    <Route path="/tokens/:tokenId" element={<TokenDetail />} />
                  </Routes>
                  )}
                </main>
              </div>
            ) : (
              <Navigate to="/login" replace />
            )
          }
        />
      </Routes>

      {showCreateOrg && (
        <CreateOrgModal
          onClose={() => setShowCreateOrg(false)}
          onCreated={handleOrgCreated}
        />
      )}
    </BrowserRouter>
  );
}

export default App;
