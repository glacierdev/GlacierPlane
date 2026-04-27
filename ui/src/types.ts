export interface AgentToken {
  id: string;
  name: string | null;
  description: string | null;
  token_preview: string;
  token?: string | null;
  expires_at: string | null;
  created_at: string;
  agents_count: number;
  connected_count: number;
  running_count: number;
}

export interface Agent {
  id: string;
  uuid: string;
  name: string;
  hostname: string;
  os: string;
  arch: string;
  version: string;
  connection_state: string;
  user_agent: string;
  meta_data: string[] | null;
  priority: number | null;
  last_seen: string | null;
  last_heartbeat: string | null;
  current_job_id: string | null;
  created_at: string;
  queue_id: string | null;
  queue_name: string | null;
}

export interface Job {
  id: string;
  build_id: string;
  agent_id: string | null;
  agent_name: string | null;
  state: string;
  exit_status: string | null;
  started_at: string | null;
  finished_at: string | null;
  created_at: string;
  label: string | null;
}

export interface JobWithLogs {
  job: Job;
  logs: string;
}

export interface TokenDetailResponse {
  token: AgentToken;
  agents: Agent[];
  jobs: JobWithLogs[];
}

export interface User {
  id: string;
  email: string;
  name: string;
  created_at: string;
}

export interface LoginResponse {
  user: User;
  token: string;
}

export interface RegisterResponse {
  user: User;
  token: string;
}

export interface Pipeline {
  id: string;
  slug: string;
  name: string;
  description: string | null;
  repository_url: string;
  default_branch: string | null;
  created_at: string;
  updated_at: string;
}

export interface PipelineStats {
  total_builds: number;
  passed_builds: number;
  failed_builds: number;
  running_builds: number;
  avg_duration_seconds: number;
}

export interface BuildSummary {
  id: string;
  number: number;
  commit: string;
  branch: string;
  message: string | null;
  author_name: string | null;
  state: string;
  source: string;
  created_at: string;
  started_at: string | null;
  finished_at: string | null;
}

export interface PipelineQueueInfo {
  id: string;
  name: string;
  key: string;
}

export interface PipelineWithStats extends Pipeline {
  total_builds: number;
  passed_builds: number;
  failed_builds: number;
  running_builds: number;
  avg_duration_seconds: number;
  recent_builds: BuildSummary[];
  queues: PipelineQueueInfo[];
}

export interface JobSummary {
  id: string;
  state: string;
  exit_status: string | null;
  label: string | null;
  started_at: string | null;
  finished_at: string | null;
}

export interface JobLogResponse {
  content: string;
  size: number;
  header_times: number[];
}

export interface BuildWithJobs {
  id: string;
  number: number;
  pipeline_slug: string;
  commit: string;
  branch: string;
  message: string | null;
  author_name: string | null;
  author_email: string | null;
  state: string;
  source: string;
  created_at: string;
  started_at: string | null;
  finished_at: string | null;
  jobs: JobSummary[];
}

export interface PipelineDetailResponse {
  pipeline: Pipeline;
  stats: PipelineStats;
  builds: BuildWithJobs[];
}

export interface BuildCreateData {
  commit: string;
  branch: string;
  message?: string;
  author?: {
    name?: string;
    email?: string;
  };
  env?: Record<string, string>;
  meta_data?: Record<string, string>;
}

export interface BuildFilterOptions {
  state?: string;
  branch?: string;
  commit?: string;
  created_from?: string;
  created_to?: string;
  finished_from?: string;
  creator?: string;
}

export interface PipelineCreateData {
  name: string;
  slug: string;
  repository_url: string;
  description?: string;
  default_branch?: string;
}

export interface PipelineUpdateData {
  name: string;
  description?: string;
  repository_url: string;
  default_branch?: string;
}

export interface Queue {
  id: string;
  user_id: string;
  pipeline_id: string | null;
  name: string;
  key: string;
  description: string | null;
  is_default: boolean;
  created_at: string;
  updated_at: string;
}

export interface QueueWithStats extends Queue {
  pipeline_name: string | null;
  agents_count: number;
  connected_count: number;
  running_count: number;
}

export interface QueueAgentToken {
  id: string;
  name: string | null;
  description: string | null;
  token_preview: string;
  created_at: string;
  registrations_count: number;
  connected_count: number;
  running_count: number;
}

export interface QueueDetailResponse {
  queue: Queue;
  agents: QueueAgentToken[];
  pipeline_name: string | null;
}

export interface QueueCreateData {
  name: string;
  key: string;
  description?: string;
  pipeline_id?: string;
}

export interface QueueUpdateData {
  name: string;
  description?: string;
  pipeline_id?: string | null;
}

export interface AgentTokenCreateData {
  name: string;
  description?: string;
}

export interface AgentTokenDetailResponse {
  token: AgentToken;
  agents: Agent[];
}

export interface Organization {
  id: string;
  name: string;
  slug: string;
  role: string;
  created_at: string;
  updated_at: string;
}

export interface OrganizationMember {
  id: string;
  user_id: string;
  email: string;
  name: string;
  role: string;
  created_at: string;
}

export interface OrganizationInvitation {
  id: string;
  token: string;
  invite_url: string;
  expires_at: string;
  created_at: string;
  used: boolean;
}

export interface OrganizationDetailResponse {
  organization: Organization;
  members: OrganizationMember[];
  invitations: OrganizationInvitation[];
}

export interface OrganizationCreateData {
  name: string;
  slug: string;
}
