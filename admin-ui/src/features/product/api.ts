/// Queries and mutations behind the user-facing pages.

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import { API_BASE, AdminApiError, adminJsonResponse, apiJson } from '../../admin-ui-core';
import type { AgentsPayload, ConnectorsPayload, InstallReport, SkillLibrary, SkillReloadResult, UpdateCheck } from './model';

const keys = {
  connectors: ['product', 'connectors'] as const,
  agents: ['product', 'agents'] as const,
  skills: ['product', 'skills'] as const,
  update: (version: string) => ['product', 'update', version] as const,
  settings: ['product', 'settings'] as const,
};

// Installs download and run host installers; give them the gateway's own ceiling.
const INSTALL_TIMEOUT_MS = 11 * 60_000;
const SKILL_TIMEOUT_MS = 70_000;
// Every open app re-scans its folders; a slow host can take a while.
const RELOAD_TIMEOUT_MS = 120_000;

async function postJson<T>(path: string, body: unknown, timeoutMs: number, method = 'POST'): Promise<T> {
  const res = await fetch(`${API_BASE}${path}`, {
    method,
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
    signal: AbortSignal.timeout(timeoutMs),
  });
  return adminJsonResponse<T>(res, path);
}

/// The `error` code of an admin error envelope (`local_only`, `install_in_progress`, …).
export function errorCode(err: unknown): string | null {
  if (err instanceof AdminApiError && err.payload && typeof err.payload === 'object') {
    const code = (err.payload as { error?: unknown }).error;
    return typeof code === 'string' ? code : null;
  }
  return null;
}

export function errorMessage(err: unknown): string {
  if (err instanceof AdminApiError && err.payload && typeof err.payload === 'object') {
    const message = (err.payload as { message?: unknown }).message;
    if (typeof message === 'string' && message) return message;
  }
  return err instanceof Error ? err.message : String(err);
}

export function useConnectorsQuery(enabled = true) {
  return useQuery({
    queryKey: keys.connectors,
    queryFn: () => apiJson<ConnectorsPayload>('/connectors'),
    enabled,
    staleTime: 60_000,
  });
}

export function useAgentsQuery(enabled = true) {
  return useQuery({
    queryKey: keys.agents,
    queryFn: () => apiJson<AgentsPayload>('/agents'),
    enabled,
    staleTime: 30_000,
  });
}

export function useInstallConnector() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (body: { dcc_type: string; project?: string; dcc_path?: string }) =>
      postJson<InstallReport>('/connectors/install', body, INSTALL_TIMEOUT_MS),
    onSettled: () => client.invalidateQueries({ queryKey: keys.connectors }),
  });
}

export function useAgentSkill() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (body: { agents: string[]; remove: boolean }) =>
      postJson<AgentsPayload>('/agents/skill', body, SKILL_TIMEOUT_MS),
    onSettled: () => client.invalidateQueries({ queryKey: keys.agents }),
  });
}

/// Polled while the Skills page is open, so skills an agent loads show up.
export function useSkillLibraryQuery() {
  return useQuery({
    queryKey: keys.skills,
    queryFn: () => apiJson<SkillLibrary>('/skill-library'),
    refetchInterval: 5_000,
  });
}

export function useReloadSkills() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: () => postJson<SkillReloadResult>('/skill-library/reload', {}, RELOAD_TIMEOUT_MS),
    onSettled: () => client.invalidateQueries({ queryKey: keys.skills }),
  });
}

/// Adding or removing a folder also makes the open apps reload their skills.
export function useSkillFolder() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: async (change: { add: string } | { remove: number }) => {
      if ('add' in change) return postJson<{ ok: boolean }>('/skill-paths', { path: change.add }, RELOAD_TIMEOUT_MS);
      const path = `/skill-paths/${change.remove}`;
      const res = await fetch(`${API_BASE}${path}`, { method: 'DELETE', signal: AbortSignal.timeout(RELOAD_TIMEOUT_MS) });
      return adminJsonResponse<{ ok: boolean }>(res, path);
    },
    onSettled: () => client.invalidateQueries({ queryKey: keys.skills }),
  });
}

/// `/v1/update/check` sits at the gateway root, next to `/admin`.
function gatewayRootUrl(path: string): string {
  const base = API_BASE.replace(/\/admin\/api\/?$/, '').replace(/\/api\/?$/, '');
  return `${base}${path}`;
}

/// Checked once per page load (it reads the release manifest online, never
/// downloads); `refetch()` checks again.
export type Settings = { update_check: boolean };

/// Preferences shared with the CLI (`~/.meshy-dcc/settings.json`).
export function useSettings() {
  return useQuery({ queryKey: keys.settings, queryFn: () => apiJson<Settings>('/settings'), staleTime: 60_000 });
}

export function useSetUpdateCheck() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (on: boolean) => postJson<Settings>('/settings', { update_check: on }, 10_000, 'PUT'),
    onSuccess: (settings) => client.setQueryData(keys.settings, settings),
  });
}

/// The release check. It runs by itself only while the daily check is on;
/// `refetch()` is the "Check for updates" button either way.
export function useUpdateCheck(currentVersion: string | undefined) {
  const automatic = useSettings().data?.update_check ?? false;
  return useQuery({
    queryKey: keys.update(currentVersion ?? ''),
    queryFn: async () => {
      const path = `/v1/update/check?binary=meshy-dcc-server&current_version=${encodeURIComponent(currentVersion ?? '')}`;
      const res = await fetch(gatewayRootUrl(path), { signal: AbortSignal.timeout(20_000) });
      return adminJsonResponse<UpdateCheck>(res, path);
    },
    enabled: Boolean(currentVersion) && automatic,
    retry: false,
    staleTime: 24 * 60 * 60_000,
  });
}
