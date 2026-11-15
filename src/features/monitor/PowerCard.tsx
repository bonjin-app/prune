import { BatteryCharging, BatteryFull, Plug, Thermometer } from "lucide-react";
import { Card, CardHeader } from "@/components/ui/Card";
import { Meter } from "@/components/ui/Meter";
import { formatDuration, formatPercent } from "@/lib/format";
import type { BatteryStatus, Temperatures } from "@/types/models";

/**
 * Battery and temperatures, read-only. Anything the operating system does not report is shown
 * as unknown, never as zero: a desktop has no battery, and most Windows machines expose no
 * temperature sensors without elevated rights.
 */
export function PowerCard({
  battery,
  temperatures,
}: {
  battery: BatteryStatus | null | undefined;
  temperatures: Temperatures | undefined;
}) {
  const sensors: [string, number | null | undefined][] = [
    ["Processor", temperatures?.cpuCelsius],
    ["Battery", temperatures?.batteryCelsius],
    ["Storage", temperatures?.storageCelsius],
  ];
  const anySensor = sensors.some(([, v]) => v != null);

  return (
    <Card>
      <CardHeader
        title="Power and temperature"
        action={
          battery ? (
            <span className="text-[18px] font-semibold tnum">{formatPercent(battery.percent)}</span>
          ) : undefined
        }
      />
      <div className="space-y-3 px-4 pb-4 text-[12px]">
        {battery ? <BatteryRows battery={battery} /> : <p className="text-fg-faint">No battery.</p>}

        <div className="border-t border-line pt-3">
          {anySensor ? (
            <dl className="grid grid-cols-3 gap-2">
              {sensors.map(([label, value]) => (
                <div key={label}>
                  <dt className="flex items-center gap-1 text-fg-muted">
                    <Thermometer size={11} className="text-fg-faint" /> {label}
                  </dt>
                  <dd className="mt-0.5 tnum">{value != null ? `${Math.round(value)} °C` : "—"}</dd>
                </div>
              ))}
            </dl>
          ) : (
            <p className="text-fg-faint">This system does not report temperatures to Prune.</p>
          )}
        </div>
      </div>
    </Card>
  );
}

function BatteryRows({ battery }: { battery: BatteryStatus }) {
  const state = battery.charging
    ? "Charging"
    : battery.pluggedIn
      ? "Plugged in, not charging"
      : "On battery";
  const Icon = battery.charging ? BatteryCharging : battery.pluggedIn ? Plug : BatteryFull;
  const time =
    battery.minutesRemaining != null
      ? `${formatDuration(battery.minutesRemaining * 60)} ${battery.charging ? "until full" : "left"}`
      : null;

  return (
    <>
      <div>
        <div className="flex justify-between">
          <span className="flex items-center gap-1.5 text-fg-muted">
            <Icon size={12} /> {state}
          </span>
          <span className="tnum">{time ?? ""}</span>
        </div>
        <Meter value={battery.percent} className="mt-1" tone="info" height={5} />
      </div>
      <dl className="grid grid-cols-2 gap-2">
        <div>
          <dt className="text-fg-muted">Holds when full</dt>
          <dd className="mt-0.5 tnum">
            {battery.capacityPercent != null
              ? `about ${formatPercent(battery.capacityPercent)} of new`
              : "—"}
          </dd>
        </div>
        <div>
          <dt className="text-fg-muted">Charge cycles</dt>
          <dd className="mt-0.5 tnum">
            {battery.cycleCount != null
              ? battery.designCycleCount != null
                ? `${battery.cycleCount} of ${battery.designCycleCount}`
                : String(battery.cycleCount)
              : "—"}
          </dd>
        </div>
      </dl>
    </>
  );
}
