import { 
  AgentToken, 
  TokenDetailResponse, 
  User, 
  LoginResponse, 
  RegisterResponse,
  Pipeline,
  PipelineWithStats,
  PipelineDetailResponse,
  PipelineCreateData,
  PipelineUpdateData,
  BuildWithJobs,
  BuildCreateData,
  BuildFilterOptions,
  JobLogResponse,
  QueueWithStats,
  QueueDetailResponse,
  QueueCreateData,
  QueueUpdateData,
  Agent,
  AgentTokenCreateData,
  AgentTokenDetailResponse,
  Organization,
  OrganizationDetailResponse,
  OrganizationCreateData,
  OrganizationInvitation,
} from './types';

const controlPlaneUrl = import.meta.env.VITE_CONTROL_PLANE_URL;

if (!controlPlaneUrl) {
  throw new Error('VITE_CONTROL_PLANE_URL is required');
}

const API_BASE = controlPlaneUrl.replace(/\/+$/, '');

export function getAuthToken(): string | null {
  return localStorage.getItem('auth_token');
}

export function setAuthToken(token: string): void {
  localStorage.setItem('auth_token', token);
}

export function clearAuthToken(): void {
  localStorage.removeItem('auth_token');
}

export function getSelectedOrgId(): string | null {
  return localStorage.getItem('selected_org_id');
}

export function setSelectedOrgId(orgId: string): void {
  localStorage.setItem('selected_org_id', orgId);
}

export function getSelectedOrgSlug(): string | null {
  return localStorage.getItem('selected_org_slug');
}

export function setSelectedOrgSlug(slug: string): void {
  localStorage.setItem('selected_org_slug', slug);
}

export function clearSelectedOrgId(): void {
  localStorage.removeItem('selected_org_id');
  localStorage.removeItem('selected_org_slug');
  localStorage.removeItem('selected_org_role');
}

export function getSelectedOrgRole(): string | null {
  return localStorage.getItem('selected_org_role');
}

export function setSelectedOrgRole(role: string): void {
  localStorage.setItem('selected_org_role', role);
}

export function isAdminOrOwnerRole(): boolean {
  const role = getSelectedOrgRole();
  return role === 'owner' || role === 'admin';
}

function getAuthHeaders(): HeadersInit {
  const token = getAuthToken();
  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
  };
  if (token) {
    headers['Authorization'] = `Bearer ${token}`;
  }
  return headers;
}

function getOrgSlugForApi(): string {
  const slug = getSelectedOrgSlug();
  if (!slug) throw new Error('No organization selected');
  return slug;
}

export async function fetchTokens(): Promise<AgentToken[]> {
  const response = await fetch(`${API_BASE}/api/admin/tokens`, {
    headers: getAuthHeaders(),
  });
  if (!response.ok) {
    throw new Error(`Failed to fetch tokens: ${response.statusText}`);
  }
  return response.json();
}

export async function fetchTokenDetails(tokenId: string): Promise<TokenDetailResponse> {
  const response = await fetch(`${API_BASE}/api/admin/tokens/${tokenId}`, {
    headers: getAuthHeaders(),
  });
  if (!response.ok) {
    throw new Error(`Failed to fetch token details: ${response.statusText}`);
  }
  return response.json();
}

export async function login(email: string, password: string): Promise<LoginResponse> {
  const response = await fetch(`${API_BASE}/api/auth/login`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ email, password }),
  });

  if (!response.ok) {
    const errorText = await response.text();
    try {
      const errorJson = JSON.parse(errorText);
      throw new Error(errorJson.message || errorJson.error || errorText);
    } catch {
      throw new Error(errorText || `Login failed (${response.status})`);
    }
  }

  const data = await response.json();
  setAuthToken(data.token);
  return data;
}

export async function register(email: string, name: string, password: string): Promise<RegisterResponse> {
  const response = await fetch(`${API_BASE}/api/auth/register`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ email, name, password }),
  });

  if (!response.ok) {
    const errorText = await response.text();
    try {
      const errorJson = JSON.parse(errorText);
      throw new Error(errorJson.message || errorJson.error || errorText);
    } catch {
      throw new Error(errorText || `Registration failed (${response.status})`);
    }
  }

  const data = await response.json();
  setAuthToken(data.token);
  return data;
}

export async function getCurrentUser(): Promise<User> {
  const token = getAuthToken();
  if (!token) {
    throw new Error('Not authenticated');
  }

  const response = await fetch(`${API_BASE}/api/auth/me`, {
    headers: getAuthHeaders(),
  });

  if (!response.ok) {
    if (response.status === 401) {
      clearAuthToken();
    }
    throw new Error('Failed to get current user');
  }

  return response.json();
}

export async function logout(): Promise<void> {
  const token = getAuthToken();
  if (token) {
    try {
      await fetch(`${API_BASE}/api/auth/logout`, {
        method: 'POST',
        headers: getAuthHeaders(),
      });
    } catch {
      /* noop */
    }
  }
  clearAuthToken();
}

export async function fetchPipelines(): Promise<PipelineWithStats[]> {
  const orgSlug = getOrgSlugForApi();
  const response = await fetch(`${API_BASE}/api/v2/organizations/${orgSlug}/pipelines?per_page=100`, {
    headers: getAuthHeaders(),
  });
  
  if (!response.ok) {
    if (response.status === 401) {
      clearAuthToken();
      throw new Error('Not authenticated');
    }
    throw new Error(`Failed to fetch pipelines: ${response.statusText}`);
  }
  
  return response.json();
}

export async function fetchPipelineDetail(pipelineSlug: string): Promise<PipelineDetailResponse> {
  const orgSlug = getOrgSlugForApi();
  const response = await fetch(`${API_BASE}/api/v2/organizations/${orgSlug}/pipelines/${pipelineSlug}`, {
    headers: getAuthHeaders(),
  });
  
  if (!response.ok) {
    if (response.status === 401) {
      clearAuthToken();
      throw new Error('Not authenticated');
    }
    if (response.status === 404) {
      throw new Error('Pipeline not found');
    }
    throw new Error(`Failed to fetch pipeline: ${response.statusText}`);
  }
  
  return response.json();
}

export async function createPipeline(data: PipelineCreateData): Promise<Pipeline> {
  const orgSlug = getOrgSlugForApi();
  const response = await fetch(`${API_BASE}/api/v2/organizations/${orgSlug}/pipelines`, {
    method: 'POST',
    headers: getAuthHeaders(),
    body: JSON.stringify(data),
  });
  
  if (!response.ok) {
    if (response.status === 401) {
      clearAuthToken();
      throw new Error('Not authenticated');
    }
    if (response.status === 409) {
      throw new Error('A pipeline with this slug already exists');
    }
    
    const errorText = await response.text();
    try {
      const errorJson = JSON.parse(errorText);
      throw new Error(errorJson.message || errorJson.error || errorText);
    } catch {
      throw new Error(errorText || `Failed to create pipeline (${response.status})`);
    }
  }
  
  return response.json();
}

export async function updatePipeline(pipelineSlug: string, data: PipelineUpdateData): Promise<Pipeline> {
  const orgSlug = getOrgSlugForApi();
  const response = await fetch(`${API_BASE}/api/v2/organizations/${orgSlug}/pipelines/${pipelineSlug}`, {
    method: 'PATCH',
    headers: getAuthHeaders(),
    body: JSON.stringify(data),
  });
  
  if (!response.ok) {
    if (response.status === 401) {
      clearAuthToken();
      throw new Error('Not authenticated');
    }
    if (response.status === 404) {
      throw new Error('Pipeline not found');
    }
    
    const errorText = await response.text();
    try {
      const errorJson = JSON.parse(errorText);
      throw new Error(errorJson.message || errorJson.error || errorText);
    } catch {
      throw new Error(errorText || `Failed to update pipeline (${response.status})`);
    }
  }
  
  return response.json();
}

export async function deletePipeline(pipelineSlug: string): Promise<void> {
  const orgSlug = getOrgSlugForApi();
  const response = await fetch(`${API_BASE}/api/v2/organizations/${orgSlug}/pipelines/${pipelineSlug}`, {
    method: 'DELETE',
    headers: getAuthHeaders(),
  });
  
  if (!response.ok) {
    if (response.status === 401) {
      clearAuthToken();
      throw new Error('Not authenticated');
    }
    if (response.status === 404) {
      throw new Error('Pipeline not found');
    }
    throw new Error(`Failed to delete pipeline: ${response.statusText}`);
  }
}

export async function fetchPipelineBuilds(
  pipelineSlug: string,
  filters?: BuildFilterOptions,
  perPage?: number,
): Promise<BuildWithJobs[]> {
  const orgSlug = getOrgSlugForApi();
  const params = new URLSearchParams();
  params.set('per_page', (perPage ?? 100).toString());

  if (filters) {
    if (filters.state) params.set('state', filters.state);
    if (filters.branch) params.set('branch', filters.branch);
    if (filters.commit) params.set('commit', filters.commit);
    if (filters.created_from) params.set('created_from', filters.created_from);
    if (filters.created_to) params.set('created_to', filters.created_to);
    if (filters.finished_from) params.set('finished_from', filters.finished_from);
    if (filters.creator) params.set('creator', filters.creator);
  }

  const url = `${API_BASE}/api/v2/organizations/${orgSlug}/pipelines/${pipelineSlug}/builds?${params.toString()}`;
  const response = await fetch(url, {
    headers: getAuthHeaders(),
  });
  
  if (!response.ok) {
    if (response.status === 401) {
      clearAuthToken();
      throw new Error('Not authenticated');
    }
    if (response.status === 404) {
      throw new Error('Pipeline not found');
    }
    throw new Error(`Failed to fetch builds: ${response.statusText}`);
  }
  
  return response.json();
}

export async function createBuild(pipelineSlug: string, data: BuildCreateData): Promise<BuildWithJobs> {
  const orgSlug = getOrgSlugForApi();
  const response = await fetch(
    `${API_BASE}/api/v2/organizations/${orgSlug}/pipelines/${pipelineSlug}/builds`,
    {
      method: 'POST',
      headers: getAuthHeaders(),
      body: JSON.stringify(data),
    },
  );

  if (!response.ok) {
    if (response.status === 401) {
      clearAuthToken();
      throw new Error('Not authenticated');
    }
    if (response.status === 404) {
      throw new Error('Pipeline not found');
    }

    const errorText = await response.text();
    try {
      const errorJson = JSON.parse(errorText);
      throw new Error(errorJson.message || errorJson.error || errorText);
    } catch {
      throw new Error(errorText || `Failed to create build (${response.status})`);
    }
  }

  return response.json();
}

export async function fetchJobLog(pipelineSlug: string, buildNumber: number, jobId: string): Promise<JobLogResponse> {
  const orgSlug = getOrgSlugForApi();
  const response = await fetch(
    `${API_BASE}/api/v2/organizations/${orgSlug}/pipelines/${pipelineSlug}/builds/${buildNumber}/jobs/${jobId}/log`,
    { headers: getAuthHeaders() },
  );

  if (!response.ok) {
    if (response.status === 401) {
      clearAuthToken();
      throw new Error('Not authenticated');
    }
    if (response.status === 404) {
      throw new Error('Job log not found');
    }
    throw new Error(`Failed to fetch job log: ${response.statusText}`);
  }

  return response.json();
}

export async function fetchQueues(): Promise<QueueWithStats[]> {
  const orgSlug = getOrgSlugForApi();
  const response = await fetch(`${API_BASE}/api/v2/organizations/${orgSlug}/queues?per_page=100`, {
    headers: getAuthHeaders(),
  });
  
  if (!response.ok) {
    if (response.status === 401) {
      clearAuthToken();
      throw new Error('Not authenticated');
    }
    throw new Error(`Failed to fetch queues: ${response.statusText}`);
  }
  
  return response.json();
}

export async function fetchQueueDetail(queueId: string): Promise<QueueDetailResponse> {
  const orgSlug = getOrgSlugForApi();
  const response = await fetch(`${API_BASE}/api/v2/organizations/${orgSlug}/queues/${queueId}`, {
    headers: getAuthHeaders(),
  });
  
  if (!response.ok) {
    if (response.status === 401) {
      clearAuthToken();
      throw new Error('Not authenticated');
    }
    if (response.status === 404) {
      throw new Error('Queue not found');
    }
    throw new Error(`Failed to fetch queue: ${response.statusText}`);
  }
  
  return response.json();
}

export async function createQueue(data: QueueCreateData): Promise<QueueWithStats> {
  const orgSlug = getOrgSlugForApi();
  const response = await fetch(`${API_BASE}/api/v2/organizations/${orgSlug}/queues`, {
    method: 'POST',
    headers: getAuthHeaders(),
    body: JSON.stringify(data),
  });
  
  if (!response.ok) {
    if (response.status === 401) {
      clearAuthToken();
      throw new Error('Not authenticated');
    }
    if (response.status === 409) {
      throw new Error('A queue with this key already exists');
    }
    
    const errorText = await response.text();
    try {
      const errorJson = JSON.parse(errorText);
      throw new Error(errorJson.message || errorJson.error || errorText);
    } catch {
      throw new Error(errorText || `Failed to create queue (${response.status})`);
    }
  }
  
  return response.json();
}

export async function updateQueue(queueId: string, data: QueueUpdateData): Promise<QueueWithStats> {
  const orgSlug = getOrgSlugForApi();
  const response = await fetch(`${API_BASE}/api/v2/organizations/${orgSlug}/queues/${queueId}`, {
    method: 'PUT',
    headers: getAuthHeaders(),
    body: JSON.stringify(data),
  });
  
  if (!response.ok) {
    if (response.status === 401) {
      clearAuthToken();
      throw new Error('Not authenticated');
    }
    if (response.status === 404) {
      throw new Error('Queue not found');
    }
    
    const errorText = await response.text();
    try {
      const errorJson = JSON.parse(errorText);
      throw new Error(errorJson.message || errorJson.error || errorText);
    } catch {
      throw new Error(errorText || `Failed to update queue (${response.status})`);
    }
  }
  
  return response.json();
}

export async function deleteQueue(queueId: string): Promise<void> {
  const orgSlug = getOrgSlugForApi();
  const response = await fetch(`${API_BASE}/api/v2/organizations/${orgSlug}/queues/${queueId}`, {
    method: 'DELETE',
    headers: getAuthHeaders(),
  });
  
  if (!response.ok) {
    if (response.status === 401) {
      clearAuthToken();
      throw new Error('Not authenticated');
    }
    if (response.status === 404) {
      throw new Error('Queue not found');
    }
    if (response.status === 400) {
      throw new Error('Cannot delete default queue');
    }
    throw new Error(`Failed to delete queue: ${response.statusText}`);
  }
}

export async function fetchUserAgentTokens(): Promise<AgentToken[]> {
  const orgSlug = getOrgSlugForApi();
  const response = await fetch(`${API_BASE}/api/v2/organizations/${orgSlug}/agent-tokens?per_page=100`, {
    headers: getAuthHeaders(),
  });
  
  if (!response.ok) {
    if (response.status === 401) {
      clearAuthToken();
      throw new Error('Not authenticated');
    }
    throw new Error(`Failed to fetch agent tokens: ${response.statusText}`);
  }
  
  return response.json();
}

export async function fetchUserAgentTokenDetail(tokenId: string): Promise<AgentTokenDetailResponse> {
  const orgSlug = getOrgSlugForApi();
  const response = await fetch(`${API_BASE}/api/v2/organizations/${orgSlug}/agent-tokens/${tokenId}`, {
    headers: getAuthHeaders(),
  });
  
  if (!response.ok) {
    if (response.status === 401) {
      clearAuthToken();
      throw new Error('Not authenticated');
    }
    if (response.status === 404) {
      throw new Error('Token not found');
    }
    throw new Error(`Failed to fetch token: ${response.statusText}`);
  }
  
  return response.json();
}

export async function createAgentToken(data: AgentTokenCreateData): Promise<AgentToken> {
  const orgSlug = getOrgSlugForApi();
  const response = await fetch(`${API_BASE}/api/v2/organizations/${orgSlug}/agent-tokens`, {
    method: 'POST',
    headers: getAuthHeaders(),
    body: JSON.stringify(data),
  });
  
  if (!response.ok) {
    if (response.status === 401) {
      clearAuthToken();
      throw new Error('Not authenticated');
    }
    
    const errorText = await response.text();
    try {
      const errorJson = JSON.parse(errorText);
      throw new Error(errorJson.message || errorJson.error || errorText);
    } catch {
      throw new Error(errorText || `Failed to create token (${response.status})`);
    }
  }
  
  return response.json();
}

export async function deleteAgentToken(tokenId: string): Promise<void> {
  const orgSlug = getOrgSlugForApi();
  const response = await fetch(`${API_BASE}/api/v2/organizations/${orgSlug}/agent-tokens/${tokenId}`, {
    method: 'DELETE',
    headers: getAuthHeaders(),
  });
  
  if (!response.ok) {
    if (response.status === 401) {
      clearAuthToken();
      throw new Error('Not authenticated');
    }
    if (response.status === 404) {
      throw new Error('Token not found');
    }
    throw new Error(`Failed to delete token: ${response.statusText}`);
  }
}

export async function fetchUserAgents(): Promise<Agent[]> {
  const orgSlug = getOrgSlugForApi();
  const response = await fetch(`${API_BASE}/api/v2/organizations/${orgSlug}/agents?per_page=100`, {
    headers: getAuthHeaders(),
  });
  
  if (!response.ok) {
    if (response.status === 401) {
      clearAuthToken();
      throw new Error('Not authenticated');
    }
    throw new Error(`Failed to fetch agents: ${response.statusText}`);
  }
  
  return response.json();
}

export async function fetchOrganizations(): Promise<Organization[]> {
  const response = await fetch(`${API_BASE}/api/v2/organizations?per_page=100`, {
    headers: getAuthHeaders(),
  });
  
  if (!response.ok) {
    if (response.status === 401) {
      clearAuthToken();
      throw new Error('Not authenticated');
    }
    throw new Error(`Failed to fetch organizations: ${response.statusText}`);
  }
  
  return response.json();
}

export async function createOrganization(data: OrganizationCreateData): Promise<Organization> {
  const response = await fetch(`${API_BASE}/api/v2/organizations`, {
    method: 'POST',
    headers: getAuthHeaders(),
    body: JSON.stringify(data),
  });
  
  if (!response.ok) {
    if (response.status === 401) {
      clearAuthToken();
      throw new Error('Not authenticated');
    }
    if (response.status === 409) {
      throw new Error('An organization with this slug already exists');
    }
    const errorText = await response.text();
    try {
      const errorJson = JSON.parse(errorText);
      throw new Error(errorJson.message || errorJson.error || errorText);
    } catch {
      throw new Error(errorText || `Failed to create organization (${response.status})`);
    }
  }
  
  return response.json();
}

export async function fetchOrganizationDetail(orgSlug: string): Promise<OrganizationDetailResponse> {
  const response = await fetch(`${API_BASE}/api/v2/organizations/${orgSlug}`, {
    headers: getAuthHeaders(),
  });
  
  if (!response.ok) {
    if (response.status === 401) {
      clearAuthToken();
      throw new Error('Not authenticated');
    }
    throw new Error(`Failed to fetch organization: ${response.statusText}`);
  }
  
  return response.json();
}

export async function createOrganizationInvitation(orgSlug: string): Promise<OrganizationInvitation> {
  const response = await fetch(`${API_BASE}/api/v2/organizations/${orgSlug}/invitations`, {
    method: 'POST',
    headers: getAuthHeaders(),
  });
  
  if (!response.ok) {
    if (response.status === 401) {
      clearAuthToken();
      throw new Error('Not authenticated');
    }
    const errorText = await response.text();
    try {
      const errorJson = JSON.parse(errorText);
      throw new Error(errorJson.message || errorJson.error || errorText);
    } catch {
      throw new Error(errorText || `Failed to create invitation (${response.status})`);
    }
  }
  
  return response.json();
}

export async function joinOrganization(token: string): Promise<Organization> {
  const response = await fetch(`${API_BASE}/api/v2/organizations/join/${token}`, {
    method: 'POST',
    headers: getAuthHeaders(),
  });
  
  if (!response.ok) {
    if (response.status === 401) {
      clearAuthToken();
      throw new Error('Not authenticated');
    }
    const errorText = await response.text();
    try {
      const errorJson = JSON.parse(errorText);
      throw new Error(errorJson.message || errorJson.error || errorText);
    } catch {
      throw new Error(errorText || `Failed to join organization (${response.status})`);
    }
  }
  
  return response.json();
}

export async function updateMemberRole(orgSlug: string, userId: string, role: string): Promise<void> {
  const response = await fetch(`${API_BASE}/api/v2/organizations/${orgSlug}/members/${userId}`, {
    method: 'PUT',
    headers: getAuthHeaders(),
    body: JSON.stringify({ role }),
  });
  
  if (!response.ok) {
    const errorText = await response.text();
    try {
      const errorJson = JSON.parse(errorText);
      throw new Error(errorJson.message || errorJson.error || errorText);
    } catch {
      throw new Error(errorText || `Failed to update role (${response.status})`);
    }
  }
}

export async function removeMember(orgSlug: string, userId: string): Promise<void> {
  const response = await fetch(`${API_BASE}/api/v2/organizations/${orgSlug}/members/${userId}`, {
    method: 'DELETE',
    headers: getAuthHeaders(),
  });
  
  if (!response.ok) {
    const errorText = await response.text();
    try {
      const errorJson = JSON.parse(errorText);
      throw new Error(errorJson.message || errorJson.error || errorText);
    } catch {
      throw new Error(errorText || `Failed to remove member (${response.status})`);
    }
  }
}
