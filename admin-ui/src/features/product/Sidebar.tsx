import type { InstanceRow, Panel } from '../../admin-types';
import { DOCS_URL, MESHY_LOGO, PRODUCT_SHORT_NAME, WEBSITE_URL, dccName } from './catalog';
import { instanceTone, sceneLabel, type InstanceTone } from './model';
import { DccLogo, ExternalLink, Icon, type T } from './ui';

export type DevLink = { id: string; label: string; active: boolean; onSelect: () => void };

const TONE_DOT: Record<InstanceTone, string> = { ready: '', busy: '', starting: 'info', notResponding: 'warn' };

export function ProductSidebar(props: {
  t: T;
  panel: Panel;
  onNavigate: (panel: Panel) => void;
  reachable: boolean;
  local: boolean;
  version: string | undefined;
  instances: InstanceRow[];
  devEnabled: boolean;
  devOpen: boolean;
  onToggleDev: () => void;
  devLinks: DevLink[];
}) {
  const { t, panel, onNavigate } = props;
  const nav = (id: Panel, icon: 'overview' | 'connectors' | 'agents', label: string) => (
    <button type="button" className="pd-nav-item" aria-current={panel === id ? 'page' : undefined}
      onClick={() => onNavigate(id)}>
      <Icon name={icon} />
      {label}
    </button>
  );
  const statusText = props.reachable
    ? [t('product.status.running'), t(props.local ? 'product.status.local' : 'product.status.remote'), props.version]
      .filter(Boolean)
      .join(' · ')
    : t('product.status.unreachable');

  return (
    <nav className="pd-sidebar" aria-label={t('common.aria.adminNavigation')}>
      <div className="pd-brand">
        <img src={MESHY_LOGO} alt="Meshy" />
        <span className="pd-brand-rule" aria-hidden="true" />
        <span className="pd-brand-name">{PRODUCT_SHORT_NAME}</span>
      </div>
      <button type="button" className="pd-status-link" onClick={() => onNavigate('settings')}>
        <span className={props.reachable ? 'pd-dot' : 'pd-dot off'} />
        {statusText}
      </button>

      {nav('home', 'overview', t('product.nav.overview'))}
      {nav('connectors', 'connectors', t('product.nav.connectors'))}
      {nav('agents', 'agents', t('product.nav.agents'))}

      <div className="pd-nav-heading">
        <span>{t('product.nav.connectedNow')}</span>
        <span>{props.instances.length}</span>
      </div>
      {props.instances.length === 0 ? (
        <p className="pd-nav-empty">{t('product.nav.connectedEmpty')}</p>
      ) : (
        props.instances.map((row) => {
          const tone = instanceTone(row);
          const detail = tone === 'notResponding'
            ? t('product.instance.notResponding')
            : tone === 'starting'
              ? t('product.instance.starting')
              : sceneLabel(row) ?? t('product.overview.table.untitled');
          return (
            <button type="button" key={row.instance_id} className="pd-nav-instance pd-nav-item" onClick={() => onNavigate('home')}>
              <DccLogo dccType={row.dcc_type} name={dccName(row.dcc_type)} size={16} />
              <span className="pd-nav-instance-text">
                <span className="pd-nav-instance-name">{dccName(row.dcc_type)}</span>
                <span className={tone === 'notResponding' ? 'pd-nav-instance-scene warn' : 'pd-nav-instance-scene'}>{detail}</span>
              </span>
              <span className={`pd-dot ${TONE_DOT[tone]}`} />
            </button>
          );
        })
      )}

      {props.devEnabled ? (
        <>
          <button type="button" className="pd-nav-item pd-dev-toggle" aria-expanded={props.devOpen} onClick={props.onToggleDev}>
            <Icon name="developer" size={14} stroke={2} />
            <span className="pd-grow">{t('product.nav.developer')}</span>
            <Icon name={props.devOpen ? 'chevronDown' : 'chevronRight'} size={14} stroke={2} />
          </button>
          {props.devOpen ? (
            <div>
              {props.devLinks.map((link) => (
                <button type="button" key={link.id} className="pd-dev-link" aria-current={link.active ? 'page' : undefined}
                  onClick={link.onSelect}>
                  {link.label}
                </button>
              ))}
            </div>
          ) : null}
        </>
      ) : null}

      <div className="pd-sidebar-foot">
        <ExternalLink href={DOCS_URL} className="pd-nav-item small">
          <Icon name="docs" />
          <span className="pd-grow">{t('product.nav.docs')}</span>
          <Icon name="external" size={12} stroke={2} />
        </ExternalLink>
        <ExternalLink href={WEBSITE_URL} className="pd-nav-item small">
          <Icon name="website" />
          <span className="pd-grow">{t('product.nav.website')}</span>
          <Icon name="external" size={12} stroke={2} />
        </ExternalLink>
        <button type="button" className="pd-nav-item small" aria-current={panel === 'settings' ? 'page' : undefined}
          onClick={() => onNavigate('settings')}>
          <Icon name="settings" />
          {t('product.nav.settings')}
        </button>
      </div>
    </nav>
  );
}
