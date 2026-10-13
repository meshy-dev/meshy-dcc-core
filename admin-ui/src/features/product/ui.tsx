/// Small building blocks shared by the user-facing pages.

import { useState, type ReactNode } from 'react';

import type { InterpolationValues, MessageKey } from '../../i18n';
import { AGENT_LOGOS, DCC_LOGOS, type Logo } from './catalog';

export type T = (key: MessageKey, values?: InterpolationValues) => string;

// ── Icons (24px grid, drawn with currentColor) ───────────────────────────────

const PATHS = {
  overview: <><rect x="3" y="3" width="7" height="7" rx="1.5" /><rect x="14" y="3" width="7" height="7" rx="1.5" /><rect x="3" y="14" width="7" height="7" rx="1.5" /><rect x="14" y="14" width="7" height="7" rx="1.5" /></>,
  connectors: <><path d="M12 2 3 7v10l9 5 9-5V7z" /><path d="m3 7 9 5 9-5M12 12v10" /></>,
  agents: <><rect x="3" y="4" width="18" height="16" rx="2" /><path d="m7 9 3 3-3 3M13 15h4" /></>,
  docs: <><path d="M4 4.5A1.5 1.5 0 0 1 5.5 3H20v15H5.5A1.5 1.5 0 0 0 4 19.5z" /><path d="M4 19.5A1.5 1.5 0 0 0 5.5 21H20" /></>,
  website: <><circle cx="12" cy="12" r="9" /><path d="M3 12h18M12 3a14 14 0 0 1 0 18M12 3a14 14 0 0 0 0 18" /></>,
  settings: <><path d="M4 6h10M18 6h2M4 12h4M12 12h8M4 18h12" /><circle cx="16" cy="6" r="2" /><circle cx="10" cy="12" r="2" /><circle cx="18" cy="18" r="2" /></>,
  developer: <path d="m8 8-4 4 4 4M16 8l4 4-4 4" />,
  chevronRight: <path d="m9 6 6 6-6 6" />,
  chevronDown: <path d="m6 9 6 6 6-6" />,
  external: <path d="M7 17 17 7M8 7h9v9" />,
  search: <><circle cx="11" cy="11" r="7" /><path d="m20 20-4-4" /></>,
  copy: <><rect x="8" y="8" width="12" height="12" rx="2" /><path d="M16 8V5a1 1 0 0 0-1-1H5a1 1 0 0 0-1 1v10a1 1 0 0 0 1 1h3" /></>,
  check: <path d="m5 12 5 5 9-10" />,
  sparkle: <path d="M12 3v3M12 18v3M3 12h3M18 12h3M5.6 5.6l2.1 2.1M16.3 16.3l2.1 2.1M5.6 18.4l2.1-2.1M16.3 7.7l2.1-2.1" />,
} as const;

export type IconName = keyof typeof PATHS | 'bulb';

export function Icon({ name, size = 16, stroke = 1.8 }: { name: IconName; size?: number; stroke?: number }) {
  if (name === 'bulb') {
    return (
      <svg width={size} height={size} viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
        <path d="M12 2a7 7 0 0 0-4 12.74V17a1 1 0 0 0 1 1h6a1 1 0 0 0 1-1v-2.26A7 7 0 0 0 12 2zM9 20h6v1a1 1 0 0 1-1 1h-4a1 1 0 0 1-1-1z" />
      </svg>
    );
  }
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={stroke}
      strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      {PATHS[name]}
    </svg>
  );
}

// ── Logos ─────────────────────────────────────────────────────────────────────

function LogoImage({ logo, name, size }: { logo: Logo | undefined; name: string; size: number }) {
  const cls = `pd-logo s${size}`;
  if (!logo) {
    // No official logo: the initial stands in, so lists keep their rhythm.
    return (
      <span className="pd-avatar" style={{ width: size, height: size }} aria-hidden="true">
        {name.slice(0, 1).toUpperCase()}
      </span>
    );
  }
  return <img src={logo.src} alt="" className={logo.mono ? `${cls} pd-mono` : cls} />;
}

export function DccLogo({ dccType, name, size = 20 }: { dccType: string; name: string; size?: number }) {
  return <LogoImage logo={DCC_LOGOS[dccType]} name={name} size={size} />;
}

export function AgentLogo({ id, name, size = 20 }: { id: string; name: string; size?: number }) {
  return <LogoImage logo={AGENT_LOGOS[id]} name={name} size={size} />;
}

// ── Controls ──────────────────────────────────────────────────────────────────

export function Tag({ tone, children, dot = false }: { tone: 'ok' | 'warn' | 'info' | 'muted'; children: ReactNode; dot?: boolean }) {
  return (
    <span className={`pd-tag ${tone}`}>
      {dot ? <span className="pd-dot" /> : null}
      {children}
    </span>
  );
}

export function Switch({ checked, label, disabled, onChange }: {
  checked: boolean;
  label: string;
  disabled?: boolean;
  onChange: (next: boolean) => void;
}) {
  return (
    <button type="button" role="switch" className="pd-switch" aria-checked={checked} aria-label={label}
      disabled={disabled} onClick={() => onChange(!checked)}>
      <span />
    </button>
  );
}

async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
}

export function CopyButton({ text, t, label, size = 'sm', className = 'secondary' }: {
  text: string;
  t: T;
  label?: string;
  size?: 'sm' | 'md';
  className?: string;
}) {
  const [copied, setCopied] = useState(false);
  return (
    <button
      type="button"
      className={`pd-btn ${size === 'sm' ? 'sm' : ''} ${className}`}
      onClick={async () => {
        if (await copyText(text)) {
          setCopied(true);
          window.setTimeout(() => setCopied(false), 1600);
        }
      }}
    >
      <Icon name={copied ? 'check' : 'copy'} size={14} />
      {copied ? t('product.action.copied') : (label ?? t('product.action.copy'))}
    </button>
  );
}

/// A shell command the user can copy.
export function Command({ text, t }: { text: string; t: T }) {
  return (
    <div className="pd-code">
      <code>{text}</code>
      <CopyButton text={text} t={t} />
    </div>
  );
}

export function ExternalLink({ href, className, children }: { href: string; className?: string; children: ReactNode }) {
  return (
    <a href={href} className={className} target="_blank" rel="noopener noreferrer">
      {children}
    </a>
  );
}

/// "2 h 14 min", "5 min", "40 s".
export function formatDuration(secs: number, t: T): string {
  const s = Math.max(0, Math.floor(secs));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  if (h > 0) return t('product.time.hm', { h, m });
  if (m > 0) return t('product.time.m', { m });
  return t('product.time.s', { s });
}

/// Pages reached from the browser on another computer are read-only for changes.
export function isLocalPage(): boolean {
  const host = window.location.hostname.replace(/^\[|\]$/g, '');
  return host === 'localhost' || host.endsWith('.localhost') || host === '::1' || /^127\./.test(host);
}
