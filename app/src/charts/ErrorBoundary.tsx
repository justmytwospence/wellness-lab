import { Component, type ReactNode } from 'react';

export class ErrorBoundary extends Component<{ children: ReactNode; name?: string }, { error: Error | null }> {
  state = { error: null as Error | null };
  static getDerivedStateFromError(error: Error) {
    return { error };
  }
  render() {
    if (this.state.error) return <div className="card err" data-testid="error-boundary"><h2>Something broke{this.props.name ? ` in ${this.props.name}` : ''}</h2><pre className="mono">{this.state.error.message}</pre></div>;
    return this.props.children;
  }
}
