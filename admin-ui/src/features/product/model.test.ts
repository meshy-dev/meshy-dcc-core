import { describe, expect, it } from 'vitest';

import type { InstanceRow } from '../../admin-types';
import {
  agentEntries,
  connectedInstances,
  connectorViews,
  guessHome,
  instanceTone,
  isNewer,
  onePerFolder,
  skillPromptTarget,
  skillSourceCounts,
  startProgress,
  tildePath,
  visibleSkills,
  type AgentsPayload,
  type SkillLibrary,
} from './model';

const row = (patch: Partial<InstanceRow>): InstanceRow => ({
  instance_id: 'a3f1c0de-0000-4000-8000-000000000000',
  display_name: null,
  dcc_type: 'blender',
  status: 'available',
  stale: false,
  pid: 1,
  uptime_secs: 1,
  version: '4.5.1',
  server_version: null,
  adapter_version: '0.1.0',
  instance_type: 'gui',
  cpu_percent: null,
  memory_bytes: null,
  mcp_url: '',
  dispatch_ready: true,
  ...patch,
});

describe('connectorViews', () => {
  it('orders installed, available, then coming soon, and spots updates', () => {
    const views = connectorViews({
      dcc_types: [
        { dcc_type: 'maya', adapters: [{ name: 'm', catalog_install_available: false }] },
        { dcc_type: 'unity', adapters: [{ name: 'u', version: '0.2.0', catalog_install_available: true, per_project: true }] },
        { dcc_type: 'blender', adapters: [{ name: 'b', version: '0.1.0', catalog_install_available: true }] },
      ],
      installed: [{ dcc_type: 'unity', adapter_version: '0.1.0', core_version: '0.2.0', installed_at: 1, project: '/p' }],
    });
    expect(views.map((v) => [v.dccType, v.state])).toEqual([
      ['unity', 'installed'],
      ['blender', 'available'],
      ['maya', 'soon'],
    ]);
    expect(views[0].updateAvailable).toBe(true);
    expect(views[0].perProject).toBe(true);
    expect(views[1].updateAvailable).toBe(false);
  });
});

describe('instances', () => {
  it('maps registry status to what the user sees', () => {
    expect(instanceTone(row({}))).toBe('ready');
    expect(instanceTone(row({ status: 'busy' }))).toBe('busy');
    expect(instanceTone(row({ status: 'booting' }))).toBe('starting');
    expect(instanceTone(row({ dispatch_ready: false }))).toBe('ready');
    expect(instanceTone(row({ status: 'unreachable' }))).toBe('notResponding');
    expect(connectedInstances([row({}), row({ stale: true }), row({ status: 'shutting_down' })])).toHaveLength(1);
  });
});

describe('agents', () => {
  const payload: AgentsPayload = {
    skill: 'meshy-dcc',
    digest: 'x',
    failed: false,
    targets: [
      { agent: 'claude-code', agents: [{ id: 'claude-code', name: 'Claude Code', detected: true }], path: '/home/u/.claude/skills/meshy-dcc', state: 'current', action: 'none' },
      {
        agent: 'codex',
        agents: [{ id: 'codex', name: 'Codex', detected: true }, { id: 'cline', name: 'Cline', detected: false }],
        path: '/home/u/.agents/skills/meshy-dcc',
        state: 'missing',
        action: 'install',
      },
    ],
  };

  it('lists every agent with its shared folder', () => {
    const entries = agentEntries(payload);
    expect(entries.map((e) => [e.id, e.on, e.detected])).toEqual([
      ['claude-code', true, true],
      ['codex', false, true],
      ['cline', false, false],
    ]);
    expect(entries[1].sharedWith).toEqual(['Cline']);
    expect(onePerFolder(entries).map((e) => e.id)).toEqual(['claude-code', 'codex']);
    const home = guessHome(entries);
    expect(home).toBe('/home/u');
    expect(tildePath(entries[1].path, home)).toBe('~/.agents/skills');
  });

  it('counts the three get-started steps', () => {
    const progress = startProgress({ connectors: [], agents: agentEntries(payload), successfulCalls: 0 });
    expect(progress).toEqual({ connectors: false, agents: true, call: false, done: false });
  });
});

describe('isNewer', () => {
  it('compares dotted versions', () => {
    expect(isNewer('0.10.0', '0.9.1')).toBe(true);
    expect(isNewer('0.2.0', '0.2.0')).toBe(false);
    expect(isNewer('0.2.0', undefined)).toBe(false);
  });
});

describe('skills', () => {
  const library: SkillLibrary = {
    instances: [{ instance_id: 'b0af-1', dcc_type: 'blender', skill_count: 3, loaded: ['blender-scene'], error: null }],
    skills: [
      { dcc_type: 'blender', name: 'blender-scene', description: 'Scene objects', stage: 'scene', tool_count: 4, source: 'builtin', loaded_on: ['b0af-1'] },
      { dcc_type: 'blender', name: 'my-report', description: 'Count objects', stage: 'scene', tool_count: 1, source: 'mine', loaded_on: [] },
      { dcc_type: 'blender', name: 'blender-camera', description: 'Cameras', stage: 'scene', tool_count: 4, source: 'builtin', loaded_on: [] },
    ],
    folders: [{ path: '~/.meshy-dcc/blender/skills', dcc_type: 'blender', custom_id: null, exists: true, skills: [] }],
  };

  it('counts sources and lists the user\'s skills first', () => {
    expect(skillSourceCounts(library)).toEqual({ all: 3, builtin: 2, mine: 1, marketplace: 0 });
    expect(visibleSkills(library, 'all', '').map((s) => s.name)).toEqual(['my-report', 'blender-camera', 'blender-scene']);
    expect(visibleSkills(library, 'builtin', 'camera').map((s) => s.name)).toEqual(['blender-camera']);
    expect(visibleSkills(library, 'all', 'count').map((s) => s.name)).toEqual(['my-report']);
  });

  it('writes the prompt for an open app and its default folder', () => {
    expect(skillPromptTarget(library)).toEqual({ dccType: 'blender', folder: '~/.meshy-dcc/blender/skills' });
    expect(skillPromptTarget(undefined)).toEqual({ dccType: 'blender', folder: '~/.meshy-dcc/blender/skills' });
  });
});
