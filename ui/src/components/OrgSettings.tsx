import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  fetchOrganizationDetail,
  createOrganizationInvitation,
  updateMemberRole,
  removeMember,
  getSelectedOrgSlug,
} from '../api';
import {
  OrganizationDetailResponse,
  OrganizationMember,
  OrganizationInvitation,
} from '../types';

function formatTimeAgo(dateString: string): string {
  const date = new Date(dateString);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffDay = Math.floor(diffMs / (1000 * 60 * 60 * 24));
  if (diffDay < 1) return 'Today';
  if (diffDay === 1) return 'Yesterday';
  return `${diffDay}d ago`;
}

interface MemberRowProps {
  member: OrganizationMember;
  currentUserRole: string;
  onUpdateRole: (userId: string, role: string) => void;
  onRemove: (userId: string) => void;
}

function MemberRow({ member, currentUserRole, onUpdateRole, onRemove }: MemberRowProps) {
  const isOwner = member.role === 'owner';
  const canModify = (currentUserRole === 'owner' || currentUserRole === 'admin') && !isOwner;
  const canPromote = currentUserRole === 'owner' && !isOwner;

  return (
    <div className="member-row">
      <div className="member-info">
        <div className="member-avatar">
          {member.name.charAt(0).toUpperCase()}
        </div>
        <div>
          <div className="member-name">{member.name}</div>
          <div className="member-email">{member.email}</div>
        </div>
      </div>
      <div className="member-actions">
        <span className={`role-badge role-${member.role}`}>
          {member.role}
        </span>
        {canPromote && member.role === 'member' && (
          <button
            className="btn-small btn-secondary"
            onClick={() => onUpdateRole(member.user_id, 'admin')}
          >
            Promote to Admin
          </button>
        )}
        {canPromote && member.role === 'admin' && (
          <button
            className="btn-small btn-secondary"
            onClick={() => onUpdateRole(member.user_id, 'member')}
          >
            Restrict to Member
          </button>
        )}
        {canModify && (
          <button
            className="btn-small btn-danger"
            onClick={() => onRemove(member.user_id)}
          >
            Remove
          </button>
        )}
      </div>
    </div>
  );
}

interface InvitationRowProps {
  invitation: OrganizationInvitation;
}

function InvitationRow({ invitation }: InvitationRowProps) {
  const [copied, setCopied] = useState(false);
  const isExpired = new Date(invitation.expires_at) < new Date();
  const baseUrl = window.location.origin;
  const fullUrl = `${baseUrl}/join/${invitation.token}`;

  const copyLink = async () => {
    try {
      await navigator.clipboard.writeText(fullUrl);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      const textArea = document.createElement('textarea');
      textArea.value = fullUrl;
      document.body.appendChild(textArea);
      textArea.select();
      document.execCommand('copy');
      document.body.removeChild(textArea);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  };

  return (
    <div className="invitation-row">
      <div className="invitation-info">
        <code className="invitation-link">{fullUrl}</code>
        <div className="invitation-meta">
          <span>Created {formatTimeAgo(invitation.created_at)}</span>
          {invitation.used && <span className="status-badge status-used">Used</span>}
          {isExpired && !invitation.used && <span className="status-badge status-expired">Expired</span>}
          {!isExpired && !invitation.used && <span className="status-badge status-active">Active</span>}
        </div>
      </div>
      {!invitation.used && !isExpired && (
        <button className="btn-small btn-secondary" onClick={copyLink}>
          {copied ? 'Copied!' : 'Copy Link'}
        </button>
      )}
    </div>
  );
}

export function OrgSettings() {
  const [data, setData] = useState<OrganizationDetailResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [creatingInvite, setCreatingInvite] = useState(false);
  const navigate = useNavigate();

  const orgSlug = getSelectedOrgSlug();

  const loadData = async () => {
    if (!orgSlug) {
      setError('No organization selected');
      setLoading(false);
      return;
    }
    try {
      const detail = await fetchOrganizationDetail(orgSlug);
      setData(detail);
      setError(null);
    } catch (err) {
      if (err instanceof Error && err.message === 'Not authenticated') {
        navigate('/login');
        return;
      }
      setError(err instanceof Error ? err.message : 'Failed to load organization');
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadData();
  }, [orgSlug]);

  const handleCreateInvitation = async () => {
    if (!orgSlug) return;
    setCreatingInvite(true);
    try {
      await createOrganizationInvitation(orgSlug);
      await loadData();
    } catch (err) {
      alert(err instanceof Error ? err.message : 'Failed to create invitation');
    } finally {
      setCreatingInvite(false);
    }
  };

  const handleUpdateRole = async (userId: string, role: string) => {
    if (!orgSlug) return;
    try {
      await updateMemberRole(orgSlug, userId, role);
      await loadData();
    } catch (err) {
      alert(err instanceof Error ? err.message : 'Failed to update role');
    }
  };

  const handleRemoveMember = async (userId: string) => {
    if (!orgSlug) return;
    if (!confirm('Remove this member from the organization?')) return;
    try {
      await removeMember(orgSlug, userId);
      await loadData();
    } catch (err) {
      alert(err instanceof Error ? err.message : 'Failed to remove member');
    }
  };

  if (loading) {
    return (
      <div className="loading">
        <div className="spinner"></div>
        Loading settings...
      </div>
    );
  }

  if (error || !data) {
    return (
      <div className="error">
        <p>Warning: {error || 'Failed to load organization'}</p>
      </div>
    );
  }

  const currentUserRole = data.organization.role;

  return (
    <div>
      <div className="pipelines-header">
        <h1 className="page-title" style={{ marginBottom: 0 }}>
          <span className="page-title-icon">
          {data.organization ? data.organization.name.charAt(0).toUpperCase() : '?'}
          </span>
          {data.organization.name}
        </h1>
      </div>

      <div className="settings-section">
        <div className="settings-section-header">
          <h2>Members ({data.members.length})</h2>
        </div>
        <div className="members-list">
          {data.members.map((member) => (
            <MemberRow
              key={member.id}
              member={member}
              currentUserRole={currentUserRole}
              onUpdateRole={handleUpdateRole}
              onRemove={handleRemoveMember}
            />
          ))}
        </div>
      </div>

      {(currentUserRole === 'owner' || currentUserRole === 'admin') && (
        <div className="settings-section">
          <div className="settings-section-header">
            <h2>Invitation Links</h2>
            <button
              className="btn-primary"
              onClick={handleCreateInvitation}
              disabled={creatingInvite}
            >
              {creatingInvite ? 'Creating...' : '+ Generate Invite Link'}
            </button>
          </div>
          {data.invitations.length === 0 ? (
            <div className="empty-state" style={{ padding: '2rem' }}>
              <p>No invitation links yet. Generate one to invite members.</p>
            </div>
          ) : (
            <div className="invitations-list">
              {data.invitations.map((inv) => (
                <InvitationRow key={inv.id} invitation={inv} />
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
