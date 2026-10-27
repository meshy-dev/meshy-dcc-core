import { useState } from 'react';

import type { HealthPayload } from '../../admin-types';
import { SUPPORTED_LOCALES, type SupportedLocale } from '../../i18n';
import { LOCALE_LABELS } from '../../locale';
import type { ThemeMode } from '../../theme';
import { useSetTelemetry, useSetUpdateCheck, useSettings, useUpdateCheck } from './api';
import { PRODUCT_NAME, WEBSITE_URL } from './catalog';
import { ReportPanel } from './ReportPanel';
import { Command, ExternalLink, Icon, Switch, Tag, type T } from './ui';

export function SettingsPanel(props: {
  t: T;
  health: HealthPayload | undefined;
  reachable: boolean;
  local: boolean;
  address: string;
  locale: SupportedLocale;
  onLocale: (locale: SupportedLocale) => void;
  theme: ThemeMode;
  onTheme: (mode: ThemeMode) => void;
  devEnabled: boolean;
  onDev: (on: boolean) => void;
  onDebug: () => void;
}) {
  const { t, health } = props;
  const version = health?.version;
  const update = useUpdateCheck(version);
  const settings = useSettings();
  const setUpdateCheck = useSetUpdateCheck();
  const setTelemetry = useSetTelemetry();
  const telemetryOffBy = settings.data?.telemetry_off_reason;
  const [remoteHelp, setRemoteHelp] = useState(false);
  const [reporting, setReporting] = useState(false);
  const network = health?.service?.network_listener;

  if (reporting) return <ReportPanel t={t} local={props.local} onBack={() => setReporting(false)} onDebug={props.onDebug} />;

  return (
    <div className="pd-page settings">
      <h1 className="pd-h1">{t('product.nav.settings')}</h1>

      <section className="pd-section" aria-labelledby="pd-general">
        <h2 id="pd-general" className="pd-h2 caps">{t('product.settings.general')}</h2>
        <div className="pd-card round pd-rows">
          <div className="pd-row">
            <label htmlFor="pd-lang">{t('product.settings.language')}</label>
            <select id="pd-lang" className="pd-select" value={props.locale}
              onChange={(e) => props.onLocale(e.target.value as SupportedLocale)}>
              {SUPPORTED_LOCALES.map((locale) => <option key={locale} value={locale}>{LOCALE_LABELS[locale]}</option>)}
            </select>
          </div>
          <div className="pd-row">
            <label htmlFor="pd-theme">{t('product.settings.appearance')}</label>
            <select id="pd-theme" className="pd-select" value={props.theme}
              onChange={(e) => props.onTheme(e.target.value as ThemeMode)}>
              <option value="dark">{t('product.settings.theme.dark')}</option>
              <option value="light">{t('product.settings.theme.light')}</option>
              <option value="system">{t('product.settings.theme.system')}</option>
            </select>
          </div>
        </div>
      </section>

      <section className="pd-section" aria-labelledby="pd-core">
        <h2 id="pd-core" className="pd-h2 caps">{PRODUCT_NAME}</h2>
        <div className="pd-card round pd-rows">
          <div className="pd-row">
            <div className="pd-row-text">
              <span>
                {t('product.settings.connection')}
                {props.reachable
                  ? <Tag tone="ok">{t('product.status.running')}</Tag>
                  : <Tag tone="muted">{t('product.status.unreachable')}</Tag>}
              </span>
              <span className="pd-note">
                {t(props.local ? 'product.settings.connection.local' : 'product.settings.connection.remote')}
                {' · '}<code className="pd-mono-text">{props.address}</code>{' · '}
                {network ? `${t('product.settings.network.on')} (${network})` : t('product.settings.network.off')}
              </span>
            </div>
            <button type="button" className="pd-btn secondary" aria-expanded={remoteHelp} onClick={() => setRemoteHelp((v) => !v)}>
              {t('product.settings.remote')}
            </button>
            {remoteHelp ? (
              <div className="pd-row-detail">
                <span className="pd-note">{t('product.settings.remoteHelp')}</span>
                <Command text="meshy-dcc gateway register --name studio http://192.168.1.20:19765" t={t} />
                <Command text="meshy-dcc gateway set studio && meshy-dcc ui" t={t} />
              </div>
            ) : null}
          </div>
          <div className="pd-row">
            <div className="pd-row-text">
              <span>{t('product.settings.version', { version: version ?? '—' })}</span>
              <span className="pd-note">
                {update.data && !update.data.update_available ? `${t('product.settings.upToDate')} · ` : ''}
                <ExternalLink href={t('product.link.releaseNotes')} className="pd-a">{t('product.settings.releaseNotes')}</ExternalLink>
              </span>
            </div>
            <button type="button" className="pd-btn secondary" disabled={update.isFetching || !version}
              onClick={() => void update.refetch()}>
              {update.isFetching ? t('product.installed.checking') : t('product.installed.check')}
            </button>
            {update.isError ? <p className="pd-row-detail pd-error">{t('product.installed.checkFailed')}</p> : null}
            {update.data?.update_available ? (
              <div className="pd-row-detail">
                <span className="pd-note">{t('product.settings.updateAvailable', { version: update.data.latest_version ?? '' })}</span>
                <Command text="meshy-dcc update apply" t={t} />
              </div>
            ) : null}
          </div>
          <div className="pd-row" style={{ paddingRight: 16 }}>
            <div className="pd-row-text">
              <span>{t('product.settings.dailyCheck')}</span>
              <span className="pd-note">{t('product.settings.dailyCheckHint')}</span>
            </div>
            <Switch checked={settings.data?.update_check ?? false} label={t('product.settings.dailyCheck')}
              disabled={!props.local || !settings.data || setUpdateCheck.isPending}
              onChange={(on) => setUpdateCheck.mutate(on)} />
            {setUpdateCheck.isError ? <p className="pd-row-detail pd-error">{t('product.settings.saveFailed')}</p> : null}
          </div>
          <div className="pd-row" style={{ paddingRight: 16 }}>
            <div className="pd-row-text">
              <span>{t('product.settings.telemetry')}</span>
              <span className="pd-note">
                {telemetryOffBy
                  ? t('product.settings.telemetryOffBy', { reason: telemetryOffBy })
                  : t('product.settings.telemetryHint')}
                {' '}
                <ExternalLink href={settings.data?.telemetry_docs ?? t('product.link.docs')} className="pd-a">
                  {t('product.settings.telemetryDetails')}
                </ExternalLink>
              </span>
            </div>
            <Switch checked={!telemetryOffBy && (settings.data?.telemetry ?? false)} label={t('product.settings.telemetry')}
              disabled={!props.local || !settings.data || Boolean(telemetryOffBy) || setTelemetry.isPending}
              onChange={(on) => setTelemetry.mutate({ telemetry: on })} />
            {setTelemetry.isError ? <p className="pd-row-detail pd-error">{t('product.settings.saveFailed')}</p> : null}
          </div>
        </div>
      </section>

      <section className="pd-section" aria-labelledby="pd-advanced">
        <h2 id="pd-advanced" className="pd-h2 caps">{t('product.settings.advanced')}</h2>
        <div className="pd-card round">
          <div className="pd-row" style={{ paddingRight: 16 }}>
            <div className="pd-row-text">
              <span>{t('product.settings.dev')}</span>
              <span className="pd-note">{t('product.settings.devHint')}</span>
            </div>
            <Switch checked={props.devEnabled} label={t('product.settings.dev')} onChange={props.onDev} />
          </div>
        </div>
      </section>

      <section className="pd-section" aria-labelledby="pd-help">
        <h2 id="pd-help" className="pd-h2 caps">{t('product.settings.help')}</h2>
        <div className="pd-card round pd-rows">
          <ExternalLink href={t('product.link.docs')} className="pd-row-link">
            {t('product.settings.docs')}<Icon name="external" size={12} stroke={2} />
          </ExternalLink>
          <ExternalLink href={WEBSITE_URL} className="pd-row-link">
            {t('product.nav.website')}<Icon name="external" size={12} stroke={2} />
          </ExternalLink>
          <button type="button" className="pd-row-link" onClick={() => setReporting(true)}>
            {t('product.settings.report')}<span className="pd-note">{t('product.settings.reportHint')}</span>
          </button>
        </div>
      </section>
    </div>
  );
}

/// First-run notice for usage telemetry, shown on this computer until the user
/// dismisses it or turns telemetry on or off.
export function TelemetryBanner({ t, local }: { t: T; local: boolean }) {
  const settings = useSettings();
  const setTelemetry = useSetTelemetry();
  if (!local || !settings.data?.telemetry_notice) return null;
  return (
    <div className="pd-banner info" role="status" style={{ marginBottom: 16 }}>
      <span>
        {t('product.telemetry.notice')}{' '}
        <ExternalLink href={settings.data.telemetry_docs ?? t('product.link.docs')} className="pd-a">
          {t('product.settings.telemetryDetails')}
        </ExternalLink>
      </span>
      <button type="button" className="pd-btn sm secondary" disabled={setTelemetry.isPending}
        onClick={() => setTelemetry.mutate({ telemetry: false })}>{t('product.telemetry.turnOff')}</button>
      <button type="button" className="pd-btn sm primary" disabled={setTelemetry.isPending}
        onClick={() => setTelemetry.mutate({ telemetry_notice_seen: true })}>{t('product.telemetry.ok')}</button>
    </div>
  );
}
