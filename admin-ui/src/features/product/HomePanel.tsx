import { useState } from 'react';

import type { HealthPayload, InstanceRow, Panel } from '../../admin-types';
import { useUpdateCheck } from './api';
import { DOCS_URL, MESHY_ICON, PRODUCT_NAME, WEBSITE_URL, dccName } from './catalog';
import {
  appLabel,
  instanceTone,
  sceneLabel,
  shortInstanceId,
  toneCounts,
  type AgentEntry,
  type ConnectorView,
  type InstanceTone,
  type StartProgress,
} from './model';
import { AgentLogo, CopyButton, DccLogo, ExternalLink, Icon, Tag, formatDuration, type T } from './ui';

const START_DISMISSED_KEY = 'meshy-dcc-ui-start-dismissed';

function readFlag(key: string): boolean {
  try {
    return window.localStorage.getItem(key) === '1';
  } catch {
    return false;
  }
}

function writeFlag(key: string, on: boolean): void {
  try {
    if (on) window.localStorage.setItem(key, '1');
    else window.localStorage.removeItem(key);
  } catch {
    // Storage may be blocked; the choice holds for this visit only.
  }
}

/// Agents that count as "turned on": detected ones, plus any the user added
/// on purpose (not just sharing a folder with another agent).
export function visibleOnAgents(agents: AgentEntry[]): AgentEntry[] {
  return agents.filter((a) => a.on && (a.detected || a.sharedWith.length === 0));
}

const TONE_TAG: Record<InstanceTone, 'ok' | 'warn' | 'info'> = { ready: 'ok', busy: 'info', starting: 'info', notResponding: 'warn' };
const TONE_KEY = {
  ready: 'product.instance.ready',
  busy: 'product.instance.busy',
  starting: 'product.instance.starting',
  notResponding: 'product.instance.notResponding',
} as const;
const TONE_COUNT_KEY = {
  ready: 'product.instance.ready.count',
  busy: 'product.instance.busy.count',
  starting: 'product.instance.starting.count',
  notResponding: 'product.instance.notResponding.count',
} as const;

export type HomeData = {
  health: HealthPayload | undefined;
  reachable: boolean;
  local: boolean;
  address: string;
  instances: InstanceRow[];
  connectors: ConnectorView[];
  agents: AgentEntry[];
  progress: StartProgress;
  /// Connectors, agents and call stats have all answered once.
  progressKnown: boolean;
};

export function HomePanel({ t, data, onNavigate, onRecheck }: {
  t: T;
  data: HomeData;
  onNavigate: (panel: Panel) => void;
  onRecheck: () => void;
}) {
  const [dismissed, setDismissed] = useState(() => readFlag(START_DISMISSED_KEY));
  const [forceStart, setForceStart] = useState(false);
  const showStart = forceStart || (data.progressKnown && !data.progress.done && !dismissed);

  if (showStart) {
    return (
      <GetStarted
        t={t}
        data={data}
        onNavigate={onNavigate}
        onClose={() => {
          writeFlag(START_DISMISSED_KEY, true);
          setDismissed(true);
          setForceStart(false);
        }}
      />
    );
  }
  return <Overview t={t} data={data} onNavigate={onNavigate} onRecheck={onRecheck} onShowStart={() => setForceStart(true)} />;
}

// ── Get started ───────────────────────────────────────────────────────────────

function GetStarted({ t, data, onNavigate, onClose }: {
  t: T;
  data: HomeData;
  onNavigate: (panel: Panel) => void;
  onClose: () => void;
}) {
  const { progress } = data;
  const current = !progress.connectors ? 1 : !progress.agents ? 2 : !progress.call ? 3 : 0;
  const stepClass = (n: number, done: boolean) => `pd-step${done ? ' done' : n === current ? ' current' : ''}`;
  const actionClass = (n: number) => `pd-btn ${n === current ? 'primary' : 'secondary'}`;
  // Hosts with a released connector version, in display order.
  const shipped = data.connectors.filter((c) => c.adapter?.version);
  const moreAgents = data.agents.length > 13 ? Math.floor((data.agents.length - 3) / 10) * 10 : 70;

  return (
    <div className="pd-start">
      <div className="pd-start-inner">
        <div className="pd-start-hero">
          <img src={MESHY_ICON} alt="" />
          <h1>{t('product.start.title', { product: PRODUCT_NAME })}</h1>
          <p className="pd-lead">{t('product.start.subtitle')}</p>
        </div>
        <section className="pd-steps" aria-label={t('product.start.title', { product: PRODUCT_NAME })}>
          <div className={stepClass(1, progress.connectors)}>
            <span className="pd-step-num">{progress.connectors ? <Icon name="check" size={14} stroke={2.4} /> : 1}</span>
            <span className="pd-step-text">
              <span className="pd-step-title">{t('product.start.step1.title')}</span>
              <span className="pd-step-hint">
                {shipped.slice(0, 4).map((c) => <DccLogo key={c.dccType} dccType={c.dccType} name={c.name} size={14} />)}
                {t('product.start.step1.hint')}
              </span>
            </span>
            <button type="button" className={actionClass(1)} onClick={() => onNavigate('connectors')}>
              {t('product.start.step1.action')}
            </button>
          </div>
          <div className={stepClass(2, progress.agents)}>
            <span className="pd-step-num">{progress.agents ? <Icon name="check" size={14} stroke={2.4} /> : 2}</span>
            <span className="pd-step-text">
              <span className="pd-step-title">{t('product.start.step2.title')}</span>
              <span className="pd-step-hint">{t('product.start.step2.hint', { count: moreAgents })}</span>
            </span>
            <button type="button" className={actionClass(2)} onClick={() => onNavigate('agents')}>
              {t('product.start.step2.action')}
            </button>
          </div>
          <div className={stepClass(3, progress.call)}>
            <span className="pd-step-num">{progress.call ? <Icon name="check" size={14} stroke={2.4} /> : 3}</span>
            <span className="pd-step-text">
              <span className="pd-step-title">{t('product.start.step3.title')}</span>
              <span className="pd-step-hint">{t('product.start.step3.hint')}</span>
            </span>
            <CopyButton text={t('product.start.prompt')} t={t} label={t('product.start.step3.action')} size="md"
              className={current === 3 ? 'primary' : 'tertiary'} />
          </div>
        </section>
        <p className="pd-note" style={{ textAlign: 'center' }}>
          {t('product.start.footer')}{' '}
          <ExternalLink href={DOCS_URL} className="pd-a">{t('product.start.guide')}</ExternalLink>
          {' · '}
          <button type="button" className="pd-btn sm tertiary" onClick={onClose}>{t('product.start.toOverview')}</button>
        </p>
      </div>
    </div>
  );
}

// ── Overview ──────────────────────────────────────────────────────────────────

function Overview({ t, data, onNavigate, onRecheck, onShowStart }: {
  t: T;
  data: HomeData;
  onNavigate: (panel: Panel) => void;
  onRecheck: () => void;
  onShowStart: () => void;
}) {
  const { health, instances } = data;
  const version = health?.version;
  const update = useUpdateCheck(version);
  const service = health?.service;
  const counts = toneCounts(instances);
  const stuck = instances.find((row) => instanceTone(row) === 'notResponding');
  const installed = data.connectors.filter((c) => c.state === 'installed');
  const available = data.connectors.filter((c) => c.state === 'available');
  const onAgents = visibleOnAgents(data.agents);
  const foundOff = data.agents.filter((a) => a.detected && !a.on);

  const versionSub = update.data
    ? update.data.update_available
      ? t('product.overview.version.available', { version: update.data.latest_version ?? '' })
      : t('product.overview.version.latest')
    : t('product.overview.version.unchecked');
  const uptimeSub = service?.persist
    ? t('product.overview.uptime.persist')
    : service?.started_by && service.started_by !== 'gateway'
      ? t('product.overview.uptime.startedBy', { app: dccName(service.started_by) })
      : t('product.overview.uptime.idle', { seconds: service?.idle_timeout_secs ?? 30 });
  const breakdown = (Object.keys(counts) as InstanceTone[])
    .filter((tone) => counts[tone] > 0)
    .map((tone) => t(TONE_COUNT_KEY[tone], { count: counts[tone] }))
    .join(' · ');

  return (
    <div className="pd-page">
      <div className="pd-page-head">
        <div>
          <h1 className="pd-h1">{PRODUCT_NAME}</h1>
          <span className="pd-lead">
            {data.local ? t('product.overview.subtitle') : t('product.overview.subtitleRemote', { address: data.address })}
          </span>
        </div>
        <button type="button" className="pd-btn icon" aria-label={t('product.action.showStart')} title={t('product.action.showStart')}
          onClick={onShowStart}>
          <Icon name="bulb" />
        </button>
      </div>

      {!data.reachable ? (
        <div className="pd-banner" role="status">
          <span>{t('product.overview.unreachable', { address: data.address })}</span>
        </div>
      ) : stuck ? (
        <div className="pd-banner" role="status">
          <DccLogo dccType={stuck.dcc_type} name={dccName(stuck.dcc_type)} size={16} />
          <span>{t('product.overview.notResponding', { app: dccName(stuck.dcc_type) })}</span>
          <button type="button" className="pd-btn sm secondary" onClick={onRecheck}>{t('product.action.checkAgain')}</button>
        </div>
      ) : null}

      <section className="pd-section" aria-labelledby="pd-status-title">
        <div className="pd-section-head">
          <h2 id="pd-status-title" className="pd-h2">{t('product.overview.status')}</h2>
          <span className="pd-note">{t('product.overview.live')}</span>
        </div>
        <div className="pd-card pd-status">
          <div className="pd-status-core">
            <div className="pd-between">
              <button type="button" className="pd-stat-label" onClick={() => onNavigate('settings')}>
                {t('product.overview.core')}<Icon name="chevronRight" size={14} stroke={1.5} />
              </button>
              {data.reachable
                ? <Tag tone="ok" dot>{t('product.status.running')}</Tag>
                : <Tag tone="muted">{t('product.status.unreachable')}</Tag>}
            </div>
            <div className="pd-tiles">
              <div className="pd-tile">
                <p>{t('product.overview.mode')}</p>
                <p className="pd-tile-value">{t(data.local ? 'product.overview.mode.local' : 'product.overview.mode.remote')}</p>
                <p className="pd-tile-sub">{t(data.local ? 'product.overview.mode.localHint' : 'product.overview.mode.remoteHint')}</p>
              </div>
              <div className="pd-tile">
                <p>{t('product.overview.address')}</p>
                <p className="pd-tile-value code">{data.address}</p>
                <p className="pd-tile-sub">
                  {service?.network_listener ? `${t('product.overview.network.on')} · ${service.network_listener}` : t('product.overview.network.off')}
                </p>
              </div>
              <div className="pd-tile">
                <p>{t('product.overview.version')}</p>
                <p className="pd-tile-value">{version ?? '—'}</p>
                <p className={update.data && !update.data.update_available ? 'pd-tile-sub ok' : 'pd-tile-sub'}>{versionSub}</p>
              </div>
              <div className="pd-tile">
                <p>{t('product.overview.uptime')}</p>
                <p className="pd-tile-value">{health ? formatDuration(health.uptime_secs, t) : '—'}</p>
                <p className="pd-tile-sub">{uptimeSub}</p>
              </div>
            </div>
          </div>
          <div className="pd-vrule" aria-hidden="true" />
          <div className="pd-status-side">
            <div className="pd-status-row">
              <div className="pd-stat" style={{ flex: '3 1 160px' }}>
                <span className="pd-stat-label">{t('product.overview.connected')}</span>
                <div className="pd-stat-foot">
                  <div>
                    <p className="pd-stat-value">{instances.length}</p>
                    <p className="pd-stat-sub">{breakdown || t('product.overview.connected.none')}</p>
                  </div>
                </div>
              </div>
              <div className="pd-vrule" style={{ margin: '20px 0 0' }} aria-hidden="true" />
              <div className="pd-stat" style={{ flex: '4 1 160px' }}>
                <button type="button" className="pd-stat-label" onClick={() => onNavigate('connectors')}>
                  {t('product.overview.connectors')}<Icon name="chevronRight" size={14} stroke={1.5} />
                </button>
                <div className="pd-stat-foot">
                  <div>
                    <p className="pd-stat-value">{installed.length}</p>
                    <p className="pd-stat-sub">
                      {installed.map((c) => <DccLogo key={c.dccType} dccType={c.dccType} name={c.name} size={14} />)}
                      {available.length > 0
                        ? t('product.overview.connectors.summary', { count: available.length })
                        : t('product.overview.connectors.summaryNone')}
                    </p>
                  </div>
                </div>
              </div>
            </div>
            <div className="pd-hrule" style={{ marginRight: 16 }} aria-hidden="true" />
            <div className="pd-stat">
              <button type="button" className="pd-stat-label" onClick={() => onNavigate('agents')}>
                {t('product.overview.agents')}<Icon name="chevronRight" size={14} stroke={1.5} />
              </button>
              <div className="pd-stat-foot">
                <div>
                  <p className="pd-stat-value">{onAgents.length}</p>
                  <p className="pd-stat-sub">
                    {onAgents.slice(0, 3).map((a) => <AgentLogo key={a.id} id={a.id} name={a.name} size={14} />)}
                    {[
                      onAgents.slice(0, 3).map((a) => a.name).join(', '),
                      foundOff.length > 0 ? t('product.overview.agents.found', { count: foundOff.length }) : '',
                    ].filter(Boolean).join(' · ') || t('product.overview.agents.none')}
                  </p>
                </div>
                {foundOff.length > 0 ? (
                  <button type="button" className="pd-btn sm secondary" onClick={() => onNavigate('agents')}>
                    {t('product.overview.agents.turnOn')}
                  </button>
                ) : null}
              </div>
            </div>
          </div>
        </div>
      </section>

      <div className="pd-split">
        <section className="pd-section" aria-labelledby="pd-open-title">
          <div className="pd-section-head">
            <h2 id="pd-open-title" className="pd-h2">{t('product.overview.connected')}</h2>
            <span className="pd-note">{t('product.overview.table.hint')}</span>
          </div>
          <div className="pd-card pd-table">
            {instances.length === 0 ? (
              <p className="pd-table-empty">{t('product.overview.table.empty')}</p>
            ) : (
              <div className="pd-table-inner" role="table">
                <div className="pd-tr head" role="row">
                  <span role="columnheader">{t('product.overview.table.app')}</span>
                  <span role="columnheader">{t('product.overview.table.scene')}</span>
                  <span role="columnheader">{t('product.overview.table.instance')}</span>
                  <span role="columnheader">{t('product.overview.table.status')}</span>
                </div>
                {instances.map((row) => {
                  const tone = instanceTone(row);
                  return (
                    <div className="pd-tr" role="row" key={row.instance_id}>
                      <span className="app" role="cell">
                        <DccLogo dccType={row.dcc_type} name={dccName(row.dcc_type)} />
                        {appLabel(row)}
                      </span>
                      <span className="muted" role="cell" title={row.scene ?? undefined}>
                        {sceneLabel(row) ?? t('product.overview.table.untitled')}
                      </span>
                      <span className="id" role="cell" title={row.instance_id}>{shortInstanceId(row.instance_id)}</span>
                      <span role="cell"><Tag tone={TONE_TAG[tone]}>{t(TONE_KEY[tone])}</Tag></span>
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        </section>

        <section className="pd-section" aria-labelledby="pd-installed-title">
          <h2 id="pd-installed-title" className="pd-h2">{t('product.installed.title')}</h2>
          <div className="pd-card pd-installed">
            <div className="pd-installed-row">
              <img src={MESHY_ICON} alt="" className="pd-logo" />
              <span>{PRODUCT_NAME}</span>
              <span>{version ?? '—'}</span>
            </div>
            <div>
              <div className="pd-installed-row">
                <img src={MESHY_ICON} alt="" className="pd-logo" />
                <span>{t('product.installed.skill')}</span>
                <span>{version ?? '—'}</span>
              </div>
              <div className="pd-installed-in">
                {onAgents.length === 0 ? t('product.installed.nowhere') : (
                  <>
                    {t('product.installed.in')}
                    {onAgents.map((a) => (
                      <span key={a.id} style={{ display: 'inline-flex', alignItems: 'center', gap: 6 }}>
                        <AgentLogo id={a.id} name={a.name} size={14} />{a.name}
                      </span>
                    ))}
                  </>
                )}
              </div>
            </div>
            {installed.length > 0 ? <div className="pd-hrule" style={{ margin: '2px 8px' }} aria-hidden="true" /> : null}
            {installed.map((c) => (
              <div className="pd-installed-row" key={c.dccType}>
                <DccLogo dccType={c.dccType} name={c.name} />
                <span>{t('product.installed.connector', { app: c.name })}</span>
                <span>{c.installedVersion}</span>
              </div>
            ))}
            <div className="pd-installed-foot">
              <button type="button" className="pd-btn sm secondary" style={{ width: '100%' }} disabled={update.isFetching}
                onClick={() => void update.refetch()}>
                {update.isFetching ? t('product.installed.checking') : t('product.installed.check')}
              </button>
              <span className="pd-note">
                {update.isError
                  ? t('product.installed.checkFailed')
                  : update.data?.update_available
                    ? t('product.installed.available', { product: PRODUCT_NAME, version: update.data.latest_version ?? '' })
                    : update.data
                      ? t('product.installed.upToDate')
                      : ''}
              </span>
            </div>
          </div>
        </section>
      </div>

      <section className="pd-section" aria-labelledby="pd-res-title">
        <h2 id="pd-res-title" className="pd-h2">{t('product.resources.title')}</h2>
        <div className="pd-resources">
          <ExternalLink href={DOCS_URL} className="pd-resource">
            <span className="pd-resource-icon"><Icon name="docs" size={20} stroke={1.5} /></span>
            <span><small>{t('product.resources.docs')}</small><strong>{t('product.resources.docs.title', { product: PRODUCT_NAME })}</strong></span>
          </ExternalLink>
          <ExternalLink href={DOCS_URL} className="pd-resource">
            <span className="pd-resource-icon"><Icon name="sparkle" size={20} stroke={1.5} /></span>
            <span><small>{t('product.resources.notes')}</small><strong>{t('product.resources.notes.title', { version: version ?? '' })}</strong></span>
          </ExternalLink>
          <ExternalLink href={WEBSITE_URL} className="pd-resource">
            <span className="pd-resource-icon"><Icon name="website" size={20} stroke={1.5} /></span>
            <span><small>{t('product.nav.website')}</small><strong>{t('product.resources.website.title')}</strong></span>
          </ExternalLink>
        </div>
      </section>
    </div>
  );
}
