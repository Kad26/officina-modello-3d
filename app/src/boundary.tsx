// If something in the interface throws while drawing (most likely: odd data
// from a plugin), show a small notice and keep the rest of the app alive,
// instead of an empty window.
import { Component, type ReactNode } from "react";

interface Props {
  children: ReactNode;
  /** what to show instead (default: a short notice) */
  fallback?: (reset: () => void) => ReactNode;
  /** told once when it catches something */
  onError?: (e: unknown) => void;
}

export class ErrorBoundary extends Component<Props, { failed: boolean }> {
  state = { failed: false };
  static getDerivedStateFromError() {
    return { failed: true };
  }
  componentDidCatch(e: unknown) {
    this.props.onError?.(e);
  }
  reset = () => this.setState({ failed: false });
  render() {
    if (!this.state.failed) return this.props.children;
    if (this.props.fallback) return this.props.fallback(this.reset);
    return (
      <div className="sheet-backdrop" role="alert">
        <div className="sheet ask">
          <b>Something went wrong showing that</b>
          <p className="muted small">Your model is safe. This window couldn't be drawn.</p>
          <div className="row gap">
            <button className="btn small" onClick={this.reset}>
              Close
            </button>
          </div>
        </div>
      </div>
    );
  }
}
