//! Prune's icon in the menu bar (the notification area on Windows).
//!
//! It exists only while Prune runs, as part of the same process: there is no helper, nothing is
//! started at login, and Quit ends all of it. Its menu shows a few figures from the same monitor
//! the Monitor view uses, read when the pointer reaches the icon rather than on a timer, so an
//! icon nobody looks at costs nothing.
//!
//! With Settings → "Keep Prune in the menu bar when its window is closed", closing the window
//! hides it instead of quitting. That stays a choice the user makes; the default quits.

use std::sync::atomic::{AtomicBool, Ordering};

use prune_core::models::SystemSnapshot;
use tauri::menu::{Menu, MenuBuilder, MenuItemBuilder};
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Runtime};

use crate::AppState;

const TRAY: &str = "prune";
const OPEN: &str = "tray-open";
const QUIT: &str = "tray-quit";

/// Set while a menu is being rebuilt, so a pointer moving over the icon does not queue more.
static REFRESHING: AtomicBool = AtomicBool::new(false);

/// Shows or removes the icon to match the setting.
pub fn apply<R: Runtime>(app: &AppHandle<R>, show: bool) {
    let exists = app.tray_by_id(TRAY).is_some();
    if show && !exists {
        if let Err(e) = create(app) {
            tracing::warn!(error = %e, "could not show the menu bar icon");
        }
    } else if !show && exists {
        app.remove_tray_by_id(TRAY);
    }
}

/// Whether the icon is showing, so closing the window can leave Prune somewhere to be found.
pub fn is_shown<R: Runtime>(app: &AppHandle<R>) -> bool {
    app.tray_by_id(TRAY).is_some()
}

/// Brings the window back, and on macOS the Dock icon with it.
pub fn show_window<R: Runtime>(app: &AppHandle<R>) {
    #[cfg(target_os = "macos")]
    let _ = app.set_dock_visibility(true);
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// Hides the window instead of closing it. On macOS the Dock icon goes too, since the app is
/// now reached from the menu bar.
pub fn hide_window<R: Runtime>(window: &tauri::Window<R>) {
    let _ = window.hide();
    #[cfg(target_os = "macos")]
    let _ = window.app_handle().set_dock_visibility(false);
}

fn create<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let builder = TrayIconBuilder::with_id(TRAY)
        .tooltip("Prune")
        .menu(&menu(app, &[])?)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            OPEN => show_window(app),
            QUIT => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Enter { .. } = event {
                refresh(tray.app_handle());
            }
        });
    // A template image follows the menu bar's light or dark appearance. Windows has no such
    // thing, and a black glyph vanishes on a dark taskbar, so it gets the app's own icon.
    #[cfg(target_os = "macos")]
    let builder = builder
        .icon(tauri::image::Image::from_bytes(include_bytes!(
            "../icons/tray.png"
        ))?)
        .icon_as_template(true);
    #[cfg(not(target_os = "macos"))]
    let builder = match app.default_window_icon() {
        Some(icon) => builder.icon(icon.clone()),
        None => builder,
    };
    builder.build(app)?;
    refresh(app);
    Ok(())
}

/// Rebuilds the menu with fresh figures, off the main thread: reading them runs `ioreg` and
/// can take tens of milliseconds.
fn refresh<R: Runtime>(app: &AppHandle<R>) {
    if REFRESHING.swap(true, Ordering::AcqRel) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let lines = app
            .try_state::<AppState>()
            .map(|state| lines(&state.monitor.snapshot()))
            .unwrap_or_default();
        match (app.tray_by_id(TRAY), menu(&app, &lines)) {
            (Some(tray), Ok(menu)) => {
                let _ = tray.set_menu(Some(menu));
            }
            (_, Err(e)) => tracing::warn!(error = %e, "could not update the menu bar menu"),
            _ => {}
        }
        REFRESHING.store(false, Ordering::Release);
    });
}

fn menu<R: Runtime>(app: &AppHandle<R>, lines: &[String]) -> tauri::Result<Menu<R>> {
    let mut builder = MenuBuilder::new(app);
    for (i, line) in lines.iter().enumerate() {
        // Information, not actions: shown greyed out, as menu bar status lines are.
        let item = MenuItemBuilder::with_id(format!("tray-line-{i}"), line)
            .enabled(false)
            .build(app)?;
        builder = builder.item(&item);
    }
    if !lines.is_empty() {
        builder = builder.separator();
    }
    builder
        .text(OPEN, "Open Prune")
        .text(QUIT, "Quit Prune")
        .build()
}

/// The figures shown in the menu. Anything the system does not report is left out.
pub(crate) fn lines(s: &SystemSnapshot) -> Vec<String> {
    let mut lines = vec![
        format!("Processor {:.0}%", s.cpu.usage_percent),
        format!(
            "Memory {} of {} in use",
            bytes(s.memory.used_bytes),
            bytes(s.memory.total_bytes)
        ),
    ];
    if let Some(busy) = s.gpus.iter().find_map(|g| g.usage_percent) {
        lines.push(format!("Graphics {busy:.0}%"));
    }
    if let Some(disk) = s.disks.iter().find(|d| d.is_primary) {
        let name = if disk.name.is_empty() {
            &disk.mount_point
        } else {
            &disk.name
        };
        lines.push(format!("{} free on {name}", bytes(disk.available_bytes)));
    }
    if let Some(b) = &s.battery {
        let state = if b.charging {
            ", charging"
        } else if b.plugged_in {
            ", plugged in"
        } else {
            ""
        };
        lines.push(format!("Battery {:.0}%{state}", b.percent));
    }
    lines
}

/// Decimal units, as the rest of the interface, Finder and Windows Settings use.
fn bytes(n: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    if n < 1000 {
        return format!("{n} B");
    }
    let mut value = n as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    let digits = if value >= 100.0 {
        0
    } else if value >= 10.0 {
        1
    } else {
        2
    };
    format!("{value:.digits$} {}", UNITS[unit])
}

#[cfg(test)]
mod tests {
    use prune_core::models::{
        BatteryStatus, CpuStatus, DiskStatus, GpuStatus, MemoryStatus, NetworkStatus,
    };

    use super::*;

    fn snapshot() -> SystemSnapshot {
        SystemSnapshot {
            cpu: CpuStatus {
                usage_percent: 12.4,
                per_core_percent: vec![],
            },
            memory: MemoryStatus {
                total_bytes: 64_000_000_000,
                used_bytes: 18_200_000_000,
                available_bytes: 0,
                swap_total_bytes: 0,
                swap_used_bytes: 0,
            },
            disks: vec![DiskStatus {
                name: "Macintosh HD".into(),
                mount_point: "/".into(),
                file_system: "apfs".into(),
                total_bytes: 1_000_000_000_000,
                available_bytes: 220_000_000_000,
                is_removable: false,
                is_primary: true,
            }],
            network: NetworkStatus {
                down_bytes_per_sec: 0,
                up_bytes_per_sec: 0,
                total_received_bytes: 0,
                total_transmitted_bytes: 0,
            },
            uptime_seconds: 0,
            process_count: 0,
            battery: Some(BatteryStatus {
                percent: 99.0,
                plugged_in: true,
                charging: true,
                minutes_remaining: None,
                cycle_count: None,
                design_cycle_count: None,
                capacity_percent: None,
            }),
            temperatures: Default::default(),
            gpus: vec![GpuStatus {
                usage_percent: Some(7.0),
                ..Default::default()
            }],
        }
    }

    #[test]
    fn the_menu_reads_like_the_monitor() {
        assert_eq!(
            lines(&snapshot()),
            [
                "Processor 12%",
                "Memory 18.2 GB of 64.0 GB in use",
                "Graphics 7%",
                "220 GB free on Macintosh HD",
                "Battery 99%, charging",
            ]
        );
    }

    #[test]
    fn what_the_system_does_not_report_is_left_out() {
        let mut s = snapshot();
        s.battery = None;
        s.gpus = vec![GpuStatus::default()];
        s.disks.clear();
        assert_eq!(
            lines(&s),
            ["Processor 12%", "Memory 18.2 GB of 64.0 GB in use"]
        );
    }
}
