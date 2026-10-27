import { useState } from 'react';

import { useReport } from './api';
import type { ReportResult } from './model';
import { Command, ExternalLink, type T } from './ui';

const MAX_LENGTH = 4000;

/// Settings > Report a problem. Nothing leaves the computer until Send, and
/// Send is only offered for the exact text whose preview the user has seen.
export function ReportPanel({ t, local, onBack, onDebug }: {
  t: T;
  local: boolean;
  onBack: () => void;
  onDebug: () => void;
}) {
  const report = useReport();
  const [text, setText] = useState('');
  const [previewed, setPreviewed] = useState<{ summary: string; bytes: number; json: string } | null>(null);
  const [result, setResult] = useState<ReportResult | null>(null);
  const summary = text.trim();
  const ready = previewed?.summary === summary;
  const sending = report.isPending && report.variables?.send === true;

  const run = async (send: boolean) => {
    setResult(null);
    const outcome = await report.mutateAsync({ summary, send });
    if (outcome.status === 'preview') {
      setPreviewed({ summary, bytes: outcome.bytes, json: JSON.stringify(outcome.report, null, 2) });
      return;
    }
    // A report that may have arrived needs a new Preview before it can be sent again.
    if (send && outcome.status !== 'failed') setPreviewed(null);
    setResult(outcome);
  };

  return (
    <div className="pd-page settings">
      <div>
        <button type="button" className="pd-btn tertiary sm" onClick={onBack}>← {t('product.nav.settings')}</button>
      </div>
      <div className="pd-page-head">
        <div>
          <h1 className="pd-h1">{t('product.report.title')}</h1>
          <p className="pd-lead">{t('product.report.lead')}</p>
        </div>
      </div>

      {local ? (
        <section className="pd-card round pd-report">
          <label htmlFor="pd-report-text" className="pd-h2">{t('product.report.label')}</label>
          <textarea id="pd-report-text" className="pd-input pd-textarea" value={text} maxLength={MAX_LENGTH}
            onChange={(e) => setText(e.target.value)} />
          <div className="pd-between">
            <span className="pd-note">{t('product.report.includes')}</span>
            <span className="pd-note pd-report-count">{text.length} / {MAX_LENGTH}</span>
          </div>

          <div className="pd-report-actions">
            <button type="button" className="pd-btn secondary" disabled={!summary || report.isPending}
              onClick={() => void run(false)}>
              {t('product.report.preview')}
            </button>
            <button type="button" className="pd-btn primary"
              disabled={!ready || report.isPending}
              onClick={() => void run(true)}>
              {sending ? t('product.report.sending') : t('product.report.send')}
            </button>
          </div>

          {previewed && ready ? (
            <details className="pd-report-payload" open>
              <summary>{t('product.report.payload', { size: (previewed.bytes / 1024).toFixed(1) })}</summary>
              <pre className="pd-mono-text">{previewed.json}</pre>
            </details>
          ) : null}

          {result ? <ReportOutcome t={t} result={result} /> : null}
        </section>
      ) : (
        <section className="pd-section">
          <div className="pd-banner info"><span>{t('product.report.localOnly')}</span></div>
          <Command text={`meshy-dcc report --summary "<${t('product.report.placeholder')}>"`} t={t} />
        </section>
      )}

      <button type="button" className="pd-a pd-linkbtn" onClick={onDebug}>{t('product.report.debug')}</button>
    </div>
  );
}

function ReportOutcome({ t, result }: { t: T; result: ReportResult }) {
  if (result.status === 'sent') {
    return (
      <p className="pd-note" role="status">
        {t('product.report.sent', { id: result.report_id })}{' '}
        {/^https:\/\//.test(result.issues_url)
          ? <ExternalLink href={result.issues_url} className="pd-a">{t('product.report.issues')}</ExternalLink>
          : null}
      </p>
    );
  }
  if (result.status === 'preview') return null;
  return (
    <p className="pd-error" role="status">
      {t(result.status === 'unknown' ? 'product.report.unknown' : 'product.report.failed', { reason: result.error })}
    </p>
  );
}
