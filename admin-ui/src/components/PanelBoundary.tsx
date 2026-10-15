import { Component, type ReactNode } from 'react';

/// Keeps one broken panel from unmounting the whole app. Give it `key={panel}`
/// so moving to another page (or Back) clears the error.
export class PanelBoundary extends Component<{ message: (error: string) => string; children: ReactNode }, { error: string | null }> {
  state = { error: null as string | null };

  static getDerivedStateFromError(error: unknown) {
    return { error: error instanceof Error ? error.message : String(error) };
  }

  render() {
    if (this.state.error === null) return this.props.children;
    return <p className="empty" role="alert">{this.props.message(this.state.error)}</p>;
  }
}
