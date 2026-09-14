import type { LucideIcon } from "lucide-react";
import { PageHeader } from "./PageHeader";
import { EmptyState } from "./EmptyState";

export function PlaceholderView({
  icon,
  title,
  phase,
  description,
  bullets,
}: {
  icon: LucideIcon;
  title: string;
  phase: number;
  description: string;
  bullets: string[];
}) {
  return (
    <div>
      <PageHeader title={title} description={description} />
      <EmptyState
        icon={icon}
        title={`Coming in Phase ${phase}`}
        description={
          <ul className="mt-2 space-y-0.5 text-left">
            {bullets.map((b) => (
              <li key={b} className="flex gap-2">
                <span className="text-fg-faint">•</span>
                <span>{b}</span>
              </li>
            ))}
          </ul>
        }
      />
    </div>
  );
}
