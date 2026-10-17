import { useState } from 'react';

import type { MessageKey } from '../../i18n';
import { errorCode, errorMessage, useConnectorsQuery, useInstallConnector } from './api';
import { DCC_LOGOS, DCC_REQUIREMENTS } from './catalog';
import { connectorProjects, connectorViews, type ConnectorView, type InstallReport } from './model';
import { Command, DccLogo, Icon, Tag, type T } from './ui';

type Filter = 'all' | 'installed' | 'available' | 'soon';

const DESCRIPTIONS: Record<string, MessageKey> = {
  blender: 'product.connectors.desc.blender',
  unity: 'product.connectors.desc.unity',
  godot: 'product.connectors.desc.godot',
  unreal: 'product.connectors.desc.unreal',
  maya: 'product.connectors.desc.maya',
  zbrush: 'product.connectors.desc.zbrush',
  freecad: 'product.connectors.desc.freecad',
  'material-maker': 'product.connectors.desc.material-maker',
  houdini: 'product.connectors.desc.houdini',
  substance3d_painter: 'product.connectors.desc.substance3d_painter',
  openscad: 'product.connectors.desc.openscad',
  c4d: 'product.connectors.desc.c4d',
  roblox: 'product.connectors.desc.roblox',
  '3dsmax': 'product.connectors.desc.3dsmax',
};

function installCommand(view: ConnectorView, t: T): string {
  const project = view.perProject ? ` --project <${t('product.connectors.project.label')}>` : '';
  return `meshy-dcc install --dcc-type ${view.dccType}${project} --execute`;
}

/// Why an install report failed, in the words the CLI used.
function reportReason(report: InstallReport): string {
  const step = report.steps?.find((s) => s.status === 'failed' && s.message);
  return report.error?.message ?? step?.message ?? report.error?.code ?? report.status;
}

type Outcome = { tone: 'ok' | 'err' | 'busy'; text: string };
/// The path an install still needs: a project folder, or where the app is
/// when the CLI could not find it.
type PathForm = { dccType: string; kind: 'project' | 'app' };

export function ConnectorsPanel({ t, local }: { t: T; local: boolean }) {
  const query = useConnectorsQuery();
  const install = useInstallConnector();
  const [filter, setFilter] = useState<Filter>('all');
  const [search, setSearch] = useState('');
  const [form, setForm] = useState<PathForm | null>(null);
  const [formPath, setFormPath] = useState('');
  const [pending, setPending] = useState<string | null>(null);
  const [outcomes, setOutcomes] = useState<Record<string, Outcome>>({});

  const views = connectorViews(query.data);
  const count = (state: Filter) => views.filter((v) => v.state === state).length;
  const needle = search.trim().toLowerCase();
  const matches = views.filter((v) => (filter === 'all' || v.state === filter)
    && (!needle || v.name.toLowerCase().includes(needle) || v.dccType.includes(needle)));
  const listed = matches.filter((v) => v.state !== 'soon');
  const soon = matches.filter((v) => v.state === 'soon');

  const runInstall = async (view: ConnectorView, paths: { project?: string; dcc_path?: string } = {}) => {
    setPending(view.dccType);
    setOutcomes((prev) => ({ ...prev, [view.dccType]: { tone: 'busy', text: t('product.connectors.installing') } }));
    let outcome: Outcome;
    try {
      const report = await install.mutateAsync({ dcc_type: view.dccType, ...paths });
      // `partial` is a normal install: the host registers itself when it starts.
      const installed = report.status === 'ok' || report.status === 'partial';
      const appMissing = report.error?.code === 'DCC_PATH_REQUIRED';
      outcome = installed
        ? { tone: 'ok', text: t('product.connectors.done', { app: view.name, version: report.adapter_version }) }
        : appMissing
          ? { tone: 'err', text: t('product.connectors.app.notFound', { app: view.name }) }
          : { tone: 'err', text: t('product.connectors.failed', { reason: reportReason(report) }) };
      if (installed) setForm(null);
      if (appMissing) {
        setForm({ dccType: view.dccType, kind: 'app' });
        setFormPath('');
      }
    } catch (err) {
      const code = errorCode(err);
      outcome = code === 'install_in_progress'
        ? { tone: 'err', text: t('product.connectors.busy') }
        : { tone: 'err', text: t('product.connectors.failed', { reason: errorMessage(err) }) };
    }
    setPending(null);
    setOutcomes((prev) => ({ ...prev, [view.dccType]: outcome }));
  };

  const description = (view: ConnectorView) => {
    const parts = [
      DESCRIPTIONS[view.dccType] ? t(DESCRIPTIONS[view.dccType]) : null,
      DCC_REQUIREMENTS[view.dccType] ?? null,
    ];
    const version = view.installedVersion ?? view.adapter?.version;
    if (version) {
      const status = view.state === 'installed'
        ? view.updateAvailable
          ? t('product.connectors.updateTo', { version: view.adapter?.version ?? '' })
          : t('product.connectors.upToDate')
        : null;
      parts.push([t('product.connectors.version', { version }), status].filter(Boolean).join(', '));
    }
    if (view.perProject && view.state !== 'installed') parts.push(t('product.connectors.perProject'));
    if (view.dccType === 'unreal') parts.push(t('product.connectors.unrealNote'));
    if (view.experimental) parts.push(t('product.connectors.experimentalNote'));
    return parts.filter(Boolean).join(' · ');
  };

  const statusTag = (view: ConnectorView) => {
    if (view.state !== 'installed') return null;
    if (!view.perProject) return <Tag tone="ok">{t('product.connectors.tag.installed')}</Tag>;
    const projects = connectorProjects(view).length;
    return <Tag tone="ok">{projects === 1 ? t('product.connectors.tag.oneProject') : t('product.connectors.tag.projects', { count: projects })}</Tag>;
  };

  const actions = (view: ConnectorView) => {
    if (!view.installable) return null;
    const busy = pending !== null;
    if (view.perProject) {
      return (
        <button type="button" className="pd-btn secondary" disabled={busy}
          onClick={() => { setForm({ dccType: view.dccType, kind: 'project' }); setFormPath(''); }}>
          {t('product.connectors.addToProject')}
        </button>
      );
    }
    if (view.state === 'installed' && !view.updateAvailable) return null;
    return (
      <button type="button" className={view.state === 'installed' ? 'pd-btn secondary' : 'pd-btn primary'} disabled={busy}
        onClick={() => void runInstall(view)}>
        {view.state === 'installed' ? t('product.connectors.update') : t('product.connectors.install')}
      </button>
    );
  };

  return (
    <div className="pd-page narrow">
      <div className="pd-page-head">
        <div>
          <h1 className="pd-h1">{t('product.nav.connectors')}</h1>
          <p className="pd-lead">{t('product.connectors.lead')}</p>
        </div>
      </div>

      <div className="pd-toolbar">
        <div className="pd-segmented" role="group" aria-label={t('product.connectors.filter')}>
          {(['all', 'installed', 'available', 'soon'] as Filter[]).map((id) => (
            <button type="button" key={id} aria-pressed={filter === id} onClick={() => setFilter(id)}>
              {id === 'all' ? t('product.connectors.filter.all') : t(`product.connectors.filter.${id}` as MessageKey, { count: count(id) })}
            </button>
          ))}
        </div>
        <label className="pd-search">
          <Icon name="search" size={14} />
          <span className="pd-sr-only">{t('product.connectors.search')}</span>
          <input type="search" placeholder={t('product.connectors.search')} value={search} onChange={(e) => setSearch(e.target.value)} />
        </label>
      </div>

      {query.isError ? <p className="pd-error">{t('product.connectors.loadFailed', { reason: errorMessage(query.error) })}</p> : null}

      {!local && listed.some((v) => v.installable) ? (
        <div className="pd-banner info"><span>{t('product.connectors.localOnly')}</span></div>
      ) : null}

      {listed.length > 0 ? (
        <ul className="pd-list">
          {listed.map((view) => {
            const outcome = outcomes[view.dccType];
            return (
              <li key={view.dccType}>
                <DccLogo dccType={view.dccType} name={view.name} size={28} />
                <div className="pd-list-text">
                  <div className="pd-list-title">
                    <h2>{view.name}</h2>
                    {statusTag(view)}
                    {view.experimental ? <Tag tone="warn">{t('product.connectors.tag.experimental')}</Tag> : null}
                  </div>
                  <span className="pd-list-desc">{description(view)}</span>
                </div>
                {local ? <div className="pd-list-actions">{actions(view)}</div> : null}
                {!local && view.installable ? <div className="pd-result"><Command text={installCommand(view, t)} t={t} /></div> : null}
                {local && form?.dccType === view.dccType ? (
                  <form className="pd-project-form" onSubmit={(e) => {
                    e.preventDefault();
                    const path = formPath.trim();
                    if (path) void runInstall(view, form.kind === 'app' ? { dcc_path: path } : { project: path });
                  }}>
                    <label className="pd-sr-only" htmlFor={`pd-path-${view.dccType}`}>
                      {form.kind === 'app' ? t('product.connectors.app.label', { app: view.name }) : t('product.connectors.project.label')}
                    </label>
                    <input id={`pd-path-${view.dccType}`} className="pd-input pd-mono-text" value={formPath} autoFocus
                      placeholder={form.kind === 'app' ? t('product.connectors.app.placeholder', { app: view.name }) : t('product.connectors.project.placeholder')}
                      onChange={(e) => setFormPath(e.target.value)} />
                    <button type="submit" className="pd-btn primary" disabled={!formPath.trim() || pending !== null}>
                      {t('product.connectors.install')}
                    </button>
                    <button type="button" className="pd-btn tertiary" disabled={pending !== null} onClick={() => setForm(null)}>
                      {t('product.action.cancel')}
                    </button>
                  </form>
                ) : null}
                {outcome ? (
                  <p className={outcome.tone === 'err' ? 'pd-result pd-error' : 'pd-result pd-note'} role="status">{outcome.text}</p>
                ) : null}
              </li>
            );
          })}
        </ul>
      ) : null}

      {soon.length > 0 ? (
        <section className="pd-section" aria-labelledby="pd-soon-title">
          <h2 id="pd-soon-title" className="pd-h2">{t('product.connectors.soon')}</h2>
          <ul className="pd-chips">
            {soon.map((view) => (
              <li key={view.dccType}>
                {DCC_LOGOS[view.dccType] ? <DccLogo dccType={view.dccType} name={view.name} size={14} /> : null}
                {view.name}
              </li>
            ))}
          </ul>
        </section>
      ) : null}

      {needle && matches.length === 0 ? <p className="pd-note">{t('product.connectors.empty', { query: search.trim() })}</p> : null}
    </div>
  );
}
