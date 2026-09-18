import { Component, type ErrorInfo, type ReactNode } from "react";
import { RotateCcw, TriangleAlert } from "lucide-react";
import { Button } from "@/components/ui/Button";

interface Props {
  children: ReactNode;
}

interface State {
  error: Error | null;
  info: string | null;
}

/**
 * The last thing between a rendering bug and a blank window.
 *
 * Without this, one thrown error unmounts the whole tree and leaves an empty frame with no menu,
 * no message and nothing to click — indistinguishable, to the person looking at it, from the app
 * having crashed. Nothing here can lose data: the frontend holds only what a scan found, and a
 * scan is repeatable.
 */
export class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null, info: null };

  static getDerivedStateFromError(error: Error): Partial<State> {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    // Goes to the log the desktop app writes, which is local and never leaves the machine.
    console.error("Prune interface error", error, info.componentStack);
    this.setState({ info: info.componentStack ?? null });
  }

  private reset = () => this.setState({ error: null, info: null });

  render() {
    const { error, info } = this.state;
    if (!error) return this.props.children;

    return (
      <div className="flex h-full w-full items-center justify-center bg-bg p-8">
        <div className="w-full max-w-[520px]">
          <div className="flex items-start gap-3">
            <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-danger-soft text-danger">
              <TriangleAlert size={18} />
            </div>
            <div className="min-w-0 flex-1">
              <h1 className="text-[16px] font-semibold tracking-tight">
                Something in the interface broke
              </h1>
              <p className="mt-1 text-[12.5px] text-fg-muted">
                Nothing was removed, and nothing was lost — a scan can simply be run again. Going
                back should be enough; if it is not, quit and reopen Prune.
              </p>
            </div>
          </div>

          <pre className="mt-4 max-h-[200px] overflow-auto rounded-md border border-line bg-surface px-3 py-2 font-mono text-[11.5px] whitespace-pre-wrap text-fg-muted">
            {error.message}
            {info ? `\n${info.trim()}` : ""}
          </pre>

          <div className="mt-4 flex justify-end">
            <Button variant="primary" onClick={this.reset}>
              <RotateCcw size={13} /> Go back
            </Button>
          </div>
        </div>
      </div>
    );
  }
}
