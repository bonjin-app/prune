import { useEffect } from "react";
import { Card, CardHeader } from "@/components/ui/Card";
import { Switch } from "@/components/ui/Switch";
import { useSettings } from "@/stores/settings";
import type { Platform } from "@/types/models";

/**
 * Prune's icon in the menu bar, and whether closing the window leaves Prune there. The icon
 * exists only while Prune runs; Prune never starts itself.
 */
export function MenuBar({ platform }: { platform: Platform | undefined }) {
  const view = useSettings((s) => s.view);
  const load = useSettings((s) => s.load);
  const saving = useSettings((s) => s.saving);
  const setMenuBar = useSettings((s) => s.setMenuBar);

  useEffect(() => {
    if (!view) void load();
  }, [view, load]);

  const place = platform === "windows" ? "notification area" : "menu bar";
  const icon = view?.settings.menuBarIcon ?? true;
  const keep = view?.settings.keepInMenuBar ?? false;

  return (
    <Card>
      <CardHeader
        title={platform === "windows" ? "Notification area" : "Menu bar"}
        subtitle={`Processor, memory, graphics, free space and battery at a glance from the ${place}, while Prune is open.`}
      />
      <div className="flex flex-col gap-3 px-4 pb-4 text-[12.5px]">
        <Row
          label={`Show Prune in the ${place}`}
          checked={icon}
          busy={saving}
          disabled={!view}
          onChange={(next) => void setMenuBar(next, keep)}
        />
        <Row
          label="Keep Prune there when its window is closed"
          hint="Quit it from the icon's menu. Prune never starts by itself."
          checked={icon && keep}
          busy={saving}
          disabled={!view || !icon}
          onChange={(next) => void setMenuBar(icon, next)}
        />
      </div>
    </Card>
  );
}

function Row({
  label,
  hint,
  checked,
  busy,
  disabled,
  onChange,
}: {
  label: string;
  hint?: string;
  checked: boolean;
  busy: boolean;
  disabled: boolean;
  onChange: (next: boolean) => void;
}) {
  return (
    <div className="flex items-center justify-between gap-4">
      <div>
        <div>{label}</div>
        {hint && <div className="text-[11.5px] text-fg-faint">{hint}</div>}
      </div>
      <Switch label={label} checked={checked} busy={busy} disabled={disabled} onChange={onChange} />
    </div>
  );
}
