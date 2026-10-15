import { useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';

import type { InstanceRow } from '../../admin-types';
import { useInstalledMarketplaceQuery, useMarketplaceCatalogQuery, useMarketplaceInstall } from '../../hooks/queries';
import type { MessageKey } from '../../i18n';
import { errorCode, errorMessage, useReloadSkills, useSkillFolder, useSkillLibraryQuery } from './api';
import { dccName } from './catalog';
import {
  appLabel,
  sceneLabel,
  shortInstanceId,
  skillPromptTarget,
  skillSourceCounts,
  visibleSkills,
  type FolderSkillState,
  type SkillFilter,
  type SkillLibrary,
  type SkillSource,
} from './model';
import { Command, CopyButton, DccLogo, Icon, Tag, type T } from './ui';

const STAGES = new Set(['scene', 'authoring', 'lookdev', 'animation', 'render', 'pipeline', 'interchange', 'import', 'diagnostics', 'bootstrap', 'simulation']);
const SOURCE_TONE: Record<SkillSource, 'ok' | 'info' | 'muted'> = { mine: 'ok', marketplace: 'info', builtin: 'muted' };
const STATE_TONE: Record<FolderSkillState, 'ok' | 'warn' | 'muted'> = { active: 'ok', pending: 'warn', shadowed: 'warn', app_closed: 'muted' };

type Notice = { tone: 'ok' | 'err'; text: string } | null;

export function SkillsPanel({ t, local, rows }: { t: T; local: boolean; rows: InstanceRow[] }) {
  const query = useSkillLibraryQuery();
  const reload = useReloadSkills();
  const folder = useSkillFolder();
  const [filter, setFilter] = useState<SkillFilter>('all');
  const [search, setSearch] = useState('');
  const [newFolder, setNewFolder] = useState('');
  const [notice, setNotice] = useState<Notice>(null);

  const library = query.data;
  const counts = skillSourceCounts(library);
  const listed = visibleSkills(library, filter, search);
  const target = skillPromptTarget(library);
  const rowById = new Map(rows.map((row) => [row.instance_id, row]));
  const shortId = (id: string) => shortInstanceId(id);
  const stage = (value: string | null) =>
    value && STAGES.has(value) ? t(`product.skills.stage.${value}` as MessageKey) : value;
  const busy = reload.isPending || folder.isPending;
  // With one instance of an app, "loaded" says it all; with several, say where.
  const instancesOf = (dccType: string) => (library?.instances ?? []).filter((i) => i.dcc_type === dccType).length;
  const loadedTag = (skill: SkillLibrary['skills'][number]) =>
    instancesOf(skill.dcc_type) > 1
      ? t('product.skills.loadedOn', { instances: skill.loaded_on.map(shortId).join(', ') })
      : t('product.skills.loaded');

  const runReload = async () => {
    setNotice(null);
    try {
      const result = await reload.mutateAsync();
      const reason = result.instances.find((row) => row.error)?.error ?? '';
      setNotice(result.failed > 0
        ? { tone: 'err', text: t('product.skills.mine.reloadFailed', { count: result.failed, reason }) }
        : { tone: 'ok', text: t('product.skills.mine.reloaded', { count: result.reloaded }) });
    } catch (err) {
      setNotice({ tone: 'err', text: t('product.skills.mine.reloadFailed', { count: 1, reason: errorMessage(err) }) });
    }
  };

  const changeFolder = async (change: { add: string } | { remove: number }) => {
    setNotice(null);
    try {
      await folder.mutateAsync(change);
      if ('add' in change) setNewFolder('');
    } catch (err) {
      setNotice({
        tone: 'err',
        text: errorCode(err) === 'not_a_folder'
          ? t('product.skills.mine.notFolder')
          : t('product.skills.mine.changeFailed', { reason: errorMessage(err) }),
      });
    }
  };

  return (
    <div className="pd-page narrow">
      <div className="pd-page-head">
        <div>
          <h1 className="pd-h1">{t('product.nav.skills')}</h1>
          <p className="pd-lead">{t('product.skills.lead')}</p>
        </div>
      </div>

      {query.isError ? <p className="pd-error">{t('product.skills.loadFailed', { reason: errorMessage(query.error) })}</p> : null}

      <section className="pd-section" aria-labelledby="pd-skill-instances">
        <h2 id="pd-skill-instances" className="pd-h2">{t('product.skills.instances')}</h2>
        {library && library.instances.length === 0 ? <p className="pd-note">{t('product.skills.instancesEmpty')}</p> : null}
        <div className="pd-skill-instances">
          {library?.instances.map((instance) => {
            const row = rowById.get(instance.instance_id);
            const scene = row ? sceneLabel(row) ?? t('product.overview.table.untitled') : null;
            return (
              <article className="pd-card pd-skill-instance" key={instance.instance_id}>
                <div className="pd-skill-instance-head">
                  <DccLogo dccType={instance.dcc_type} name={dccName(instance.dcc_type)} size={24} />
                  <div className="pd-list-text">
                    <div className="pd-list-title"><h3>{row ? appLabel(row) : dccName(instance.dcc_type)}</h3></div>
                    <span className="pd-list-desc">{[scene, shortId(instance.instance_id)].filter(Boolean).join(' · ')}</span>
                  </div>
                  <span className="pd-skill-count">
                    {t('product.skills.instanceCount', { loaded: instance.loaded.length, total: instance.skill_count })}
                  </span>
                </div>
                {instance.error ? <p className="pd-error">{t('product.skills.instanceError', { reason: instance.error })}</p> : null}
                {!instance.error && instance.loaded.length === 0 ? <p className="pd-note">{t('product.skills.noneLoaded')}</p> : null}
                {instance.loaded.length > 0 ? (
                  <ul className="pd-chips">
                    {instance.loaded.map((name) => <li key={name}><span className="pd-dot" />{name}</li>)}
                  </ul>
                ) : null}
              </article>
            );
          })}
        </div>
      </section>

      <section className="pd-section" aria-labelledby="pd-skill-all">
        <h2 id="pd-skill-all" className="pd-h2">{t('product.skills.all')}</h2>
        <div className="pd-toolbar">
          <div className="pd-segmented" role="group" aria-label={t('product.skills.filter')}>
            {(['all', 'builtin', 'mine', 'marketplace'] as SkillFilter[])
              .filter((id) => id === 'all' || counts[id] > 0)
              .map((id) => (
                <button type="button" key={id} aria-pressed={filter === id} onClick={() => setFilter(id)}>
                  {id === 'all' ? t('product.skills.filter.all') : t(`product.skills.filter.${id}` as MessageKey, { count: counts[id] })}
                </button>
              ))}
          </div>
          <label className="pd-search">
            <Icon name="search" size={14} />
            <span className="pd-sr-only">{t('product.skills.search')}</span>
            <input type="search" placeholder={t('product.skills.search')} value={search} onChange={(e) => setSearch(e.target.value)} />
          </label>
        </div>
        {listed.length > 0 ? (
          <ul className="pd-list">
            {listed.map((skill) => (
              <li key={`${skill.dcc_type}/${skill.name}`}>
                <DccLogo dccType={skill.dcc_type} name={dccName(skill.dcc_type)} size={20} />
                <div className="pd-list-text">
                  <div className="pd-list-title">
                    <h3 className="pd-mono-text">{skill.name}</h3>
                    {skill.source !== 'builtin' ? <Tag tone={SOURCE_TONE[skill.source]}>{t(`product.skills.source.${skill.source}` as MessageKey)}</Tag> : null}
                    {skill.loaded_on.length > 0 ? (
                      <Tag tone="ok" dot>{loadedTag(skill)}</Tag>
                    ) : null}
                  </div>
                  <span className="pd-list-desc pd-clamp" title={skill.description}>
                    {[skill.description, stage(skill.stage), skill.tool_count != null ? t('product.skills.tools', { count: skill.tool_count }) : null]
                      .filter(Boolean)
                      .join(' · ')}
                  </span>
                </div>
              </li>
            ))}
          </ul>
        ) : null}
        {library && listed.length === 0 && search.trim() ? <p className="pd-note">{t('product.skills.empty', { query: search.trim() })}</p> : null}
      </section>

      <section className="pd-section" aria-labelledby="pd-skill-mine">
        <div className="pd-section-head">
          <div>
            <h2 id="pd-skill-mine" className="pd-h2">{t('product.skills.mine')}</h2>
            <span className="pd-lead" style={{ fontSize: 13 }}>{t('product.skills.mine.lead')}</span>
          </div>
          {local ? (
            <button type="button" className="pd-btn secondary" disabled={busy || !library?.instances.length} onClick={() => void runReload()}>
              {reload.isPending ? t('product.skills.mine.reloading') : t('product.skills.mine.reload')}
            </button>
          ) : null}
        </div>
        {notice ? <p className={notice.tone === 'err' ? 'pd-error' : 'pd-note'} role="status">{notice.text}</p> : null}
        {!local ? (
          <>
            <div className="pd-banner info"><span>{t('product.skills.mine.localOnly')}</span></div>
            <Command text="meshy-dcc reload-skills" t={t} />
          </>
        ) : null}
        <ul className="pd-list">
          {library?.folders.map((entry) => (
            <li key={entry.path} className="pd-skill-folder">
              <div className="pd-list-text">
                <div className="pd-list-title">
                  <h3>{entry.dcc_type ? t('product.skills.mine.default', { app: dccName(entry.dcc_type) }) : t('product.skills.mine.custom')}</h3>
                </div>
                <span className="pd-list-desc pd-mono-text" title={entry.path}>{entry.path}</span>
              </div>
              <div className="pd-list-actions">
                <CopyButton text={entry.path} t={t} label={t('product.skills.mine.copyPath')} />
                {local && entry.custom_id != null ? (
                  <button type="button" className="pd-btn sm tertiary" disabled={busy}
                    onClick={() => void changeFolder({ remove: entry.custom_id! })}>
                    {t('product.skills.mine.remove')}
                  </button>
                ) : null}
              </div>
              <FolderSkills t={t} skills={entry.skills} />
            </li>
          ))}
        </ul>
        {local ? (
          <form className="pd-project-form pd-skill-add" onSubmit={(e) => {
            e.preventDefault();
            if (newFolder.trim()) void changeFolder({ add: newFolder.trim() });
          }}>
            <label className="pd-sr-only" htmlFor="pd-skill-folder">{t('product.skills.mine.addLabel')}</label>
            <input id="pd-skill-folder" className="pd-input pd-mono-text" value={newFolder}
              placeholder={t('product.skills.mine.addPlaceholder')} onChange={(e) => setNewFolder(e.target.value)} />
            <button type="submit" className="pd-btn secondary" disabled={!newFolder.trim() || busy}>
              {t('product.skills.mine.add')}
            </button>
          </form>
        ) : null}
        <div className="pd-card pd-skill-write">
          <div className="pd-list-title"><h3>{t('product.skills.write')}</h3></div>
          <p className="pd-note">{t('product.skills.write.hint')}</p>
          <div className="pd-code pd-prompt">
            <p>{t('product.skills.write.prompt', { app: dccName(target.dccType), folder: target.folder })}</p>
            <CopyButton text={t('product.skills.write.prompt', { app: dccName(target.dccType), folder: target.folder })} t={t} />
          </div>
        </div>
      </section>

      <MarketplaceMore t={t} local={local} library={library} />
    </div>
  );
}

function FolderSkills({ t, skills }: { t: T; skills: SkillLibrary['folders'][number]['skills'] }) {
  if (skills.length === 0) return <p className="pd-result pd-note">{t('product.skills.mine.empty')}</p>;
  return (
    <>
    <ul className="pd-result pd-skill-folder-skills">
      {skills.map((skill) => (
        <li key={skill.name}>
          <span className="pd-mono-text">{skill.name}</span>
          <Tag tone={STATE_TONE[skill.state]}>{t(`product.skills.state.${skill.state}` as MessageKey)}</Tag>
          <span className="pd-list-desc">{skill.description}</span>
        </li>
      ))}
    </ul>
    {skills.some((skill) => skill.state === 'pending') ? <p className="pd-result pd-note">{t('product.skills.mine.pendingHint')}</p> : null}
    </>
  );
}

/// Packages from the Meshy marketplace for the open apps; hidden while the
/// marketplace lists nothing or cannot be reached.
function MarketplaceMore({ t, local, library }: { t: T; local: boolean; library: SkillLibrary | undefined }) {
  const catalog = useMarketplaceCatalogQuery(true);
  const installed = useInstalledMarketplaceQuery(true);
  const install = useMarketplaceInstall();
  const client = useQueryClient();
  const [failure, setFailure] = useState<string | null>(null);
  const apps = new Set((library?.instances ?? []).map((instance) => instance.dcc_type));
  const entries = (catalog.data ?? []).filter((entry) => entry.dcc.some((dcc) => dcc === 'any' || apps.has(dcc)));
  if (entries.length === 0) return null;
  const isInstalled = (name: string) => installed.data?.some((pkg) => pkg.name === name) ?? false;

  const run = async (name: string, dcc: string) => {
    setFailure(null);
    try {
      await install.mutateAsync({ name, dcc });
      await client.invalidateQueries({ queryKey: ['product', 'skills'] });
    } catch (err) {
      setFailure(t('product.skills.more.failed', { name, reason: errorMessage(err) }));
    }
  };

  return (
    <section className="pd-section" aria-labelledby="pd-skill-more">
      <div>
        <h2 id="pd-skill-more" className="pd-h2">{t('product.skills.more')}</h2>
        <span className="pd-lead" style={{ fontSize: 13 }}>{t('product.skills.more.lead')}</span>
      </div>
      {failure ? <p className="pd-error" role="alert">{failure}</p> : null}
      <ul className="pd-list">
        {entries.map((entry) => {
          const dcc = entry.dcc.find((value) => apps.has(value)) ?? entry.dcc[0];
          return (
            <li key={entry.name}>
              <div className="pd-list-text">
                <div className="pd-list-title">
                  <h3 className="pd-mono-text">{entry.name}</h3>
                  {isInstalled(entry.name) ? <Tag tone="ok">{t('product.skills.more.installed')}</Tag> : null}
                </div>
                <span className="pd-list-desc">{[entry.description, entry.version].filter(Boolean).join(' · ')}</span>
              </div>
              {local && !isInstalled(entry.name) ? (
                <div className="pd-list-actions">
                  <button type="button" className="pd-btn primary" disabled={install.isPending} onClick={() => void run(entry.name, dcc)}>
                    {t('product.skills.more.install')}
                  </button>
                </div>
              ) : null}
            </li>
          );
        })}
      </ul>
    </section>
  );
}
