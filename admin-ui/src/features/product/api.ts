/// Queries and mutations behind the user-facing pages.

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import { API_BASE, AdminApiError, adminJsonResponse, apiJson } from '../../admin-ui-core';
import type { AgentsPayload, ConnectorsPayload, InstallReport, ReportResult, SkillLibrary, SkillReloadResult, UpdateCheck } from './model';

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
// The CLI waits 20 s for the intake; the page waits a little longer than that.
const REPORT_TIMEOUT_MS = 30_000;
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

/// Never throws: a refusal or a lost connection becomes a `failed` result, and
/// a send with no answer is `unknown` because the report may have arrived.
export function useReport() {
  return useMutation({
    mutationFn: async (body: { summary: string; send: boolean }): Promise<ReportResult> => {
      try {
        return await postJson<ReportResult>('/report', body, REPORT_TIMEOUT_MS);
      } catch (err) {
        // The gateway answers 502 with the CLI's own failed/unknown result.
        const payload = err instanceof AdminApiError ? (err.payload as Partial<ReportResult> | undefined) : undefined;
        if (payload?.status === 'failed' || payload?.status === 'unknown') return payload as ReportResult;
        return { status: body.send && !(err instanceof AdminApiError) ? 'unknown' : 'failed', error: errorMessage(err) };
      }
    },
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
export type Settings = {
  update_check: boolean;
  /// Anonymous usage telemetry: the saved choice, what overrides it
  /// (`DO_NOT_TRACK`, `MESHY_DCC_TELEMETRY`, `ci`, `debug_build`), whether the
  /// window still owes the first-run banner, and the docs page.
  telemetry?: boolean;
  telemetry_off_reason?: string | null;
  telemetry_notice?: boolean;
  telemetry_docs?: string;
};

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

/// Turn telemetry on or off, or dismiss its banner (either choice also dismisses it).
export function useSetTelemetry() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (change: { telemetry: boolean } | { telemetry_notice_seen: true }) =>
      postJson<Settings>('/settings', change, 10_000, 'PUT'),
    onSuccess: (settings) => client.setQueryData(keys.settings, settings),
  });
}

/// `POST /admin/api/update`: `restarting` means the gateway is about to stop
/// and come back as `target_version`.
export type UpdateNowResult = {
  ok: boolean;
  status: 'restarting' | 'staged' | 'up_to_date';
  target_version?: string | null;
  started_by?: string | null;
};

// Downloads two binaries; the gateway's own ceiling is 15 minutes.
const UPDATE_TIMEOUT_MS = 16 * 60_000;
const RESTART_WAIT_MS = 120_000;

/// Poll `readVersion` until it returns `version`; false if it never does in
/// time. A failed read is the gateway restarting, so polling goes on.
export async function waitForVersion(
  readVersion: () => Promise<string | undefined>,
  version: string,
  { timeoutMs = RESTART_WAIT_MS, intervalMs = 1_000 } = {},
): Promise<boolean> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    await new Promise((resolve) => setTimeout(resolve, intervalMs));
    try {
      if ((await readVersion()) === version) return true;
    } catch {
      // Still restarting.
    }
  }
  return false;
}

/// The gateway did not come back with the new version in time.
export class RestartTimeout extends Error {
  constructor(readonly version: string) {
    super(`the gateway did not come back as ${version}`);
  }
}

/// "Update now": stage, wait for the restarted gateway, then reload the page
/// so everything shows the new version.
export function useUpdateNow() {
  return useMutation({
    mutationFn: async (): Promise<UpdateNowResult> => {
      const result = await postJson<UpdateNowResult>('/update', {}, UPDATE_TIMEOUT_MS);
      if (result.status === 'restarting' && result.target_version) {
        const back = await waitForVersion(
          async () => (await apiJson<{ version?: string }>('/health')).version,
          result.target_version,
        );
        if (!back) throw new RestartTimeout(result.target_version);
        window.location.reload();
      }
      return result;
    },
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
