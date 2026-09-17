/** Puts the module-level stores back to their initial data before each test. */
import { useApps } from "@/stores/apps";
import { useDisk } from "@/stores/disk";
import { useScan } from "@/stores/scan";
import { useStartup } from "@/stores/startup";
import { useSystem } from "@/stores/system";

export function resetStores() {
  useScan.setState({
    providers: [],
    scanId: null,
    scanning: false,
    progress: {},
    session: null,
    selected: new Set(),
    deleteMode: "trash",
    filter: "",
    riskFilter: "all",
    filterScope: null,
    plan: null,
    previewing: false,
    executing: false,
    cleanupProgress: null,
    result: null,
    error: null,
  });
  useDisk.setState({
    scanId: null,
    scanning: false,
    progress: null,
    summary: null,
    root: "",
    node: null,
    largeFiles: [],
    minBytes: 1_000_000_000,
    selected: new Set(),
    tab: "tree",
    error: null,
  });
  useApps.setState({
    apps: [],
    loading: false,
    measuring: false,
    progress: null,
    query: "",
    selectedId: null,
    detail: null,
    detailLoading: false,
    selectedItems: new Set(),
    error: null,
  });
  useStartup.setState({ items: [], loading: false, pending: new Set(), error: null });
  useSystem.setState({
    meta: null,
    info: null,
    permissions: null,
    snapshot: null,
    processes: [],
    error: null,
  });
}
