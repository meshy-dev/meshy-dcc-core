import { useState } from 'react';

import { errorMessage, useAgentSkill, useAgentsQuery } from './api';
import { agentEntries, guessHome, onePerFolder, tildePath, type AgentEntry } from './model';
import { AgentLogo, Command, Icon, Switch, Tag, type T } from './ui';

const GRID_PREVIEW = 12;

function sharedNote(entry: AgentEntry, t: T): string | null {
  if (entry.sharedWith.length === 0) return null;
  const names = entry.sharedWith.slice(0, 3).join(', ') + (entry.sharedWith.length > 3 ? '…' : '');
  return t('product.agents.shared', { names });
}

export function AgentsPanel({ t, local }: { t: T; local: boolean }) {
  const query = useAgentsQuery();
  const skill = useAgentSkill();
  const [search, setSearch] = useState('');
  const [showAll, setShowAll] = useState(false);
  const [pending, setPending] = useState<string | null>(null);
  const [failure, setFailure] = useState<string | null>(null);

  const entries = agentEntries(query.data);
  const home = guessHome(entries);
  const needle = search.trim().toLowerCase();
  const matching = entries.filter((e) => !needle || e.name.toLowerCase().includes(needle) || e.id.includes(needle));
  // Agents that share a folder share one switch: one card per folder found
  // here, and folders already on a card are not offered again below.
  const found = onePerFolder(matching.filter((e) => e.detected));
  const foundPaths = new Set(entries.filter((e) => e.detected).map((e) => e.path));
  const more = onePerFolder(matching.filter((e) => !e.detected && !foundPaths.has(e.path)));
  const moreShown = showAll || needle ? more : more.slice(0, GRID_PREVIEW);

  const change = async (entry: AgentEntry, on: boolean) => {
    setPending(entry.id);
    setFailure(null);
    try {
      const result = await skill.mutateAsync({ agents: [entry.id], remove: !on });
      const error = result.targets.find((target) => target.error)?.error;
      if (result.failed) setFailure(t('product.agents.failed', { agent: entry.name, reason: error ?? '' }));
    } catch (err) {
      setFailure(t('product.agents.failed', { agent: entry.name, reason: errorMessage(err) }));
    }
    setPending(null);
  };

  return (
    <div className="pd-page narrow">
      <div className="pd-page-head" style={{ alignItems: 'flex-end' }}>
        <div>
          <h1 className="pd-h1">{t('product.nav.agents')}</h1>
          <p className="pd-lead">{t('product.agents.lead')}</p>
        </div>
        <label className="pd-search">
          <Icon name="search" size={14} />
          <span className="pd-sr-only">{t('product.agents.search', { count: 70 })}</span>
          <input type="search" placeholder={t('product.agents.search', { count: Math.max(70, Math.floor(entries.length / 5) * 5) })}
            value={search} onChange={(e) => setSearch(e.target.value)} />
        </label>
      </div>

      {query.isError ? <p className="pd-error">{t('product.agents.loadFailed', { reason: errorMessage(query.error) })}</p> : null}
      {failure ? <p className="pd-error" role="alert">{failure}</p> : null}
      {!local ? (
        <>
          <div className="pd-banner info"><span>{t('product.agents.localOnly')}</span></div>
          <Command text="meshy-dcc agent-skill --execute" t={t} />
        </>
      ) : null}

      <section className="pd-section" aria-labelledby="pd-found-title">
        <h2 id="pd-found-title" className="pd-h2">{t('product.agents.found')}</h2>
        {found.length === 0 && !needle && query.isSuccess ? <p className="pd-note">{t('product.agents.foundEmpty')}</p> : null}
        <div className="pd-agent-cards">
          {found.map((entry) => (
            <article className="pd-agent-card" key={entry.id}>
              <AgentLogo id={entry.id} name={entry.name} size={28} />
              <div className="pd-list-text">
                <div className="pd-list-title">
                  <h3>{entry.name}</h3>
                  {entry.on ? <Tag tone="ok">{t('product.agents.on')}</Tag> : null}
                </div>
                <span className="pd-agent-path" title={entry.path}>
                  {[tildePath(entry.path, home), sharedNote(entry, t)].filter(Boolean).join(' · ')}
                </span>
                {entry.foreign ? <span className="pd-note">{t('product.agents.foreign')}</span> : null}
              </div>
              <Switch checked={entry.on} label={t('product.agents.switch', { agent: entry.name })}
                disabled={!local || entry.foreign || pending !== null} onChange={(next) => void change(entry, next)} />
            </article>
          ))}
        </div>
      </section>

      {more.length > 0 ? (
        <section className="pd-section" aria-labelledby="pd-more-title">
          <div className="pd-section-head">
            <h2 id="pd-more-title" className="pd-h2">{t('product.agents.more')}</h2>
            {!needle && more.length > GRID_PREVIEW ? (
              <button type="button" className="pd-btn sm tertiary" onClick={() => setShowAll((v) => !v)}>
                {showAll ? t('product.agents.showFewer') : t('product.agents.showAll', { count: more.length })}
              </button>
            ) : null}
          </div>
          <ul className="pd-agent-grid">
            {moreShown.map((entry) => (
              <li key={entry.id} title={tildePath(entry.path, home)}>
                <AgentLogo id={entry.id} name={entry.name} size={18} />
                <span className="pd-agent-name">{entry.name}</span>
                {entry.on ? <Tag tone="ok">{t('product.agents.on')}</Tag> : null}
                {local && !entry.foreign ? (
                  <button type="button" className="pd-btn sm accent" disabled={pending !== null}
                    onClick={() => void change(entry, !entry.on)}>
                    {entry.on ? t('product.agents.remove') : t('product.agents.add')}
                  </button>
                ) : null}
              </li>
            ))}
          </ul>
        </section>
      ) : null}

      {needle && matching.length === 0 ? <p className="pd-note">{t('product.agents.empty', { query: search.trim() })}</p> : null}

      <section className="pd-section" aria-labelledby="pd-custom-title">
        <div>
          <h2 id="pd-custom-title" className="pd-h2">{t('product.agents.custom')}</h2>
          <span className="pd-lead" style={{ fontSize: 13 }}>{t('product.agents.customHint')}</span>
        </div>
        <Command text={`meshy-dcc agent-skill --dir ${t('product.agents.customDir')} --execute`} t={t} />
      </section>
    </div>
  );
}
