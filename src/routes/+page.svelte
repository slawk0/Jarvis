<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { 
    Plus, 
    Trash2, 
    Server, 
    Key, 
    Lock, 
    User, 
    Globe, 
    Settings, 
    ChevronRight, 
    Loader2, 
    AlertCircle,
    Activity,
    ArrowLeft,
    Columns2,
    Rows2,
    Grid2x2,
    Maximize2,
    X,
    SplitSquareHorizontal,
    SplitSquareVertical,
    ChevronDown,
    GripVertical,
    Star,
    Keyboard,
    HelpCircle,
    RefreshCw,
  } from 'lucide-svelte';
  import {
    canNavigateBack,
    getBackDescription,
    navigateBack,
  } from '$lib/backNavigation.svelte';
  import { get } from 'svelte/store';
  import { LL } from '$lib/i18n/i18n-svelte';
  import { getNavLabel, getNavLabels, TAB_IDS } from '$lib/i18n/nav';
  import { formatInvokeError } from '$lib/i18n/backendErrors';

  // Komponenty
  import Sidebar from '../components/Sidebar.svelte';
  import Dashboard from '../components/Dashboard.svelte';
  import FileManager from '../components/FileManager.svelte';
  import ServicesManager from '../components/ServicesManager.svelte';
  import CronManager from '../components/CronManager.svelte';
  import UserManager from '../components/UserManager.svelte';
  import FirewallManager from '../components/FirewallManager.svelte';
  import LogViewer from '../components/LogViewer.svelte';
  import TerminalView from '../components/TerminalView.svelte';
  import DockerManager from '../components/DockerManager.svelte';
  import CrowdsecManager from '../components/CrowdsecManager.svelte';
  import PangolinManager from '../components/PangolinManager.svelte';
  import MaintenanceManager from '../components/MaintenanceManager.svelte';
  import BackupManager from '../components/BackupManager.svelte';
  import NetworkManager from '../components/NetworkManager.svelte';
  import RunbookManager from '../components/RunbookManager.svelte';
  import DiskManager from '../components/DiskManager.svelte';
  import NginxProxyManager from '../components/NginxProxyManager.svelte';
  import ProcessManager from '../components/ProcessManager.svelte';
  import DatabaseManager from '../components/DatabaseManager.svelte';
  import EnvManager from '../components/EnvManager.svelte';
  import NetDiagManager from '../components/NetDiagManager.svelte';
  import TimerManager from '../components/TimerManager.svelte';
  import LogAnalysisManager from '../components/LogAnalysisManager.svelte';
  import { resetAlertCooldowns } from '$lib/alerts/monitor';
  import type { ServerProfile } from '$lib/admin/types';

  // ────────────── Types ──────────────

  type ContainerSession = { containerId: string; containerName: string; useSudo: boolean; shell: string };

  interface Pane {
    id: string;
    activeTab: string;
    terminalContainerSession: ContainerSession | null;
    terminalSessionId: string;
    visitedTabs: Record<string, boolean>;
    /** Per-tab component instances kept alive for this pane (keyed by tab id). */
    componentRefs: Record<string, any>;
    r1: number;
    c1: number;
    r2: number;
    c2: number;
  }

  type DropZone = 'center' | 'top' | 'bottom' | 'left' | 'right';

  // ────────────── Sidebar Tab Metadata ──────────────

  const navLabels = $derived(getNavLabels(get(LL)));

  // ────────────── Connection State ──────────────

  let isConnected = $state(false);
  let isConnecting = $state(false);
  let connectError = $state('');
  let serverStats = $state<any>(null);
  let currentHostname = $state('');
  let currentProfileId = $state('');
  let currentProfileLabel = $state('');
  let isSwitching = $state(false);
  let isOnline = $state(true);

  // ────────────── Sidebar State ──────────────

  let sidebarCollapsed = $state(false);

  // Load collapsed state from localStorage
  onMount(async () => {
    await loadProfiles();
    const saved = localStorage.getItem('jarvis-sidebar-collapsed');
    if (saved === 'true') sidebarCollapsed = true;

    // Auto-connect to default profile
    try {
      const defaultId = await invoke<string | null>('get_default_profile');
      if (defaultId && profiles.find((p) => p.id === defaultId)) {
        defaultProfileId = defaultId;
        await handleConnect(defaultId);
      }
    } catch (err) {
      console.error('Auto-connect failed:', err);
    }
  });

  $effect(() => {
    localStorage.setItem('jarvis-sidebar-collapsed', String(sidebarCollapsed));
  });

  let pingInterval: any = null;
  let isReconnecting = $state(false);

  async function handleReconnect() {
    if (isReconnecting || !currentProfileId) return;
    isReconnecting = true;
    connectError = '';
    try {
      const stats = await invoke<any>('connect_ssh', { profileId: currentProfileId });
      serverStats = stats;
      currentHostname = stats.hostname;
      isOnline = true;
      connectError = '';
    } catch (err: unknown) {
      connectError = formatInvokeError(err);
      isOnline = false;
    } finally {
      isReconnecting = false;
    }
  }

  $effect(() => {
    if (isConnected && !isSwitching && !isConnecting) {
      pingInterval = setInterval(async () => {
        if (isOnline) {
          try {
            await invoke('ping_ssh');
            isOnline = true;
          } catch (err) {
            console.error('Ping failed:', err);
            isOnline = false;
          }
        } else {
          // If we are offline, attempt to reconnect in the background
          if (!isReconnecting) {
            console.log('Server offline, attempting background reconnect...');
            await handleReconnect();
          }
        }
      }, 5000);
    } else {
      if (pingInterval) {
        clearInterval(pingInterval);
        pingInterval = null;
      }
      isOnline = true;
    }

    return () => {
      if (pingInterval) {
        clearInterval(pingInterval);
        pingInterval = null;
      }
    };
  });

  // ────────────── Pane / Workspace State ──────────────

  function createPane(tab: string = 'dashboard'): Pane {
    return {
      id: crypto.randomUUID(),
      activeTab: tab,
      terminalContainerSession: null,
      terminalSessionId: crypto.randomUUID(),
      visitedTabs: { [tab]: true },
      componentRefs: {},
      r1: 1, c1: 1, r2: 121, c2: 121,
    };
  }

  let panes = $state<Pane[]>([createPane()]);
  let activePaneId = $state(panes[0].id);

  /** Reload the focused pane's active tab via its component's exposed refresh(). */
  function refreshActiveTab() {
    const pane = panes.find((p) => p.id === activePaneId);
    pane?.componentRefs[pane.activeTab]?.refresh?.();
  }

  // Derived: the focused pane's activeTab (used by sidebar highlighting)
  let activeTab = $derived(panes.find(p => p.id === activePaneId)?.activeTab ?? 'dashboard');

  // History for back navigation
  let tabHistory = $state<string[]>([]);

  // ────────────── Drag & Drop State ──────────────

  let dragOverPaneId = $state<string | null>(null);
  let dragOverZone = $state<DropZone | null>(null);
  let isDraggingTab = $state(false);

  // ────────────── Custom Pointer Drag State ──────────────
  let customDragState = $state<{
    type: 'tab' | 'pane';
    id: string;
    startX: number;
    startY: number;
    currentX: number;
    currentY: number;
    active: boolean;
  } | null>(null);

  // ────────────── Resizing State ──────────────
  let resizeState = $state<{
    type: 'v' | 'h';
    line: number;
    lastLine: number;
    startClientXY: number;
  } | null>(null);

  let vResizers = $derived.by(() => {
    let cols = new Set<number>();
    panes.forEach(p => { if (p.c2 > 1 && p.c2 < 121) cols.add(p.c2); });
    return Array.from(cols).map(c => {
      let minR = 121, maxR = 1;
      panes.forEach(p => {
        if (p.c2 === c || p.c1 === c) {
          if (p.r1 < minR) minR = p.r1;
          if (p.r2 > maxR) maxR = p.r2;
        }
      });
      return { c, minR, maxR };
    });
  });

  let hResizers = $derived.by(() => {
    let rows = new Set<number>();
    panes.forEach(p => { if (p.r2 > 1 && p.r2 < 121) rows.add(p.r2); });
    return Array.from(rows).map(r => {
      let minC = 121, maxC = 1;
      panes.forEach(p => {
        if (p.r2 === r || p.r1 === r) {
          if (p.c1 < minC) minC = p.c1;
          if (p.c2 > maxC) maxC = p.c2;
        }
      });
      return { r, minC, maxC };
    });
  });

  // Layout mode controls the current preset logic
  type LayoutMode = 'single' | 'split-h' | 'split-v' | 'grid';
  let layoutMode = $state<LayoutMode>('single');

  // ────────────── Tab Navigation ──────────────

  function selectTab(tab: string) {
    const pane = panes.find(p => p.id === activePaneId);
    if (!pane) return;
    if (tab === pane.activeTab) return;
    tabHistory = [...tabHistory, pane.activeTab];
    // Only start a fresh terminal on the first visit; afterwards keep the live session alive.
    if (tab === 'terminal' && !pane.visitedTabs['terminal']) {
      pane.terminalContainerSession = null;
      pane.terminalSessionId = crypto.randomUUID();
    }
    pane.activeTab = tab;
    pane.visitedTabs[tab] = true;
  }

  function focusPane(paneId: string) {
    activePaneId = paneId;
  }

  // ────────────── Pane Operations ──────────────

  function splitPane(paneId: string, direction: 'h' | 'v', newTab?: string) {
    if (panes.length >= 4) return;
    const sourcePane = panes.find(p => p.id === paneId);
    if (!sourcePane) return;
    
    const tab = newTab || sourcePane.activeTab;
    const newPane = createPane(tab);
    
    if (direction === 'h') {
      const mid = Math.floor((sourcePane.c1 + sourcePane.c2) / 2);
      newPane.r1 = sourcePane.r1;
      newPane.r2 = sourcePane.r2;
      newPane.c1 = mid;
      newPane.c2 = sourcePane.c2;
      sourcePane.c2 = mid;
    } else {
      const mid = Math.floor((sourcePane.r1 + sourcePane.r2) / 2);
      newPane.c1 = sourcePane.c1;
      newPane.c2 = sourcePane.c2;
      newPane.r1 = mid;
      newPane.r2 = sourcePane.r2;
      sourcePane.r2 = mid;
    }
    
    panes = [...panes, newPane];
    activePaneId = newPane.id;
    layoutMode = panes.length === 2 ? (direction === 'v' ? 'split-v' : 'split-h') : 'grid';
  }

  function closePaneSpace(p: Pane) {
    let neighbor = panes.find(n => n.id !== p.id && n.c1 === p.c1 && n.c2 === p.c2 && (n.r1 === p.r2 || n.r2 === p.r1));
    if (!neighbor) {
      neighbor = panes.find(n => n.id !== p.id && n.r1 === p.r1 && n.r2 === p.r2 && (n.c1 === p.c2 || n.c2 === p.c1));
    }
    if (neighbor) {
      neighbor.r1 = Math.min(neighbor.r1, p.r1);
      neighbor.c1 = Math.min(neighbor.c1, p.c1);
      neighbor.r2 = Math.max(neighbor.r2, p.r2);
      neighbor.c2 = Math.max(neighbor.c2, p.c2);
    } else {
      const touching = panes.find(n => n.id !== p.id && 
        ((n.r2 === p.r1 || n.r1 === p.r2) && (n.c1 < p.c2 && n.c2 > p.c1)) ||
        ((n.c2 === p.c1 || n.c1 === p.c2) && (n.r1 < p.r2 && n.r2 > p.r1))
      );
      if (touching) {
        if (touching.r2 === p.r1) touching.r2 = p.r2;
        else if (touching.r1 === p.r2) touching.r1 = p.r1;
        else if (touching.c2 === p.c1) touching.c2 = p.c2;
        else if (touching.c1 === p.c2) touching.c1 = p.c1;
      }
    }
  }

  function closePane(paneId: string) {
    if (panes.length <= 1) return;
    const p = panes.find(p => p.id === paneId);
    if (!p) return;
    
    closePaneSpace(p);
    
    panes = panes.filter(x => x.id !== paneId);
    if (activePaneId === paneId) {
      activePaneId = panes[0].id;
    }
    if (panes.length === 1) layoutMode = 'single';
  }

  function setLayoutPreset(mode: LayoutMode) {
    if (mode === 'single') {
      const activePane = panes.find(p => p.id === activePaneId) || panes[0];
      activePane.r1 = 1; activePane.c1 = 1; activePane.r2 = 121; activePane.c2 = 121;
      panes = [activePane];
      activePaneId = activePane.id;
    } else if (mode === 'split-h') {
      const p1 = panes.find(p => p.id === activePaneId) || panes[0];
      p1.r1 = 1; p1.r2 = 121; p1.c1 = 1; p1.c2 = 61;
      let p2 = panes.find(p => p.id !== p1.id);
      if (!p2) p2 = createPane();
      p2.r1 = 1; p2.r2 = 121; p2.c1 = 61; p2.c2 = 121;
      panes = [p1, p2];
    } else if (mode === 'split-v') {
      const p1 = panes.find(p => p.id === activePaneId) || panes[0];
      p1.r1 = 1; p1.r2 = 61; p1.c1 = 1; p1.c2 = 121;
      let p2 = panes.find(p => p.id !== p1.id);
      if (!p2) p2 = createPane();
      p2.r1 = 61; p2.r2 = 121; p2.c1 = 1; p2.c2 = 121;
      panes = [p1, p2];
    } else if (mode === 'grid') {
      while (panes.length < 4) panes.push(createPane());
      panes = panes.slice(0, 4);
      panes[0].r1 = 1; panes[0].r2 = 61; panes[0].c1 = 1; panes[0].c2 = 61;
      panes[1].r1 = 1; panes[1].r2 = 61; panes[1].c1 = 61; panes[1].c2 = 121;
      panes[2].r1 = 61; panes[2].r2 = 121; panes[2].c1 = 1; panes[2].c2 = 61;
      panes[3].r1 = 61; panes[3].r2 = 121; panes[3].c1 = 61; panes[3].c2 = 121;
      if (!panes.find(p => p.id === activePaneId)) activePaneId = panes[0].id;
    }
    layoutMode = mode;
  }

  // ────────────── Drag & Drop Handlers ──────────────

  function startCustomDrag(e: PointerEvent, type: 'tab' | 'pane', id: string) {
    if (e.button !== 0) return;
    const target = e.target as HTMLElement;
    if (target.closest('.pane-action-btn') || target.closest('.pane-tab-selector')) return;
    
    try {
      if (target.hasPointerCapture(e.pointerId)) {
        target.releasePointerCapture(e.pointerId);
      }
    } catch (err) {}
    
    customDragState = {
      type,
      id,
      startX: e.clientX,
      startY: e.clientY,
      currentX: e.clientX,
      currentY: e.clientY,
      active: false
    };
    e.preventDefault();
  }

  function handleGlobalPointerMove(e: PointerEvent) {
    if (resizeState) {
      e.preventDefault();
      const rs = resizeState;
      const container = document.querySelector('.panes-grid') as HTMLElement;
      if (!container) return;
      const rect = container.getBoundingClientRect();
      
      if (rs.type === 'v') {
        let minAllowed = 2;
        let maxAllowed = 120;
        panes.forEach(p => {
          if (p.c2 === rs.lastLine && p.c1 + 15 > minAllowed) minAllowed = p.c1 + 15;
          if (p.c1 === rs.lastLine && p.c2 - 15 < maxAllowed) maxAllowed = p.c2 - 15;
        });
        
        let newPct = (e.clientX - rect.left) / rect.width;
        let newLine = Math.round(newPct * 120) + 1;
        newLine = Math.max(minAllowed, Math.min(newLine, maxAllowed));
        
        if (newLine !== rs.lastLine) {
          const oldLine = rs.lastLine;
          panes.forEach(p => {
            if (p.c2 === oldLine) p.c2 = newLine;
            if (p.c1 === oldLine) p.c1 = newLine;
          });
          resizeState.lastLine = newLine;
        }
      } else {
        let minAllowed = 2;
        let maxAllowed = 120;
        panes.forEach(p => {
          if (p.r2 === rs.lastLine && p.r1 + 15 > minAllowed) minAllowed = p.r1 + 15;
          if (p.r1 === rs.lastLine && p.r2 - 15 < maxAllowed) maxAllowed = p.r2 - 15;
        });
        
        let newPct = (e.clientY - rect.top) / rect.height;
        let newLine = Math.round(newPct * 120) + 1;
        newLine = Math.max(minAllowed, Math.min(newLine, maxAllowed));
        
        if (newLine !== rs.lastLine) {
          const oldLine = rs.lastLine;
          panes.forEach(p => {
            if (p.r2 === oldLine) p.r2 = newLine;
            if (p.r1 === oldLine) p.r1 = newLine;
          });
          resizeState.lastLine = newLine;
        }
      }
      return;
    }

    if (!customDragState) return;
    
    if (!customDragState.active) {
      const dist = Math.hypot(e.clientX - customDragState.startX, e.clientY - customDragState.startY);
      if (dist > 5) {
        customDragState.active = true;
        isDraggingTab = true;
      }
      return;
    }
    
    customDragState.currentX = e.clientX;
    customDragState.currentY = e.clientY;
    
    const target = e.target as HTMLElement;
    const paneEl = target.closest('.pane-container');
    
    if (paneEl) {
      const paneId = paneEl.getAttribute('data-pane-id');
      if (paneId) {
        dragOverPaneId = paneId;
        const dropZoneEl = target.closest('.drop-zone');
        if (dropZoneEl) {
          if (dropZoneEl.classList.contains('drop-zone-top')) dragOverZone = 'top';
          else if (dropZoneEl.classList.contains('drop-zone-bottom')) dragOverZone = 'bottom';
          else if (dropZoneEl.classList.contains('drop-zone-left')) dragOverZone = 'left';
          else if (dropZoneEl.classList.contains('drop-zone-right')) dragOverZone = 'right';
          else if (dropZoneEl.classList.contains('drop-zone-center')) dragOverZone = 'center';
        } else {
          dragOverZone = null;
        }
      }
    } else {
      dragOverPaneId = null;
      dragOverZone = null;
    }
  }

  function handleGlobalPointerUp(e: PointerEvent) {
    if (resizeState) {
      resizeState = null;
      return;
    }
    if (!customDragState) return;
    
    if (customDragState.active && dragOverPaneId && dragOverZone) {
      handleCustomDrop(dragOverPaneId, customDragState.type, customDragState.id, dragOverZone);
    }
    
    customDragState = null;
    dragOverPaneId = null;
    dragOverZone = null;
    isDraggingTab = false;
  }

  function handleCustomDrop(targetPaneId: string, type: 'tab' | 'pane', sourceId: string, zone: DropZone | null) {
    if (type === 'pane') {
      const sourcePaneId = sourceId;
      if (sourcePaneId === targetPaneId) return;
      
      const sourcePane = panes.find(p => p.id === sourcePaneId);
      const targetPane = panes.find(p => p.id === targetPaneId);
      if (!sourcePane || !targetPane) return;

      if (zone === 'center') {
        const sr1 = sourcePane.r1, sc1 = sourcePane.c1, sr2 = sourcePane.r2, sc2 = sourcePane.c2;
        sourcePane.r1 = targetPane.r1; sourcePane.c1 = targetPane.c1;
        sourcePane.r2 = targetPane.r2; sourcePane.c2 = targetPane.c2;
        targetPane.r1 = sr1; targetPane.c1 = sc1;
        targetPane.r2 = sr2; targetPane.c2 = sc2;
        
        activePaneId = targetPane.id;
        panes = [...panes];
      } else {
        const direction = (zone === 'left' || zone === 'right') ? 'h' : 'v';
        closePaneSpace(sourcePane);
        
        if (direction === 'h') {
          const mid = Math.floor((targetPane.c1 + targetPane.c2) / 2);
          sourcePane.r1 = targetPane.r1;
          sourcePane.r2 = targetPane.r2;
          if (zone === 'left') {
            sourcePane.c1 = targetPane.c1;
            sourcePane.c2 = mid;
            targetPane.c1 = mid;
          } else {
            sourcePane.c1 = mid;
            sourcePane.c2 = targetPane.c2;
            targetPane.c2 = mid;
          }
        } else {
          const mid = Math.floor((targetPane.r1 + targetPane.r2) / 2);
          sourcePane.c1 = targetPane.c1;
          sourcePane.c2 = targetPane.c2;
          if (zone === 'top') {
            sourcePane.r1 = targetPane.r1;
            sourcePane.r2 = mid;
            targetPane.r1 = mid;
          } else {
            sourcePane.r1 = mid;
            sourcePane.r2 = targetPane.r2;
            targetPane.r2 = mid;
          }
        }
        panes = [...panes];
        activePaneId = sourcePane.id;
      }
    } else if (type === 'tab') {
      const tabId = sourceId;
      const targetPane = panes.find(p => p.id === targetPaneId);
      if (!targetPane) return;
      
      if (zone === 'center') {
        if (tabId === 'terminal' && !targetPane.visitedTabs['terminal']) {
          targetPane.terminalContainerSession = null;
          targetPane.terminalSessionId = crypto.randomUUID();
        }
        targetPane.activeTab = tabId;
        targetPane.visitedTabs[tabId] = true;
        activePaneId = targetPaneId;
      } else if (zone) {
        if (panes.length >= 4) return;
        const direction = (zone === 'left' || zone === 'right') ? 'h' : 'v';
        const newPane = createPane(tabId);
        
        if (direction === 'h') {
          const mid = Math.floor((targetPane.c1 + targetPane.c2) / 2);
          newPane.r1 = targetPane.r1;
          newPane.r2 = targetPane.r2;
          if (zone === 'left') {
            newPane.c1 = targetPane.c1;
            newPane.c2 = mid;
            targetPane.c1 = mid;
          } else {
            newPane.c1 = mid;
            newPane.c2 = targetPane.c2;
            targetPane.c2 = mid;
          }
        } else {
          const mid = Math.floor((targetPane.r1 + targetPane.r2) / 2);
          newPane.c1 = targetPane.c1;
          newPane.c2 = targetPane.c2;
          if (zone === 'top') {
            newPane.r1 = targetPane.r1;
            newPane.r2 = mid;
            targetPane.r1 = mid;
          } else {
            newPane.r1 = mid;
            newPane.r2 = targetPane.r2;
            targetPane.r2 = mid;
          }
        }
        panes = [...panes, newPane];
        activePaneId = newPane.id;
      }
    }
  }

  // ────────────── Pane Tab Dropdown ──────────────
  
  let paneSelectorOpen = $state<string | null>(null);

  function togglePaneSelector(paneId: string) {
    paneSelectorOpen = paneSelectorOpen === paneId ? null : paneId;
  }

  function selectPaneTab(paneId: string, tabId: string) {
    const pane = panes.find(p => p.id === paneId);
    if (!pane) return;
    if (tabId === 'terminal' && !pane.visitedTabs['terminal']) {
      pane.terminalContainerSession = null;
      pane.terminalSessionId = crypto.randomUUID();
    }
    pane.activeTab = tabId;
    pane.visitedTabs[tabId] = true;
    paneSelectorOpen = null;
    activePaneId = paneId;
  }

  // ────────────── Keyboard Shortcuts ──────────────
  let showShortcutsModal = $state(false);

  function cycleTab(direction: number) {
    const pane = panes.find(p => p.id === activePaneId);
    if (!pane) return;
    const currentIdx = TAB_IDS.indexOf(pane.activeTab as any);
    if (currentIdx !== -1) {
      let nextIdx = (currentIdx + direction) % TAB_IDS.length;
      if (nextIdx < 0) nextIdx += TAB_IDS.length;
      selectTab(TAB_IDS[nextIdx]);
    }
  }

  function handleGlobalKeyDown(e: KeyboardEvent) {
    if (e.key === 'Escape' && showShortcutsModal) {
      showShortcutsModal = false;
      return;
    }

    const target = e.target as HTMLElement;
    if (target && (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.contentEditable === 'true')) {
      // Let standard inputs function normally
      return;
    }

    // Ctrl + N -> New pane split
    if (e.ctrlKey && !e.shiftKey && e.key.toLowerCase() === 'n') {
      e.preventDefault();
      splitPane(activePaneId, 'h');
    }
    // Ctrl + W -> Close active pane
    else if (e.ctrlKey && !e.shiftKey && e.key.toLowerCase() === 'w') {
      e.preventDefault();
      closePane(activePaneId);
    }
    // Ctrl + (1-4) -> Switch focus to pane 1..4
    else if (e.ctrlKey && !e.shiftKey && ['1', '2', '3', '4'].includes(e.key)) {
      e.preventDefault();
      const idx = parseInt(e.key) - 1;
      if (panes[idx]) {
        focusPane(panes[idx].id);
      }
    }
    // Ctrl + Shift + T -> Open terminal in active pane
    else if (e.ctrlKey && e.shiftKey && e.key.toLowerCase() === 't') {
      e.preventDefault();
      selectTab('terminal');
    }
    // Ctrl + Alt + B -> Toggle sidebar collapse
    else if (e.ctrlKey && e.altKey && e.key.toLowerCase() === 'b') {
      e.preventDefault();
      sidebarCollapsed = !sidebarCollapsed;
    }
    // Ctrl + Tab -> Cycle tabs forward
    else if (e.ctrlKey && !e.shiftKey && e.key === 'Tab') {
      e.preventDefault();
      cycleTab(1);
    }
    // Ctrl + Shift + Tab -> Cycle tabs backward
    else if (e.ctrlKey && e.shiftKey && e.key === 'Tab') {
      e.preventDefault();
      cycleTab(-1);
    }
    // Ctrl + Shift + H -> Show cheat sheet modal
    else if (e.ctrlKey && e.shiftKey && e.key.toLowerCase() === 'h') {
      e.preventDefault();
      showShortcutsModal = !showShortcutsModal;
    }
  }

  // ────────────── Back Navigation ──────────────

  function canGoBackGlobal(): boolean {
    if (!isConnected) return showCreateProfile;
    return canNavigateBack() || tabHistory.length > 0;
  }

  function getBackTooltip(): string {
    if (!isConnected) {
      return showCreateProfile
        ? get(LL).shell.backToProfileList()
        : get(LL).shell.backNoAction();
    }
    const handler = getBackDescription();
    if (handler) return handler;
    if (tabHistory.length > 0) {
      const prev = tabHistory[tabHistory.length - 1];
      const label = getNavLabel(get(LL), prev);
      return get(LL).shell.backToTab({ label });
    }
    return get(LL).shell.backNoAction();
  }

  function performBack(): boolean {
    if (!isConnected) {
      if (showCreateProfile) {
        showCreateProfile = false;
        currentProfileId = '';
        return true;
      }
      return false;
    }

    if (navigateBack()) return true;

    if (tabHistory.length > 0) {
      const prev = tabHistory[tabHistory.length - 1];
      tabHistory = tabHistory.slice(0, -1);
      const pane = panes.find(p => p.id === activePaneId);
      if (pane) {
        const firstTerminalVisit = prev === 'terminal' && !pane.visitedTabs['terminal'];
        pane.activeTab = prev;
        pane.visitedTabs[prev] = true;
        if (firstTerminalVisit) {
          pane.terminalContainerSession = null;
          pane.terminalSessionId = crypto.randomUUID();
        }
      }
      return true;
    }

    return false;
  }

  function handleMouseBack(event: MouseEvent) {
    if (event.button !== 3) return;
    if (!canGoBackGlobal()) return;
    event.preventDefault();
    performBack();
  }

  function handleAuxClick(event: MouseEvent) {
    if (event.button !== 3) return;
    if (!canGoBackGlobal()) return;
    event.preventDefault();
    performBack();
  }

  // ────────────── Profile State ──────────────

  let profiles = $state<ServerProfile[]>([]);
  let showCreateProfile = $state(false);
  
  let profileLabel = $state('');
  let profileHost = $state('');
  let profilePort = $state(22);
  let profileUsername = $state('root');
  let profileAuthType = $state('password');
  let profileKeyPath = $state('');
  let profilePassword = $state('');
  let profileKeyPassphrase = $state('');
  let defaultProfileId = $state('');

  async function loadProfiles() {
    try {
      profiles = await invoke('get_profiles');
    } catch (err) {
      console.error(get(LL).profile.loadFailed({ error: formatInvokeError(err) }));
    }
  }

  async function handleSetDefault(profileId: string, event: Event) {
    event.stopPropagation();
    const newDefault = profileId === defaultProfileId ? '' : profileId;
    try {
      await invoke('set_default_profile', { profileId: newDefault });
      defaultProfileId = newDefault;
    } catch (err: unknown) {
      console.error('Failed to set default profile:', err);
    }
  }

  async function handleSaveProfile() {
    if (!profileLabel || !profileHost || !profileUsername) {
      alert(get(LL).profile.requiredFields());
      return;
    }

    const newProfile = {
      id: currentProfileId || Date.now().toString(),
      label: profileLabel,
      host: profileHost,
      port: Number(profilePort),
      username: profileUsername,
      auth_type: profileAuthType,
      key_path: profileAuthType === 'key' ? profileKeyPath : null
    };

    try {
      await invoke('save_profile', {
        profile: newProfile,
        password: profilePassword ? profilePassword : null,
        keyPassphrase: profileKeyPassphrase ? profileKeyPassphrase : null
      });

      profileLabel = '';
      profileHost = '';
      profilePort = 22;
      profileUsername = 'root';
      profileAuthType = 'password';
      profileKeyPath = '';
      profilePassword = '';
      profileKeyPassphrase = '';
      currentProfileId = '';
      showCreateProfile = false;

      await loadProfiles();
    } catch (err: unknown) {
      alert(get(LL).profile.saveFailed({ error: formatInvokeError(err) }));
    }
  }

  async function handleConnect(profileId: string) {
    isConnecting = true;
    connectError = '';
    currentProfileId = profileId;
    
    try {
      const stats = await invoke<any>('connect_ssh', { profileId });
      serverStats = stats;
      currentHostname = stats.hostname;
      const prof = profiles.find((p) => p.id === profileId);
      currentProfileLabel = prof?.label || stats.hostname;
      resetAlertCooldowns();
      // Reset workspace to a single pane
      const initialPane = createPane('dashboard');
      panes = [initialPane];
      activePaneId = initialPane.id;
      layoutMode = 'single';
      tabHistory = [];
      isConnected = true;
      isOnline = true;
    } catch (err: unknown) {
      connectError = formatInvokeError(err);
      isOnline = false;
    } finally {
      isConnecting = false;
    }
  }

  async function handleSwitchProfile(profileId: string) {
    if (profileId === currentProfileId || isSwitching) return;
    isSwitching = true;
    connectError = '';
    try {
      const stats = await invoke<any>('switch_ssh', { profileId });
      currentProfileId = profileId;
      serverStats = stats;
      currentHostname = stats.hostname;
      const prof = profiles.find((p) => p.id === profileId);
      currentProfileLabel = prof?.label || stats.hostname;
      resetAlertCooldowns();
      isOnline = true;
    } catch (err: unknown) {
      connectError = formatInvokeError(err);
      isOnline = false;
    } finally {
      isSwitching = false;
    }
  }

  async function handleDisconnect() {
    try {
      await invoke('disconnect_ssh');
    } catch (err) {
      console.error(err);
    } finally {
      isConnected = false;
      serverStats = null;
      currentHostname = '';
      currentProfileLabel = '';
      tabHistory = [];
      const initialPane = createPane('dashboard');
      panes = [initialPane];
      activePaneId = initialPane.id;
      layoutMode = 'single';
    }
  }

  async function handleDeleteProfile(id: string, event: Event) {
    event.stopPropagation();
    if (confirm(get(LL).profile.confirmDelete())) {
      try {
        await invoke('delete_profile', { id });
        await loadProfiles();
      } catch (err: unknown) {
        alert(formatInvokeError(err));
      }
    }
  }

  function editProfile(profile: any, event: Event) {
    event.stopPropagation();
    currentProfileId = profile.id;
    profileLabel = profile.label;
    profileHost = profile.host;
    profilePort = profile.port;
    profileUsername = profile.username;
    profileAuthType = profile.auth_type;
    profileKeyPath = profile.key_path || '';
    profilePassword = '';
    profileKeyPassphrase = '';
    showCreateProfile = true;
  }
</script>

<svelte:window onkeydown={handleGlobalKeyDown} onmouseup={handleMouseBack} onauxclick={handleAuxClick} onpointermove={handleGlobalPointerMove} onpointerup={handleGlobalPointerUp} onclick={() => { paneSelectorOpen = null; }} />

<main class="app-container">
  {#if !isConnected && showCreateProfile}
    <button
      class="global-back-btn secondary btn-icon-compact"
      type="button"
      disabled={!canGoBackGlobal()}
      onclick={() => performBack()}
      aria-label={$LL.shell.globalBackAria()}
    >
      <ArrowLeft size={16} />
      <span class="back-tooltip" role="tooltip">{getBackTooltip()}</span>
    </button>
  {/if}

  {#if isConnected}
    <!-- MAIN APPLICATION WORKSPACE -->
    <Sidebar
      activeTab={activeTab}
      bind:collapsed={sidebarCollapsed}
      hostname={currentHostname}
      profiles={profiles}
      currentProfileId={currentProfileId}
      isSwitching={isSwitching}
      isOnline={isOnline}
      isReconnecting={isReconnecting}
      onReconnect={handleReconnect}
      onSwitchProfile={handleSwitchProfile}
      onDisconnect={handleDisconnect}
      onTabSelect={(tab: string) => {
        selectTab(tab);
      }}
      onCustomDragStart={(e: PointerEvent, type: 'tab' | 'pane', id: string) => startCustomDrag(e, type, id)}
    />
    
    <div class="main-content">
      {#if connectError && isConnected}
        <div class="connect-error-bar">
          <AlertCircle size={16} />
          <span>{connectError}</span>
        </div>
      {/if}

      <!-- Workspace Control Bar -->
      <div class="workspace-bar">
        <div class="workspace-bar-left">
          {#if panes.length === 1}
            <span class="workspace-label">{navLabels[activeTab] ?? activeTab}</span>
          {:else}
            <span class="workspace-label">{$LL.shell.workspaceLabel()}</span>
            <span class="workspace-pane-count">{panes.length} {panes.length === 1 ? $LL.shell.workspacePaneCountOne() : panes.length < 5 ? $LL.shell.workspacePaneCountFew() : $LL.shell.workspacePaneCountMany()}</span>
          {/if}
        </div>
        <div class="workspace-bar-right">
          <button
            class="layout-btn"
            onclick={refreshActiveTab}
            title={$LL.shell.refresh()}
            aria-label={$LL.shell.refresh()}
          >
            <RefreshCw size={14} />
          </button>

          <div class="actions-divider"></div>

          <button
            class="layout-btn"
            class:active={layoutMode === 'single'}
            onclick={() => setLayoutPreset('single')}
            title={$LL.shell.layoutSingle()}
          >
            <Maximize2 size={14} />
          </button>
          <button
            class="layout-btn"
            class:active={layoutMode === 'split-h'}
            onclick={() => setLayoutPreset('split-h')}
            title={$LL.shell.layoutSplitHorizontal()}
          >
            <Columns2 size={14} />
          </button>
          <button
            class="layout-btn"
            class:active={layoutMode === 'split-v'}
            onclick={() => setLayoutPreset('split-v')}
            title={$LL.shell.layoutSplitVertical()}
          >
            <Rows2 size={14} />
          </button>
          <button
            class="layout-btn"
            class:active={layoutMode === 'grid'}
            onclick={() => setLayoutPreset('grid')}
            title={$LL.shell.layoutGrid()}
          >
            <Grid2x2 size={14} />
          </button>

          <button
            class="layout-btn"
            class:active={showShortcutsModal}
            onclick={() => showShortcutsModal = !showShortcutsModal}
            title="Keyboard Shortcuts (Ctrl+Shift+H)"
            aria-label="Keyboard Shortcuts"
          >
            <Keyboard size={14} />
          </button>

          <div class="actions-divider"></div>

          <button
            class="layout-btn"
            disabled={!canGoBackGlobal()}
            onclick={() => performBack()}
            title={getBackTooltip()}
            aria-label={$LL.shell.globalBackAria()}
          >
            <ArrowLeft size={14} />
          </button>
        </div>
      </div>

      <!-- Panes Grid -->
      <div
        class="panes-grid"
        class:multi={panes.length > 1}
      >
        {#each vResizers as r}
          <div class="resizer resizer-v"
               style="position: absolute; left: {((r.c - 1) / 120) * 100}%; top: {((r.minR - 1) / 120) * 100}%; height: {((r.maxR - r.minR) / 120) * 100}%; width: 10px; margin-left: -5px; cursor: col-resize; z-index: 50;"
               onpointerdown={(e) => { e.preventDefault(); e.stopPropagation(); resizeState = { type: 'v', line: r.c, lastLine: r.c, startClientXY: e.clientX }; }}
               aria-label={$LL.shell.resizeWidth()}
               role="separator"
          ></div>
        {/each}
        {#each hResizers as r}
          <div class="resizer resizer-h"
               style="position: absolute; top: {((r.r - 1) / 120) * 100}%; left: {((r.minC - 1) / 120) * 100}%; width: {((r.maxC - r.minC) / 120) * 100}%; height: 10px; margin-top: -5px; cursor: row-resize; z-index: 50;"
               onpointerdown={(e) => { e.preventDefault(); e.stopPropagation(); resizeState = { type: 'h', line: r.r, lastLine: r.r, startClientXY: e.clientY }; }}
               aria-label={$LL.shell.resizeHeight()}
               role="separator"
          ></div>
        {/each}

        {#snippet paneTabContent(pane: Pane, tabId: string)}
          {@const visible = pane.activeTab === tabId}
          {#if tabId === 'dashboard'}
            <Dashboard
              bind:this={pane.componentRefs[tabId]}
              {visible}
              initialStats={serverStats}
              profileId={currentProfileId}
              profileLabel={currentProfileLabel}
            />
          {:else if tabId === 'maintenance'}
            <MaintenanceManager bind:this={pane.componentRefs[tabId]} {visible} onDisconnect={handleDisconnect} />
          {:else if tabId === 'backups'}
            <BackupManager bind:this={pane.componentRefs[tabId]} {visible} profileId={currentProfileId} />
          {:else if tabId === 'network'}
            <NetworkManager bind:this={pane.componentRefs[tabId]} {visible} />
          {:else if tabId === 'runbooks'}
            <RunbookManager bind:this={pane.componentRefs[tabId]} {visible} profileId={currentProfileId} />
          {:else if tabId === 'files'}
            <FileManager bind:this={pane.componentRefs[tabId]} {visible} profileId={currentProfileId} />
          {:else if tabId === 'disks'}
            <DiskManager bind:this={pane.componentRefs[tabId]} {visible} profileId={currentProfileId} />
          {:else if tabId === 'services'}
            <ServicesManager bind:this={pane.componentRefs[tabId]} {visible} />
          {:else if tabId === 'docker'}
            <DockerManager bind:this={pane.componentRefs[tabId]} {visible} onRequestTerminalExec={(session: ContainerSession) => {
              pane.terminalContainerSession = session;
              pane.terminalSessionId = crypto.randomUUID();
              pane.activeTab = 'terminal';
              pane.visitedTabs['terminal'] = true;
              activePaneId = pane.id;
            }} />
          {:else if tabId === 'cron'}
            <CronManager bind:this={pane.componentRefs[tabId]} {visible} />
          {:else if tabId === 'users'}
            <UserManager bind:this={pane.componentRefs[tabId]} {visible} />
          {:else if tabId === 'firewall'}
            <FirewallManager bind:this={pane.componentRefs[tabId]} {visible} />
          {:else if tabId === 'crowdsec'}
            <CrowdsecManager bind:this={pane.componentRefs[tabId]} {visible} profileId={currentProfileId} />
          {:else if tabId === 'pangolin'}
            <PangolinManager bind:this={pane.componentRefs[tabId]} {visible} />
          {:else if tabId === 'logs'}
            <LogViewer bind:this={pane.componentRefs[tabId]} {visible} />
          {:else if tabId === 'loganalysis'}
            <LogAnalysisManager bind:this={pane.componentRefs[tabId]} {visible} profileId={currentProfileId} />
          {:else if tabId === 'webserver'}
            <NginxProxyManager bind:this={pane.componentRefs[tabId]} {visible} profileId={currentProfileId} />
          {:else if tabId === 'processes'}
            <ProcessManager bind:this={pane.componentRefs[tabId]} {visible} />
          {:else if tabId === 'database'}
            <DatabaseManager bind:this={pane.componentRefs[tabId]} {visible} profileId={currentProfileId} />
          {:else if tabId === 'envvars'}
            <EnvManager bind:this={pane.componentRefs[tabId]} {visible} profileId={currentProfileId} />
          {:else if tabId === 'netdiag'}
            <NetDiagManager bind:this={pane.componentRefs[tabId]} {visible} />
          {:else if tabId === 'timers'}
            <TimerManager bind:this={pane.componentRefs[tabId]} {visible} />
          {:else if tabId === 'terminal'}
            {#key pane.terminalSessionId}
              <TerminalView
                bind:this={pane.componentRefs[tabId]}
                {visible}
                profileId={currentProfileId}
                containerSession={pane.terminalContainerSession}
                sessionId={pane.terminalSessionId}
                onExitContainer={() => { pane.terminalContainerSession = null; }}
              />
            {/key}
          {/if}
        {/snippet}

        {#each panes as pane (pane.id)}
          {@const isActive = pane.id === activePaneId}
          {@const showDropOverlay = customDragState?.active && !(customDragState.type === 'pane' && customDragState.id === pane.id)}
          <div
            class="pane-container"
            class:pane-focused={isActive}
            style={panes.length > 1 ? `grid-area: ${pane.r1} / ${pane.c1} / ${pane.r2} / ${pane.c2};` : ''}
            data-pane-id={pane.id}
            onclick={() => focusPane(pane.id)}
            role="region"
            aria-label={navLabels[pane.activeTab] ?? pane.activeTab}
          >
            <!-- Pane Header (only visible when multiple panes) -->
            {#if panes.length > 1}
              <div 
                class="pane-header"
                role="none"
                onpointerdown={(e: PointerEvent) => startCustomDrag(e, 'pane', pane.id)}
              >
                <div 
                  class="pane-tab-selector" 
                  role="button"
                  tabindex="0"
                  onclick={(e: MouseEvent) => { e.stopPropagation(); togglePaneSelector(pane.id); }}
                  onkeydown={(e: KeyboardEvent) => { if (e.key === 'Enter' || e.key === ' ') { e.stopPropagation(); togglePaneSelector(pane.id); } }}
                >
                  <GripVertical size={12} class="drag-handle" />
                  <span class="pane-tab-name">{navLabels[pane.activeTab] ?? pane.activeTab}</span>
                  <ChevronDown size={12} />
                </div>

                <div class="pane-actions">
                  <button
                    class="pane-action-btn"
                    onclick={(e: MouseEvent) => { e.stopPropagation(); pane.componentRefs[pane.activeTab]?.refresh?.(); }}
                    title={$LL.shell.refresh()}
                  >
                    <RefreshCw size={13} />
                  </button>
                  {#if panes.length < 4}
                    <button
                      class="pane-action-btn"
                      onclick={(e: MouseEvent) => { e.stopPropagation(); splitPane(pane.id, 'h'); }}
                      title={$LL.shell.paneSplitVertical()}
                    >
                      <SplitSquareVertical size={13} />
                    </button>
                    <button
                      class="pane-action-btn"
                      onclick={(e: MouseEvent) => { e.stopPropagation(); splitPane(pane.id, 'v'); }}
                      title={$LL.shell.paneSplitHorizontal()}
                    >
                      <SplitSquareHorizontal size={13} />
                    </button>
                  {/if}
                  <button
                    class="pane-action-btn pane-close"
                    onclick={(e: MouseEvent) => { e.stopPropagation(); closePane(pane.id); }}
                    title={$LL.shell.paneClose()}
                  >
                    <X size={13} />
                  </button>
                </div>

                <!-- Pane Tab Dropdown -->
                {#if paneSelectorOpen === pane.id}
                  <div class="pane-dropdown" role="none" onclick={(e: MouseEvent) => e.stopPropagation()}>
                    {#each TAB_IDS as tabId}
                      <button
                        class="pane-dropdown-item"
                        class:active={pane.activeTab === tabId}
                        onclick={() => selectPaneTab(pane.id, tabId)}
                      >
                        {navLabels[tabId] ?? tabId}
                      </button>
                    {/each}
                  </div>
                {/if}
              </div>
            {/if}

            <!-- Drop Zone Overlay -->
            {#if showDropOverlay}
              <div class="drop-overlay">
                <div class="drop-zone drop-zone-top" class:active={dragOverPaneId === pane.id && dragOverZone === 'top'}>
                  <Rows2 size={16} />
                </div>
                <div class="drop-zone drop-zone-bottom" class:active={dragOverPaneId === pane.id && dragOverZone === 'bottom'}>
                  <Rows2 size={16} />
                </div>
                <div class="drop-zone drop-zone-left" class:active={dragOverPaneId === pane.id && dragOverZone === 'left'}>
                  <Columns2 size={16} />
                </div>
                <div class="drop-zone drop-zone-right" class:active={dragOverPaneId === pane.id && dragOverZone === 'right'}>
                  <Columns2 size={16} />
                </div>
                <div class="drop-zone drop-zone-center" class:active={dragOverPaneId === pane.id && dragOverZone === 'center'}>
                  <span>{customDragState?.type === 'pane' ? $LL.shell.dropSwap() : $LL.shell.dropChangeTab()}</span>
                </div>
              </div>
            {/if}

            <!-- Pane Content -->
            <!--
              Keep-alive: every visited tab stays mounted; only the active one is shown.
              Switching tabs hides (display:none) rather than destroying the component, so
              its state — terminal cwd/SSH session, selections, scroll — survives.
            -->
            <div class="pane-content" style={customDragState?.active || resizeState ? 'pointer-events: none;' : ''}>
              {#each Object.keys(pane.visitedTabs) as tabId (tabId)}
                <div class="pane-tab-host" style:display={pane.activeTab === tabId ? 'contents' : 'none'}>
                  {@render paneTabContent(pane, tabId)}
                </div>
              {/each}
            </div>
          </div>
        {/each}
      </div>

      <!-- Custom Drag Ghost -->
      {#if customDragState?.active}
        <div 
          class="custom-drag-ghost"
          style="left: {customDragState.currentX + 15}px; top: {customDragState.currentY + 15}px;"
        >
          {customDragState.type === 'pane' ? $LL.shell.dragMovePane() : $LL.shell.dragOpenTab()}
        </div>
      {/if}
    </div>
  {:else}
    <!-- LOGIN SCREEN / PROFILE MANAGEMENT PANEL -->
    <div class="login-screen">
      <div class="login-glow"></div>
      
      <div class="login-container glass fade-in">
        <header class="login-header">
          <div class="logo-box">J</div>
          <h1>{$LL.shell.appTitle()}</h1>
          <p class="login-subtitle">{$LL.shell.appSubtitle()}</p>
        </header>

        {#if connectError}
          <div class="login-error">
            <AlertCircle size={18} />
            <span>{connectError}</span>
          </div>
        {/if}

        {#if showCreateProfile}
          <!-- Formularz tworzenia/edycji profilu -->
          <div class="profile-form">
            <h2>{currentProfileId ? $LL.profile.editTitle() : $LL.profile.addTitle()}</h2>
            
            <div class="form-group">
              <label for="prof-label">{$LL.profile.labelField()}</label>
              <input id="prof-label" type="text" placeholder={$LL.profile.labelPlaceholder()} bind:value={profileLabel} />
            </div>

            <div class="form-row">
              <div class="form-group flex-3">
                <label for="prof-host">{$LL.profile.hostField()}</label>
                <input id="prof-host" type="text" placeholder={$LL.profile.hostPlaceholder()} bind:value={profileHost} />
              </div>
              <div class="form-group flex-1">
                <label for="prof-port">{$LL.profile.portField()}</label>
                <input id="prof-port" type="number" bind:value={profilePort} />
              </div>
            </div>

            <div class="form-group">
              <label for="prof-user">{$LL.profile.userField()}</label>
              <input id="prof-user" type="text" placeholder={$LL.profile.userPlaceholder()} bind:value={profileUsername} />
            </div>

            <div class="form-group">
              <label for="prof-authtype">{$LL.profile.authMethod()}</label>
              <select id="prof-authtype" bind:value={profileAuthType}>
                <option value="password">{$LL.profile.authPassword()}</option>
                <option value="key">{$LL.profile.authKey()}</option>
              </select>
            </div>

            {#if profileAuthType === 'password'}
              <div class="form-group">
                <label for="prof-pass">{$LL.profile.passwordField()}</label>
                <input id="prof-pass" type="password" placeholder="••••••••" bind:value={profilePassword} />
              </div>
            {:else}
              <div class="form-group">
                <label for="prof-keypath">{$LL.profile.keyPathField()}</label>
                <input id="prof-keypath" type="text" placeholder={$LL.profile.keyPathPlaceholder()} bind:value={profileKeyPath} />
              </div>
              <div class="form-group">
                <label for="prof-keypass">{$LL.profile.keyPassphraseField()}</label>
                <input id="prof-keypass" type="password" placeholder="••••••••" bind:value={profileKeyPassphrase} />
              </div>
            {/if}

            <div class="profile-form-actions">
              <button class="primary" onclick={handleSaveProfile}>{$LL.profile.saveProfile()}</button>
              <button class="secondary" onclick={() => { showCreateProfile = false; currentProfileId = ''; }}>{$LL.common.cancel()}</button>
            </div>
          </div>
        {:else}
          <!-- Connection profiles list -->
          <div class="profiles-section">
            <div class="section-header">
              <h2>{$LL.profile.savedProfiles()}</h2>
              <button class="secondary btn-sm" onclick={() => showCreateProfile = true}>
                <Plus size={14} /> {$LL.profile.newProfile()}
              </button>
            </div>

            <div class="profiles-list">
              {#each profiles as profile}
                <div 
                  class="profile-card glass {isConnecting && currentProfileId === profile.id ? 'connecting' : ''}" 
                  role="button"
                  tabindex="0"
                  onclick={() => !isConnecting && handleConnect(profile.id)}
                  onkeydown={(e) => e.key === 'Enter' && !isConnecting && handleConnect(profile.id)}
                >
                  <div class="profile-card-left">
                    <div class="server-icon-box">
                      <Server size={18} />
                    </div>
                    <div class="profile-card-info">
                      <span class="profile-label">
                        {profile.label}
                        {#if profile.id === defaultProfileId}
                          <span class="default-badge">{$LL.profile.defaultProfile()}</span>
                        {/if}
                      </span>
                      <span class="profile-details">{profile.username}@{profile.host}:{profile.port}</span>
                    </div>
                  </div>
                  <div class="profile-card-right">
                    {#if isConnecting && currentProfileId === profile.id}
                      <Loader2 class="spin accent-amber-text" size={18} />
                    {:else}
                      <button class="icon-btn-card" onclick={(e) => handleSetDefault(profile.id, e)} title={profile.id === defaultProfileId ? $LL.profile.defaultProfile() : $LL.profile.setDefault()}>
                        <Star size={14} class={profile.id === defaultProfileId ? 'star-filled' : 'star-outline'} />
                      </button>
                      <button class="icon-btn-card" onclick={(e) => editProfile(profile, e)} title={$LL.profile.editAction()}>
                        <Settings size={14} />
                      </button>
                      <button class="icon-btn-card hover-red" onclick={(e) => handleDeleteProfile(profile.id, e)} title={$LL.profile.deleteAction()}>
                        <Trash2 size={14} />
                      </button>
                      <ChevronRight class="chevron-icon" size={18} />
                    {/if}
                  </div>
                </div>
              {/each}

              {#if profiles.length === 0}
                <div class="no-profiles glass">
                  <Server size={36} class="muted-icon" />
                  <p>{$LL.profile.noProfiles()}</p>
                  <button class="primary btn-sm" onclick={() => showCreateProfile = true}>
                    <Plus size={14} /> {$LL.profile.createFirst()}
                  </button>
                </div>
              {/if}
            </div>
          </div>
        {/if}
      </div>
    </div>
  {/if}

  <!-- Keyboard Shortcuts Modal -->
  {#if showShortcutsModal}
    <div class="modal-overlay" onclick={() => showShortcutsModal = false} role="presentation">
      <div class="modal-content glass shortcuts-modal" onclick={(e) => e.stopPropagation()} role="dialog">
        <div class="modal-header">
          <div class="modal-header-left">
            <Keyboard size={20} class="accent-amber-text" />
            <h3>Keyboard Shortcuts</h3>
          </div>
          <button class="icon-btn-compact" onclick={() => showShortcutsModal = false}>
            <X size={16} />
          </button>
        </div>

        <div class="shortcuts-grid">
          <div class="shortcuts-group">
            <h4>Workspace & Panes</h4>
            <div class="shortcut-row">
              <span class="shortcut-desc">Split active pane horizontally</span>
              <div class="shortcut-keys"><kbd>Ctrl</kbd> + <kbd>N</kbd></div>
            </div>
            <div class="shortcut-row">
              <span class="shortcut-desc">Close active pane</span>
              <div class="shortcut-keys"><kbd>Ctrl</kbd> + <kbd>W</kbd></div>
            </div>
            <div class="shortcut-row">
              <span class="shortcut-desc">Focus pane 1..4</span>
              <div class="shortcut-keys"><kbd>Ctrl</kbd> + <kbd>1..4</kbd></div>
            </div>
          </div>

          <div class="shortcuts-group">
            <h4>Tab Navigation</h4>
            <div class="shortcut-row">
              <span class="shortcut-desc">Next tab in pane</span>
              <div class="shortcut-keys"><kbd>Ctrl</kbd> + <kbd>Tab</kbd></div>
            </div>
            <div class="shortcut-row">
              <span class="shortcut-desc">Previous tab in pane</span>
              <div class="shortcut-keys"><kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>Tab</kbd></div>
            </div>
            <div class="shortcut-row">
              <span class="shortcut-desc">Open Terminal tab</span>
              <div class="shortcut-keys"><kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>T</kbd></div>
            </div>
          </div>

          <div class="shortcuts-group">
            <h4>App Control</h4>
            <div class="shortcut-row">
              <span class="shortcut-desc">Toggle sidebar collapse</span>
              <div class="shortcut-keys"><kbd>Ctrl</kbd> + <kbd>Alt</kbd> + <kbd>B</kbd></div>
            </div>
            <div class="shortcut-row">
              <span class="shortcut-desc">Show this help dialog</span>
              <div class="shortcut-keys"><kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>H</kbd></div>
            </div>
            <div class="shortcut-row">
              <span class="shortcut-desc">Close modal / Cancel</span>
              <div class="shortcut-keys"><kbd>Esc</kbd></div>
            </div>
          </div>
        </div>
      </div>
    </div>
  {/if}
</main>


<style>
  .app-container {
    position: relative;
  }

  .global-back-btn {
    position: fixed;
    top: 10px;
    right: 12px;
    z-index: 200;
    transition:
      opacity 0.15s ease,
      transform 0.12s cubic-bezier(0.2, 0, 0, 1);
  }

  .global-back-btn:not(:disabled):active {
    transform: scale(0.96);
  }

  .global-back-btn:disabled {
    opacity: 0.35;
    cursor: not-allowed;
  }

  .global-back-btn .back-tooltip {
    position: absolute;
    top: calc(100% + 8px);
    right: 0;
    width: max-content;
    max-width: 280px;
    padding: 8px 10px;
    border-radius: var(--radius-sm);
    background: rgba(12, 13, 18, 0.96);
    border: 1px solid var(--border-color);
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.45);
    color: var(--text-secondary);
    font-size: 0.75rem;
    line-height: 1.35;
    text-wrap: pretty;
    pointer-events: none;
    opacity: 0;
    transform: translateY(-4px);
    transition:
      opacity 0.15s ease,
      transform 0.15s cubic-bezier(0.2, 0, 0, 1);
  }

  .global-back-btn:not(:disabled):hover .back-tooltip,
  .global-back-btn:not(:disabled):focus-visible .back-tooltip {
    opacity: 1;
    transform: translateY(0);
  }

  .connect-error-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 16px;
    background: var(--accent-red-glow);
    border-bottom: 1px solid rgba(239, 68, 68, 0.3);
    color: #ff8585;
    font-size: 0.82rem;
    flex-shrink: 0;
  }

  /* ────────────── Workspace Control Bar ────────────── */

  .workspace-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 6px 12px;
    border-bottom: 1px solid var(--border-color);
    background: rgba(0, 0, 0, 0.2);
    flex-shrink: 0;
    min-height: 36px;
  }

  .workspace-bar-left {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .workspace-label {
    font-family: var(--font-mono);
    font-size: 0.7rem;
    font-weight: 600;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .workspace-pane-count {
    font-family: var(--font-mono);
    font-size: 0.68rem;
    color: var(--text-muted);
    background: rgba(255, 255, 255, 0.03);
    padding: 2px 8px;
    border-radius: 2px;
    border: 1px solid var(--border-color);
    font-variant-numeric: tabular-nums;
  }

  .workspace-bar-right {
    display: flex;
    align-items: center;
    gap: 2px;
  }

  .layout-btn {
    width: 28px;
    height: 28px;
    min-width: 28px;
    min-height: 28px;
    padding: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    background: transparent;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    color: var(--text-muted);
    cursor: pointer;
    transition: background 0.1s ease, color 0.1s ease, border-color 0.1s ease;
  }

  .layout-btn:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .layout-btn.active {
    background: var(--bg-active);
    color: var(--accent-amber);
    border-color: rgba(29, 78, 216, 0.2);
  }

  .layout-btn:disabled {
    opacity: 0.35;
    cursor: not-allowed;
  }

  /* ────────────── Panes Grid ────────────── */

  .panes-grid {
    flex: 1;
    overflow: hidden;
    position: relative;
  }

  .panes-grid.multi {
    display: grid;
    grid-template-columns: repeat(120, 1fr);
    grid-template-rows: repeat(120, 1fr);
    gap: 1px;
    background: var(--border-color);
  }

  .pane-container {
    display: flex;
    flex-direction: column;
    overflow: hidden;
    position: relative;
    background: var(--bg-secondary);
    border: 2px solid transparent;
    transition: border-color 0.12s ease;
  }

  .panes-grid:not(.multi) .pane-container {
    height: 100%;
  }

  .pane-container.pane-focused {
    border-color: var(--accent-muted);
  }

  .panes-grid.multi .pane-container.pane-focused {
    border-color: rgba(29, 78, 216, 0.45);
  }

  /* ────────────── Pane Header ────────────── */

  .pane-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 4px 8px;
    border-bottom: 1px solid var(--border-color);
    background: rgba(0, 0, 0, 0.25);
    flex-shrink: 0;
    min-height: 30px;
    position: relative;
    cursor: grab;
    -webkit-user-drag: element;
    user-select: none;
  }
  
  .pane-header:active {
    cursor: grabbing;
  }

  .pane-tab-selector {
    display: flex;
    align-items: center;
    gap: 6px;
    background: transparent;
    border: none;
    padding: 3px 8px;
    border-radius: var(--radius-sm);
    color: var(--text-secondary);
    font-size: 0.75rem;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.1s ease, color 0.1s ease;
    -webkit-user-drag: none;
    user-select: none;
  }
  
  .drag-handle {
    opacity: 0.4;
  }

  .pane-tab-selector:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .pane-tab-name {
    white-space: nowrap;
  }

  .pane-actions {
    display: flex;
    align-items: center;
    gap: 2px;
  }

  .pane-action-btn {
    width: 24px;
    height: 24px;
    min-width: 24px;
    padding: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    color: var(--text-muted);
    cursor: pointer;
    transition: background 0.1s ease, color 0.1s ease;
  }

  .pane-action-btn:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .pane-close:hover {
    background: var(--accent-red-glow);
    color: var(--accent-red);
  }

  /* ────────────── Pane Tab Dropdown ────────────── */

  .pane-dropdown {
    position: absolute;
    top: 100%;
    left: 4px;
    z-index: 100;
    background: var(--bg-secondary);
    border: 1px solid var(--border-color);
    border-radius: var(--radius-sm);
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
    max-height: 320px;
    overflow-y: auto;
    min-width: 180px;
  }

  .pane-dropdown-item {
    width: 100%;
    background: transparent;
    border: none;
    padding: 7px 12px;
    text-align: left;
    color: var(--text-secondary);
    font-size: 0.78rem;
    cursor: pointer;
    transition: background 0.08s ease, color 0.08s ease;
    border-bottom: 1px solid rgba(255, 255, 255, 0.02);
    border-radius: 0;
  }

  .pane-dropdown-item:last-child {
    border-bottom: none;
  }

  .pane-dropdown-item:hover {
    background: var(--bg-hover);
    color: white;
  }

  .pane-dropdown-item.active {
    color: var(--accent-amber);
    background: var(--bg-active);
  }

  /* ────────────── Pane Content ────────────── */

  .pane-content {
    flex: 1;
    overflow: hidden;
  }

  /* ────────────── Drop Zone Overlay ────────────── */

  .drop-overlay {
    position: absolute;
    inset: 0;
    z-index: 50;
    pointer-events: none;
  }

  .drop-zone {
    position: absolute;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: var(--radius-sm);
    background: rgba(29, 78, 216, 0.03);
    border: 2px dashed rgba(29, 78, 216, 0.12);
    color: rgba(29, 78, 216, 0.3);
    transition: background 0.1s ease, border-color 0.1s ease, color 0.1s ease;
    font-size: 0.72rem;
    font-weight: 600;
    pointer-events: auto;
  }

  .drop-zone.active {
    background: rgba(29, 78, 216, 0.1);
    border-color: rgba(29, 78, 216, 0.5);
    color: var(--accent-amber);
    box-shadow: inset 0 0 20px rgba(29, 78, 216, 0.08);
  }

  .drop-zone-top {
    top: 4px;
    left: 25%;
    right: 25%;
    height: 22%;
  }

  .drop-zone-bottom {
    bottom: 4px;
    left: 25%;
    right: 25%;
    height: 22%;
  }

  .drop-zone-left {
    top: 25%;
    bottom: 25%;
    left: 4px;
    width: 22%;
  }

  .drop-zone-right {
    top: 25%;
    bottom: 25%;
    right: 4px;
    width: 22%;
  }

  .drop-zone-center {
    top: 30%;
    bottom: 30%;
    left: 30%;
    right: 30%;
  }

  /* ────────────── Login Screen ────────────── */

  .login-screen {
    position: relative;
    width: 100vw;
    height: 100vh;
    display: flex;
    align-items: center;
    justify-content: center;
    background-color: var(--bg-primary);
    overflow: hidden;
  }

  .login-glow {
    position: absolute;
    width: 600px;
    height: 600px;
    background: radial-gradient(circle, rgba(29, 78, 216, 0.08) 0%, rgba(30, 64, 175, 0.02) 50%, rgba(0,0,0,0) 100%);
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    z-index: 0;
    pointer-events: none;
  }

  .login-container {
    width: 460px;
    padding: 40px;
    border-radius: var(--radius-md);
    background: var(--bg-secondary);
    border: 1px solid var(--border-color);
    z-index: 1;
    max-height: 90vh;
    overflow-y: auto;
  }

  .login-header {
    text-align: center;
    margin-bottom: 30px;
    display: flex;
    flex-direction: column;
    align-items: center;
  }

  .logo-box {
    width: 48px;
    height: 48px;
    border-radius: var(--radius-sm);
    background: linear-gradient(135deg, var(--accent-amber), var(--accent-rust));
    color: #0c0d12;
    font-weight: 800;
    font-size: 1.5rem;
    font-family: var(--font-display);
    display: flex;
    align-items: center;
    justify-content: center;
    box-shadow: 0 0 20px rgba(29, 78, 216, 0.3);
    margin-bottom: 16px;
  }

  .login-header h1 {
    font-size: 1.6rem;
    color: white;
    font-weight: 700;
  }

  .login-subtitle {
    font-size: 0.85rem;
    color: var(--text-secondary);
    margin-top: 6px;
    line-height: 1.4;
  }

  .login-error {
    background: var(--accent-red-glow);
    border: 1px solid rgba(239, 68, 68, 0.3);
    border-radius: var(--radius-sm);
    padding: 12px;
    color: #ff8585;
    font-size: 0.85rem;
    margin-bottom: 24px;
    display: flex;
    align-items: flex-start;
    gap: 10px;
  }

  /* Profiles Section */
  .profiles-section {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .section-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .profiles-section h2, .profile-form h2 {
    font-size: 1.1rem;
    color: white;
  }

  .profiles-list {
    display: flex;
    flex-direction: column;
    gap: 10px;
    max-height: 400px;
    overflow-y: auto;
    padding-right: 4px;
    padding-top: 4px;
    padding-bottom: 4px;
    margin-top: -4px;
    margin-bottom: -4px;
  }

  .profile-card {
    background: rgba(255, 255, 255, 0.01);
    border: 1px solid var(--border-color);
    padding: 16px;
    border-radius: var(--radius-sm);
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: space-between;
    cursor: pointer;
    text-align: left;
    transition: background 0.1s ease, border-color 0.1s ease;
  }

  .profile-card:hover {
    background: var(--bg-hover);
    border-color: rgba(29, 78, 216, 0.2);
  }

  .profile-card.connecting {
    background: var(--bg-active);
    border-color: var(--accent-amber);
    cursor: not-allowed;
  }

  .profile-card-left {
    display: flex;
    align-items: center;
    gap: 14px;
  }

  .server-icon-box {
    background: rgba(255, 255, 255, 0.03);
    padding: 10px;
    border-radius: var(--radius-sm);
    color: var(--text-secondary);
    border: 1px solid var(--border-color);
  }

  .profile-card:hover .server-icon-box {
    color: var(--accent-amber);
    background: rgba(29, 78, 216, 0.08);
    border-color: rgba(29, 78, 216, 0.2);
  }

  .profile-card-info {
    display: flex;
    flex-direction: column;
  }

  .profile-label {
    font-size: 0.95rem;
    color: white;
    font-weight: 600;
  }

  .profile-details {
    font-size: 0.75rem;
    color: var(--text-muted);
    margin-top: 4px;
  }

  .profile-card-right {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .icon-btn-card {
    background: transparent;
    border: none;
    padding: 6px;
    border-radius: 4px;
    color: var(--text-muted);
    cursor: pointer;
    transition: background 0.1s ease, color 0.1s ease;
  }

  .icon-btn-card:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .icon-btn-card.hover-red:hover {
    color: var(--accent-red);
    background: var(--accent-red-glow);
  }

  .chevron-icon {
    color: var(--text-muted);
    transition: color 0.1s ease, transform 0.1s ease;
  }

  .profile-card:hover .chevron-icon {
    color: white;
    transform: translateX(2px);
  }

  .star-filled {
    color: var(--accent-amber, #f59e0b);
  }

  .star-outline {
    color: var(--text-muted);
    opacity: 0.5;
  }

  .star-outline:hover {
    opacity: 1;
  }

  .default-badge {
    font-size: 0.65rem;
    font-weight: 600;
    background: var(--accent-amber, #f59e0b);
    color: #000;
    padding: 1px 6px;
    border-radius: 4px;
    margin-left: 6px;
    vertical-align: middle;
    text-transform: uppercase;
    letter-spacing: 0.03em;
  }

  .no-profiles {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 40px;
    text-align: center;
    gap: 16px;
    border-radius: var(--radius-md);
  }

  .muted-icon {
    color: var(--text-muted);
  }

  .no-profiles p {
    color: var(--text-secondary);
    font-size: 0.9rem;
  }

  /* Profile Form */
  .profile-form {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .profile-form h2 {
    border-bottom: 1px solid var(--border-color);
    padding-bottom: 10px;
  }

  .form-row {
    display: flex;
    gap: 16px;
  }

  .flex-1 { flex: 1; }
  .flex-3 { flex: 3; }

  .form-group {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .form-group label {
    font-size: 0.8rem;
    color: var(--text-secondary);
    font-weight: 500;
  }

  .form-group input,
  .form-group select {
    width: 100%;
  }

  .profile-form-actions {
    display: flex;
    gap: 10px;
    justify-content: flex-end;
    margin-top: 8px;
  }

  .btn-sm {
    padding: 6px 12px;
    font-size: 0.8rem;
  }

  .custom-drag-ghost {
    position: fixed;
    background: var(--surface-2);
    border: 1px solid var(--border-color);
    box-shadow: 0 10px 25px rgba(0,0,0,0.5);
    padding: 8px 12px;
    border-radius: 6px;
    pointer-events: none;
    z-index: 9999;
    font-size: 13px;
    font-weight: 500;
    color: var(--text-primary);
  }

  .resizer {
    background: transparent;
    transition: background 0.15s ease-in-out;
  }
  .resizer:hover, .resizer:active {
    background: rgba(29, 78, 216, 0.35);
  }
  .resizer-v {
    border-left: 1px solid transparent;
    border-right: 1px solid transparent;
  }
  .resizer-h {
    border-top: 1px solid transparent;
    border-bottom: 1px solid transparent;
  }

  /* Shortcuts Modal Styles */
  .modal-overlay {
    position: fixed;
    top: 0;
    left: 0;
    width: 100vw;
    height: 100vh;
    background: rgba(0, 0, 0, 0.6);
    backdrop-filter: blur(4px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 9999;
  }

  .shortcuts-modal {
    width: 480px;
    max-width: 90%;
    padding: 24px;
    border-radius: var(--radius-md);
    display: flex;
    flex-direction: column;
    gap: 20px;
    box-shadow: 0 20px 40px rgba(0, 0, 0, 0.4);
    border: 1px solid var(--border-color);
  }

  .modal-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    border-bottom: 1px solid var(--border-color);
    padding-bottom: 12px;
  }

  .modal-header-left {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .modal-header-left h3 {
    margin: 0;
    font-size: 1.15rem;
    font-weight: 600;
  }

  .icon-btn-compact {
    background: transparent;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 4px;
    border-radius: var(--radius-sm);
    transition: color 0.15s, background-color 0.15s;
  }

  .icon-btn-compact:hover {
    color: var(--text-primary);
    background-color: var(--bg-hover);
  }

  .shortcuts-grid {
    display: flex;
    flex-direction: column;
    gap: 20px;
  }

  .shortcuts-group {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .shortcuts-group h4 {
    margin: 0 0 4px 0;
    font-size: 0.85rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-secondary);
    font-weight: 700;
  }

  .shortcut-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: 0.9rem;
    padding: 6px 0;
    border-bottom: 1px solid rgba(255, 255, 255, 0.03);
  }

  .shortcut-desc {
    color: var(--text-secondary);
  }

  .shortcut-keys {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  kbd {
    background-color: var(--surface-3, #1e293b);
    border: 1px solid var(--border-color);
    border-radius: 4px;
    box-shadow: 0 2px 0 rgba(0, 0, 0, 0.2);
    color: var(--text-primary);
    display: inline-block;
    font-family: var(--font-mono, monospace);
    font-size: 0.75rem;
    font-weight: 600;
    line-height: 1;
    padding: 4px 6px;
    white-space: nowrap;
  }
</style>
