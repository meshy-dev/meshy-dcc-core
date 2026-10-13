/// Pure view logic for the user-facing pages; no React, no fetch.

import type { InstanceRow } from '../../admin-types';
import { DCC_NAMES, dccName } from './catalog';

// ── API payloads (CLI JSON passed through the admin endpoints) ────────────────

export type ConnectorAdapter = {
  name: string;
  version?: string;
  url?: string;
  catalog_install_available: boolean;
  per_project?: boolean;
};

export type InstallReceipt = {
  dcc_type: string;
  adapter_version: string;
  core_version: string;
  installed_at: number;
  project?: string;
};

export type ConnectorsPayload = {
  dcc_types: { dcc_type: string; adapters: ConnectorAdapter[] }[];
  installed?: InstallReceipt[];
};

export type AgentRef = { id: string; name?: string; detected?: boolean };

export type AgentTarget = {
  agent: string | null;
  agents: AgentRef[];
  path: string;
  state: 'missing' | 'current' | 'outdated' | 'foreign';
  action: string;
  result?: string;
  error?: string;
};

export type AgentsPayload = { skill: string; digest: string; failed: boolean; targets: AgentTarget[] };

export type InstallReport = {
  status: string;
  dcc_type: string;
  adapter_version: string;
  error?: { code: string; message?: string } | null;
  steps?: { id: string; status: string; message?: string }[];
};

export type UpdateCheck = { update_available: boolean; latest_version?: string; current_version?: string };

// ── Versions ──────────────────────────────────────────────────────────────────

/// `a` newer than `b`, comparing dotted numbers (pre-release tags ignored).
export function isNewer(a: string | undefined, b: string | undefined): boolean {
  if (!a || !b) return false;
  const parts = (v: string) => v.split(/[-+]/)[0].split('.').map((n) => Number.parseInt(n, 10) || 0);
  const [x, y] = [parts(a), parts(b)];
  for (let i = 0; i < Math.max(x.length, y.length); i += 1) {
    const d = (x[i] ?? 0) - (y[i] ?? 0);
    if (d !== 0) return d > 0;
  }
  return false;
}

// ── Connectors ────────────────────────────────────────────────────────────────

export type ConnectorState = 'installed' | 'available' | 'soon';

export type ConnectorView = {
  dccType: string;
  name: string;
  state: ConnectorState;
  adapter?: ConnectorAdapter;
  perProject: boolean;
  installable: boolean;
  installs: InstallReceipt[];
  installedVersion?: string;
  updateAvailable: boolean;
};

const DISPLAY_ORDER = Object.keys(DCC_NAMES);
const STATE_ORDER: ConnectorState[] = ['installed', 'available', 'soon'];

function displayRank(dccType: string): number {
  const index = DISPLAY_ORDER.indexOf(dccType);
  return index === -1 ? DISPLAY_ORDER.length : index;
}

export function connectorViews(payload: ConnectorsPayload | undefined): ConnectorView[] {
  if (!payload) return [];
  const receipts = payload.installed ?? [];
  return payload.dcc_types
    .map((entry): ConnectorView => {
      const adapter = entry.adapters[0];
      const installs = receipts.filter((r) => r.dcc_type === entry.dcc_type);
      const installable = Boolean(adapter?.catalog_install_available);
      const installedVersion = installs.at(-1)?.adapter_version;
      const state: ConnectorState = installs.length > 0 ? 'installed' : installable ? 'available' : 'soon';
      return {
        dccType: entry.dcc_type,
        name: dccName(entry.dcc_type),
        state,
        adapter,
        perProject: Boolean(adapter?.per_project),
        installable,
        installs,
        installedVersion,
        updateAvailable: installable && isNewer(adapter?.version, installedVersion),
      };
    })
    .sort((a, b) => STATE_ORDER.indexOf(a.state) - STATE_ORDER.indexOf(b.state)
      || displayRank(a.dccType) - displayRank(b.dccType));
}

/// Projects the connector went into, newest install per project.
export function connectorProjects(view: ConnectorView): string[] {
  return [...new Set(view.installs.map((r) => r.project).filter((p): p is string => Boolean(p)))];
}

// ── Instances ─────────────────────────────────────────────────────────────────

export type InstanceTone = 'ready' | 'busy' | 'starting' | 'notResponding';

/// Instances that are leaving (stale, shutting down) are not listed.
export function connectedInstances(rows: InstanceRow[] | undefined): InstanceRow[] {
  return (rows ?? []).filter((row) => !row.stale && row.status !== 'stale' && row.status !== 'shutting_down'
    && !row.dcc_type.startsWith('__'));
}

export function instanceTone(row: InstanceRow): InstanceTone {
  if (row.status === 'unreachable') return 'notResponding';
  // Not every adapter reports dispatch readiness, so only the registry status counts.
  if (row.status === 'booting') return 'starting';
  if (row.status === 'busy') return 'busy';
  return 'ready';
}

export function shortInstanceId(id: string): string {
  return id.replace(/-/g, '').slice(0, 4);
}

/// The scene or project file name, without its folders.
export function sceneLabel(row: InstanceRow): string | null {
  const scene = row.scene?.trim();
  if (!scene) return null;
  return scene.split(/[\\/]/).filter(Boolean).at(-1) ?? scene;
}

/// "Blender 4.5": the host version, reduced to major.minor.
export function appLabel(row: InstanceRow): string {
  const version = row.version?.match(/^\d+(\.\d+)?/)?.[0];
  return version ? `${dccName(row.dcc_type)} ${version}` : dccName(row.dcc_type);
}

export function toneCounts(rows: InstanceRow[]): Record<InstanceTone, number> {
  const counts: Record<InstanceTone, number> = { ready: 0, busy: 0, starting: 0, notResponding: 0 };
  for (const row of rows) counts[instanceTone(row)] += 1;
  return counts;
}

// ── Agents ────────────────────────────────────────────────────────────────────

export type AgentEntry = {
  id: string;
  name: string;
  detected: boolean;
  on: boolean;
  foreign: boolean;
  path: string;
  /// Other agents that read the same skills folder.
  sharedWith: string[];
};

export function agentEntries(payload: AgentsPayload | undefined): AgentEntry[] {
  if (!payload) return [];
  return payload.targets.flatMap((target) => target.agents.map((agent) => ({
    id: agent.id,
    name: agent.name ?? agent.id,
    detected: Boolean(agent.detected),
    on: target.state === 'current' || target.state === 'outdated',
    foreign: target.state === 'foreign',
    // The skill's own folder is `<skills dir>/meshy-dcc`; show the skills dir.
    path: target.path.replace(/[\\/]meshy-dcc$/, ''),
    // Agents installed here first: they are the ones the user will recognise.
    sharedWith: target.agents
      .filter((other) => other.id !== agent.id)
      .sort((a, b) => Number(Boolean(b.detected)) - Number(Boolean(a.detected)))
      .map((other) => other.name ?? other.id),
  })));
}

/// One entry per skills folder, keeping the first; a folder is one switch.
export function onePerFolder(entries: AgentEntry[]): AgentEntry[] {
  const seen = new Set<string>();
  return entries.filter((entry) => !seen.has(entry.path) && Boolean(seen.add(entry.path)));
}

/// `/Users/me/.claude/skills` → `~/.claude/skills`.
export function tildePath(path: string, home: string | null): string {
  if (!home || !path.startsWith(home)) return path;
  return `~${path.slice(home.length)}`;
}

/// The home directory, read off the Claude Code skills path the CLI reports.
export function guessHome(entries: AgentEntry[]): string | null {
  const claude = entries.find((entry) => entry.id === 'claude-code');
  const match = claude?.path.match(/^(.*)[\\/]\.claude[\\/]skills$/);
  return match ? match[1] : null;
}

// ── Get started ───────────────────────────────────────────────────────────────

export type StartProgress = { connectors: boolean; agents: boolean; call: boolean; done: boolean };

export function startProgress(input: {
  connectors: ConnectorView[];
  agents: AgentEntry[];
  successfulCalls: number;
}): StartProgress {
  const connectors = input.connectors.some((c) => c.state === 'installed');
  const agents = input.agents.some((a) => a.on);
  const call = input.successfulCalls > 0;
  return { connectors, agents, call, done: connectors && agents && call };
}
