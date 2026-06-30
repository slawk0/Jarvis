<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import {
    Folder,
    File,
    ArrowLeft,
    RefreshCw,
    Plus,
    Trash2,
    Edit,
    CornerDownLeft,
    Download,
    Upload,
    KeyRound,
    Search,
    X,
    Loader2,
    Star,
    ChevronDown,
    FolderInput,
  } from 'lucide-svelte';
  import SftpBulkActionsBar from './SftpBulkActionsBar.svelte';
  import SudoModal from '../SudoModal.svelte';
  import SortableTh from '../ui/SortableTh.svelte';
  import PathAutocomplete from '../ui/PathAutocomplete.svelte';
  import type { FileInfo } from '$lib/sftp/types';
  import { applySort, nextSort, type SortState } from '$lib/sort/sortUtils';
  import {
    formatBytes,
    formatModified,
    formatPermissions,
    getRemotePath,
    joinRemotePath,
    isSubPath,
    fileRemotePath,
    parentPath,
    isHiddenEntry,
  } from '$lib/sftp/pathUtils';
  import {
    initTransferStore,
    destroyTransferStore,
    setOnBatchComplete,
    startUploadBatch,
    startDownloadBatch,
    startMoveBatch,
    startDeleteBatch,
  } from '$lib/sftp/transferStore.svelte';
  import { registerBackHandler } from '$lib/backNavigation.svelte';
  import { get } from 'svelte/store';
  import { formatInvokeError } from '$lib/backendErrors';

  interface Props {
    onEdit: (filePath: string, file: FileInfo) => void;
    onChmod: (file: FileInfo) => void;
    onRename: (file: FileInfo) => void;
    onNewFile: () => void;
    onNewDir: () => void;
    onError: (msg: string) => void;
    onPathChange?: (path: string) => void;
    profileId?: string;
  }

  let { onEdit, onChmod, onRename, onNewFile, onNewDir, onError, onPathChange, profileId = '' }: Props = $props();

  export function getCurrentPath() {
    return lastLoadedPath;
  }

  export async function refresh() {
    await loadDirectory();
  }

  let currentPath = $state('');
  let lastLoadedPath = $state('');
  let files = $state<FileInfo[]>([]);
  let isLoading = $state(false);

  let selectedPaths = $state<Set<string>>(new Set());
  let lastClickedIndex = $state(-1);
  let moveMode = $state(false);
  let dragOverFolder = $state<string | null>(null);
  let externalDragOver = $state(false);
  let unlistenDrag: (() => void) | null = null;
  let unregisterBack: (() => void) | null = null;

  // Conflict modal
  let showConflictModal = $state(false);
  let pendingUploadPaths = $state<string[]>([]);
  let conflictFiles = $state<string[]>([]);
  let conflictAction = $state<'overwrite' | 'skip' | 'rename'>('overwrite');
  let searchQuery = $state('');
  let hideHidden = $state(true);
  let recursiveSearch = $state(false);
  let searchRootPath = $state('');
  let searchResults = $state<FileInfo[]>([]);
  let isSearchLoading = $state(false);
  
  // Sudo modal
  let showSudoModal = $state(false);
  let sudoModalTitle = $state<string | undefined>(undefined);
  let sudoModalDesc = $state<string | undefined>(undefined);
  let pendingSudoAction: (() => Promise<void>) | null = null;
  let searchGeneration = 0;
  let searchDebounce: ReturnType<typeof setTimeout> | undefined;
  type FileSortCol = 'name' | 'size' | 'permissions' | 'modified';
  let fileSort = $state<SortState<FileSortCol>>({ column: 'name', direction: 'asc' });
  let folderSizes = $state<Map<string, number>>(new Map());
  let folderSizesLoading = $state<Set<string>>(new Set());
  let folderSizeGeneration = 0;

  // Bookmarks State & Logic
  let showBookmarksDropdown = $state(false);
  let bookmarks = $state<{ name: string; path: string }[]>([]);

  $effect(() => {
    const key = `jarvis_bookmarks_${profileId || 'global'}`;
    const stored = localStorage.getItem(key);
    if (stored) {
      try {
        bookmarks = JSON.parse(stored);
      } catch (e) {
        bookmarks = [];
      }
    } else {
      bookmarks = [
        { name: 'nginx', path: '/etc/nginx' },
        { name: 'log', path: '/var/log' },
        { name: 'home', path: '/home' }
      ];
      localStorage.setItem(key, JSON.stringify(bookmarks));
    }
  });

  function saveBookmarks() {
    const key = `jarvis_bookmarks_${profileId || 'global'}`;
    localStorage.setItem(key, JSON.stringify($state.snapshot(bookmarks)));
  }

  const isCurrentBookmarked = $derived(
    bookmarks.some((b) => b.path === currentPath)
  );

  function toggleBookmarkCurrent() {
    if (isCurrentBookmarked) {
      bookmarks = bookmarks.filter((b) => b.path !== currentPath);
    } else {
      const name = currentPath.split('/').pop() || currentPath;
      bookmarks = [...bookmarks, { name, path: currentPath }];
    }
    saveBookmarks();
  }

  function removeBookmark(path: string, e: MouseEvent) {
    e.stopPropagation();
    bookmarks = bookmarks.filter((b) => b.path !== path);
    saveBookmarks();
  }

  function navigateToBookmark(path: string) {
    currentPath = path;
    showBookmarksDropdown = false;
    void loadDirectory();
  }

  const isRecursiveActive = $derived(recursiveSearch && searchQuery.trim().length > 0);

  const effectiveSearchRoot = $derived(searchRootPath.trim() || lastLoadedPath);

  function entrySize(file: FileInfo): number {
    if (!file.is_dir) return file.size;
    const path = fileRemotePath(file, lastLoadedPath);
    return folderSizes.get(path) ?? file.size;
  }

  function formatEntrySize(file: FileInfo): string {
    if (!file.is_dir) return formatBytes(file.size);
    const path = fileRemotePath(file, lastLoadedPath);
    const computed = folderSizes.get(path);
    if (computed !== undefined) return formatBytes(computed);
    if (folderSizesLoading.has(path)) return '…';
    if (isRecursiveActive) return formatBytes(file.size);
    return '…';
  }

  const visibleFiles = $derived(
    hideHidden ? files.filter((f) => !isHiddenEntry(f.name)) : files,
  );

  const localFilteredFiles = $derived(
    !isRecursiveActive && searchQuery.trim()
      ? visibleFiles.filter((f) =>
          f.name.toLowerCase().includes(searchQuery.trim().toLowerCase()),
        )
      : visibleFiles,
  );

  const listSource = $derived(isRecursiveActive ? searchResults : localFilteredFiles);

  async function resolveFolderSizes(dirPath: string, dirs: FileInfo[]) {
    const gen = ++folderSizeGeneration;
    const paths = dirs.map((d) => getRemotePath(dirPath, d.name));
    if (paths.length === 0) return;

    folderSizes = new Map();
    folderSizesLoading = new Set(paths);

    const worker = async (path: string) => {
      if (gen !== folderSizeGeneration) return;
      try {
        const size = await invoke<number>('sftp_dir_size', { path });
        if (gen !== folderSizeGeneration) return;
        folderSizes = new Map(folderSizes).set(path, size);
      } catch {
        if (gen !== folderSizeGeneration) return;
        folderSizes = new Map(folderSizes).set(path, 0);
      } finally {
        if (gen !== folderSizeGeneration) return;
        const next = new Set(folderSizesLoading);
        next.delete(path);
        folderSizesLoading = next;
      }
    };

    const concurrency = 6;
    for (let i = 0; i < paths.length; i += concurrency) {
      if (gen !== folderSizeGeneration) return;
      await Promise.all(paths.slice(i, i + concurrency).map(worker));
    }
  }

  const displayFiles = $derived(
    applySort(
      listSource,
      fileSort,
      {
        name: (f) => f.name,
        size: (f) => entrySize(f),
        permissions: (f) => f.permissions ?? 0,
        modified: (f) => f.modified,
      },
      { dirsFirst: (f) => f.is_dir },
    ),
  );

  const visiblePaths = $derived(
    displayFiles.map((f) => fileRemotePath(f, lastLoadedPath)),
  );

  const allSelected = $derived(
    displayFiles.length > 0 &&
      displayFiles.every((f) => selectedPaths.has(fileRemotePath(f, lastLoadedPath))),
  );

  function clearSearch() {
    searchQuery = '';
    searchResults = [];
    isSearchLoading = false;
    if (searchDebounce) clearTimeout(searchDebounce);
  }

  function setSearchScope(recursive: boolean) {
    if (recursiveSearch === recursive) return;
    recursiveSearch = recursive;
    if (recursive) {
      if (!searchRootPath.trim()) searchRootPath = lastLoadedPath;
    } else {
      searchResults = [];
      isSearchLoading = false;
    }
    if (searchQuery.trim()) scheduleRecursiveSearch();
  }

  function scheduleRecursiveSearch() {
    if (!recursiveSearch) {
      searchResults = [];
      isSearchLoading = false;
      return;
    }

    if (searchDebounce) clearTimeout(searchDebounce);

    const q = searchQuery.trim();
    if (!q) {
      searchResults = [];
      isSearchLoading = false;
      return;
    }

    isSearchLoading = true;

    searchDebounce = setTimeout(() => {
      void runRecursiveSearch();
    }, 350);
  }

  async function runRecursiveSearch() {
    const q = searchQuery.trim();
    if (!q || !recursiveSearch) return;

    const gen = ++searchGeneration;
    isSearchLoading = true;
    onError('');

    try {
      const results = await invoke<FileInfo[]>('sftp_find', {
        root: effectiveSearchRoot,
        query: q,
        hideHidden,
      });
      if (gen !== searchGeneration) return;
      searchResults = results;
    } catch (err: unknown) {
      if (gen !== searchGeneration) return;
      searchResults = [];
      onError(`Search error: ${formatInvokeError(err)}`);
    } finally {
      if (gen === searchGeneration) isSearchLoading = false;
    }
  }

  function onSearchInput() {
    scheduleRecursiveSearch();
  }

  function onHideHiddenChange() {
    if (recursiveSearch && searchQuery.trim()) {
      isSearchLoading = true;
      scheduleRecursiveSearch();
    }
  }

  function resolveFileByPath(path: string): FileInfo | undefined {
    return (
      files.find((f) => fileRemotePath(f, lastLoadedPath) === path) ??
      searchResults.find((f) => fileRemotePath(f, lastLoadedPath) === path)
    );
  }

  function refreshAfterFsChange() {
    if (isRecursiveActive) {
      void runRecursiveSearch();
    } else {
      void loadDirectory();
    }
  }

  async function loadDirectory() {
    isLoading = true;
    onError('');
    try {
      const result = await invoke<FileInfo[]>('sftp_list', { path: currentPath });
      const loadedPath = currentPath;
      files = result;
      lastLoadedPath = loadedPath;
      selectedPaths = new Set();
      clearSearch();
      searchRootPath = loadedPath;
      onPathChange?.(lastLoadedPath);
      void resolveFolderSizes(loadedPath, result.filter((f) => f.is_dir));
    } catch (err: unknown) {
      const errStr = String(err).toLowerCase();
      if (errStr.includes('permission denied')) {
        try {
          const hasSudo = await invoke<boolean>('has_sudo_password');
          if (hasSudo) {
            await loadDirectorySudo(currentPath);
            return;
          }
        } catch {}
        
        sudoModalTitle = "Sudo password required to change permissions:";
        sudoModalDesc = undefined;
        pendingSudoAction = async () => { await loadDirectorySudo(currentPath); };
        showSudoModal = true;
        return;
      }
      onError(`Could not read directory: ${formatInvokeError(err)}`);
      currentPath = lastLoadedPath;
    } finally {
      isLoading = false;
    }
  }

  async function loadDirectorySudo(path: string) {
    isLoading = true;
    onError('');
    try {
      const escapedPath = "'" + path.replace(/'/g, "'\\''") + "'";
      const cmd = `ls -la --time-style=+%s ${escapedPath}`;
      const out = await invoke<string>('exec_custom_command', { cmd, useSudo: true });
      const lines = out.split('\n');
      const newFiles: FileInfo[] = [];
      for (const line of lines) {
        if (!line.trim() || line.startsWith('total ')) continue;
        const match = line.match(/^([d\-l][rwx\-st]+)\s+\d+\s+\S+\s+\S+\s+(\d+)\s+(\d+)\s+(.+)$/);
        if (match) {
          const name = match[4];
          if (name === '.' || name === '..') continue;
          let p = 0;
          const perms = match[1];
          if (perms[1] === 'r') p |= 0o400;
          if (perms[2] === 'w') p |= 0o200;
          if (perms[3] === 'x' || perms[3] === 's') p |= 0o100;
          if (perms[4] === 'r') p |= 0o040;
          if (perms[5] === 'w') p |= 0o020;
          if (perms[6] === 'x' || perms[6] === 's') p |= 0o010;
          if (perms[7] === 'r') p |= 0o004;
          if (perms[8] === 'w') p |= 0o002;
          if (perms[9] === 'x' || perms[9] === 't') p |= 0o001;
          
          newFiles.push({
            name,
            is_dir: perms.startsWith('d'),
            size: parseInt(match[2], 10),
            modified: parseInt(match[3], 10),
            permissions: p,
            path: undefined
          });
        }
      }
      
      newFiles.sort((a, b) => {
        if (a.is_dir !== b.is_dir) return a.is_dir ? -1 : 1;
        return a.name.localeCompare(b.name);
      });
      
      const loadedPath = path;
      files = newFiles;
      lastLoadedPath = loadedPath;
      currentPath = loadedPath;
      selectedPaths = new Set();
      clearSearch();
      searchRootPath = loadedPath;
      onPathChange?.(lastLoadedPath);
    } catch (err: unknown) {
      onError(`Could not read directory: ${formatInvokeError(err)}`);
      currentPath = lastLoadedPath;
    } finally {
      isLoading = false;
    }
  }

  async function navigateToFolder(path: string) {
    if (moveMode) {
      await executeBulkMove(path);
      return;
    }
    clearSearch();
    recursiveSearch = false;
    currentPath = path;
    await loadDirectory();
  }

  async function openFolderByName(name: string) {
    await navigateToFolder(getRemotePath(lastLoadedPath, name));
  }

  async function handleGoBack() {
    if (lastLoadedPath === '/' || lastLoadedPath === '') return;
    const parts = lastLoadedPath.split('/');
    parts.pop();
    currentPath = parts.join('/') || '/';
    await loadDirectory();
  }

  function toggleSelect(path: string, index: number, event: MouseEvent) {
    const next = new Set(selectedPaths);
    if (event.shiftKey && lastClickedIndex >= 0) {
      const start = Math.min(lastClickedIndex, index);
      const end = Math.max(lastClickedIndex, index);
      for (let i = start; i <= end; i++) {
        next.add(visiblePaths[i]);
      }
    } else if (event.ctrlKey || event.metaKey) {
      if (next.has(path)) next.delete(path);
      else next.add(path);
    } else {
      if (next.has(path) && next.size === 1) next.clear();
      else {
        next.clear();
        next.add(path);
      }
    }
    selectedPaths = next;
    lastClickedIndex = index;
  }

  function setFileSort(column: string) {
    fileSort = nextSort(fileSort, column as FileSortCol);
  }

  function toggleSelectAll() {
    if (allSelected) {
      const next = new Set(selectedPaths);
      for (const p of visiblePaths) next.delete(p);
      selectedPaths = next;
    } else {
      selectedPaths = new Set([...selectedPaths, ...visiblePaths]);
    }
  }

  function handleRowClick(file: FileInfo, remotePath: string, index: number, e: MouseEvent) {
    const target = e.target as HTMLElement;
    if (target.closest('.actions-cell') || target.closest('input[type=checkbox]')) return;

    if (moveMode && file.is_dir) {
      void openFolderByName(file.name);
      return;
    }

    toggleSelect(remotePath, index, e);
  }

  function handleRowDblClick(file: FileInfo, remotePath: string, e: MouseEvent) {
    const target = e.target as HTMLElement;
    if (target.closest('.actions-cell') || target.closest('input[type=checkbox]')) return;
    if (moveMode) return;

    if (file.is_dir) {
      void navigateToFolder(remotePath);
    } else {
      onEdit(remotePath, file);
    }
  }

  function clearSelection() {
    selectedPaths = new Set();
    moveMode = false;
  }

  async function handleBulkDownload() {
    const paths = Array.from(selectedPaths);
    if (paths.length === 0) return;
    try {
      await startDownloadBatch(paths);
      clearSelection();
    } catch (err: unknown) {
      onError(`Download error: ${formatInvokeError(err)}`);
    }
  }

  async function handleBulkDelete() {
    const paths = Array.from(selectedPaths);
    if (paths.length === 0) return;
    if (!confirm(`Delete ${String(paths.length)} item(s)?`)) return;

    const items = paths.map((p) => {
      const file = resolveFileByPath(p);
      return { path: p, is_dir: file?.is_dir ?? false };
    });

    try {
      await startDeleteBatch(items);
      clearSelection();
      setOnBatchComplete(() => refreshAfterFsChange());
    } catch (err: unknown) {
      onError(`Delete error: ${formatInvokeError(err)}`);
    }
  }

  function startMoveMode() {
    if (selectedPaths.size === 0) return;
    moveMode = true;
  }

  function startMoveSingle(file: FileInfo) {
    const path = fileRemotePath(file, lastLoadedPath);
    selectedPaths = new Set([path]);
    moveMode = true;
  }

  async function executeBulkMove(destDir: string) {
    const moves = Array.from(selectedPaths).map((src) => {
      const name = src.split('/').pop() || src;
      return { src, dest: joinRemotePath(destDir, name) };
    });

    for (const m of moves) {
      if (isSubPath(m.src, m.dest)) {
        onError("Cannot move folder into itself or a subfolder.");
        moveMode = false;
        return;
      }
    }

    try {
      await startMoveBatch(moves);
      moveMode = false;
      clearSelection();
      setOnBatchComplete(() => refreshAfterFsChange());
    } catch (err: unknown) {
      onError(`Move error: ${formatInvokeError(err)}`);
      moveMode = false;
    }
  }

  async function handleMoveHere() {
    await executeBulkMove(lastLoadedPath);
  }

  // Internal drag-and-drop (move on server)
  const DRAG_MIME = 'application/x-jarvis-sftp';

  function handleDragStart(e: DragEvent, file: FileInfo) {
    const path = fileRemotePath(file, lastLoadedPath);
    let paths: string[];
    if (selectedPaths.has(path) && selectedPaths.size > 1) {
      paths = Array.from(selectedPaths);
    } else {
      paths = [path];
      if (!selectedPaths.has(path)) {
        selectedPaths = new Set([path]);
      }
    }
    e.dataTransfer?.setData(DRAG_MIME, JSON.stringify({ paths }));
    e.dataTransfer!.effectAllowed = 'move';
  }

  function handleDragOverFolder(e: DragEvent, folderName: string) {
    e.preventDefault();
    e.dataTransfer!.dropEffect = 'move';
    dragOverFolder = folderName;
  }

  function handleDragLeaveFolder() {
    dragOverFolder = null;
  }

  async function handleDropOnFolder(e: DragEvent, folderName: string) {
    e.preventDefault();
    dragOverFolder = null;
    const raw = e.dataTransfer?.getData(DRAG_MIME);
    if (!raw) return;

    try {
      const { paths } = JSON.parse(raw) as { paths: string[] };
      const destDir = getRemotePath(lastLoadedPath, folderName);
      const moves = paths.map((src) => {
        const name = src.split('/').pop() || src;
        return { src, dest: joinRemotePath(destDir, name) };
      });

      for (const m of moves) {
        if (isSubPath(m.src, m.dest)) {
          onError("Cannot move folder into itself or a subfolder.");
          return;
        }
      }

      await startMoveBatch(moves);
      setOnBatchComplete(() => refreshAfterFsChange());
    } catch (err: unknown) {
      onError(`Move error: ${formatInvokeError(err)}`);
    }
  }

  // External upload via Tauri drag-drop
  async function queueExternalUpload(paths: string[]) {
    const existing = new Set(files.map((f) => f.name));
    const conflicts = paths
      .map((p) => p.split(/[/\\]/).pop() || p)
      .filter((name) => existing.has(name));

    if (conflicts.length > 0) {
      pendingUploadPaths = paths;
      conflictFiles = conflicts;
      showConflictModal = true;
      return;
    }

    await doUpload(paths);
  }

  async function doUpload(paths: string[], action?: 'overwrite' | 'skip' | 'rename') {
    try {
      if (action === 'skip') {
        const existing = new Set(files.map((f) => f.name));
        paths = paths.filter((p) => !existing.has(p.split(/[/\\]/).pop() || ''));
      }
      if (paths.length === 0) return;

      if (action === 'rename') {
        const existing = new Set(files.map((f) => f.name));
        const pairs = paths.map((p) => {
          let name = p.split(/[/\\]/).pop() || p;
          if (existing.has(name)) {
            const dot = name.lastIndexOf('.');
            if (dot > 0) {
              name = `${name.slice(0, dot)}.1${name.slice(dot)}`;
            } else {
              name = `${name}.1`;
            }
          }
          return `${p}::${joinRemotePath(lastLoadedPath, name)}`;
        });
        await startUploadBatch(lastLoadedPath, pairs);
      } else {
        await startUploadBatch(lastLoadedPath, paths);
      }
      setOnBatchComplete(() => refreshAfterFsChange());
    } catch (err: unknown) {
      onError(`Upload error: ${formatInvokeError(err)}`);
    }
  }

  async function pickAndUpload() {
    try {
      const paths = await invoke<string[]>('sftp_pick_files');
      if (paths.length > 0) await queueExternalUpload(paths);
    } catch (err: unknown) {
      onError(`File picker error: ${formatInvokeError(err)}`);
    }
  }

  function resolveConflict() {
    showConflictModal = false;
    doUpload(pendingUploadPaths, conflictAction);
    pendingUploadPaths = [];
    conflictFiles = [];
  }

  async function pickFolderAndUpload() {
    try {
      const folder = await invoke<string | null>('sftp_pick_folder');
      if (folder) await queueExternalUpload([folder]);
    } catch (err: unknown) {
      onError(`Folder picker error: ${formatInvokeError(err)}`);
    }
  }

  async function downloadSingle(file: FileInfo) {
    const path = fileRemotePath(file, lastLoadedPath);
    try {
      await startDownloadBatch([path]);
    } catch (err: unknown) {
      onError(`Download error: ${formatInvokeError(err)}`);
    }
  }

  async function deleteSingle(file: FileInfo) {
    const path = fileRemotePath(file, lastLoadedPath);
    if (!confirm(`Are you sure you want to delete ${file.is_dir ? 'directory' : 'file'} "${file.name}"?`)) return;
    try {
      await startDeleteBatch([{ path, is_dir: file.is_dir }]);
      setOnBatchComplete(() => refreshAfterFsChange());
    } catch (err: unknown) {
      onError(`Delete error: ${formatInvokeError(err)}`);
    }
  }

  // Context menu
  let showContextMenu = $state(false);
  let contextMenuX = $state(0);
  let contextMenuY = $state(0);
  let contextMenuFile = $state<FileInfo | null>(null);

  function handleContextMenu(event: MouseEvent, file: FileInfo) {
    event.preventDefault();
    contextMenuFile = file;
    contextMenuX = event.clientX;
    contextMenuY = event.clientY;
    showContextMenu = true;
  }

  function closeContextMenu() {
    showContextMenu = false;
    showBookmarksDropdown = false;
  }

  onMount(async () => {
    unregisterBack = registerBackHandler({
      id: 'sftp-browser',
      priority: 80,
      canGoBack: () =>
        showConflictModal ||
        showContextMenu ||
        moveMode ||
        (lastLoadedPath !== '/' && lastLoadedPath !== ''),
      goBack: () => {
        if (showConflictModal) {
          showConflictModal = false;
          pendingUploadPaths = [];
          return;
        }
        if (showContextMenu) {
          closeContextMenu();
          return;
        }
        if (moveMode) {
          moveMode = false;
          return;
        }
        void handleGoBack();
      },
      label: () => {
        if (showConflictModal) return "Cancel conflict resolution";
        if (showContextMenu) return "Close context menu";
        if (moveMode) return "Cancel file move";
        return "Back to parent folder";
      },
    });

    await initTransferStore();
    setOnBatchComplete(() => refreshAfterFsChange());

    try {
      const home = await invoke<string>('sftp_get_home_dir');
      currentPath = home;
      lastLoadedPath = home;
    } catch {
      currentPath = '/';
      lastLoadedPath = '/';
    }
    await loadDirectory();

    try {
      const win = getCurrentWindow();
      unlistenDrag = await win.onDragDropEvent((event) => {
        if (event.payload.type === 'over') {
          externalDragOver = true;
        } else if (event.payload.type === 'drop') {
          externalDragOver = false;
          if (event.payload.paths?.length) {
            queueExternalUpload(event.payload.paths);
          }
        } else {
          externalDragOver = false;
        }
      });
    } catch {
      // drag-drop may be unavailable outside Tauri
    }
  });

  onDestroy(() => {
    unregisterBack?.();
    unlistenDrag?.();
    destroyTransferStore();
  });
</script>

<svelte:window onclick={closeContextMenu} />

<SudoModal
  bind:open={showSudoModal}
  title={sudoModalTitle}
  description={sudoModalDesc}
  onSuccess={() => {
    if (pendingSudoAction) pendingSudoAction();
  }}
/>

<div class="browser">
  <div
    class="nav-bar glass"
    class:drop-active={externalDragOver}
  >
    <button class="secondary btn-icon-compact" onclick={handleGoBack} disabled={currentPath === '/'}>
      <ArrowLeft size={15} />
    </button>
    <PathAutocomplete
      bind:value={currentPath}
      class="path-input mono-val"
      onlyDirs={true}
      onEnter={loadDirectory}
    />
    <button class="secondary btn-icon-compact" onclick={loadDirectory} disabled={isLoading}>
      <RefreshCw size={15} class={isLoading ? 'spin' : ''} />
    </button>

    <!-- Bookmarks Dropdown -->
    <div class="bookmarks-container">
      <button 
        class="secondary btn-icon-compact bookmark-btn" 
        class:bookmarked={isCurrentBookmarked}
        onclick={(e) => { e.stopPropagation(); showBookmarksDropdown = !showBookmarksDropdown; }}
        title="Bookmarks"
      >
        <Star size={15} class={isCurrentBookmarked ? 'star-active' : ''} />
      </button>

      {#if showBookmarksDropdown}
        <div class="bookmarks-dropdown glass" onclick={(e) => e.stopPropagation()}>
          <div class="bookmarks-header">
            <span>Bookmarks</span>
            <button class="btn-text-action" onclick={toggleBookmarkCurrent}>
              {isCurrentBookmarked ? "Remove bookmark" : "Bookmark current path"}
            </button>
          </div>
          <div class="bookmarks-list">
            {#each bookmarks as b}
              <div 
                class="bookmark-item" 
                onclick={() => navigateToBookmark(b.path)}
                role="button"
                tabindex="0"
                onkeydown={(e: KeyboardEvent) => { if (e.key === 'Enter' || e.key === ' ') navigateToBookmark(b.path); }}
              >
                <div class="bookmark-info">
                  <span class="bookmark-name">{b.name}</span>
                  <span class="bookmark-path mono-val">{b.path}</span>
                </div>
                <button class="icon-btn-compact delete-bookmark-btn" onclick={(e: MouseEvent) => removeBookmark(b.path, e)} title="Remove bookmark">
                  <Trash2 size={12} />
                </button>
              </div>
            {/each}
            {#if bookmarks.length === 0}
              <div class="no-bookmarks">
                <span>No bookmarks saved.</span>
              </div>
            {/if}
          </div>
        </div>
      {/if}
    </div>

    <div class="actions-group">
      <button class="secondary btn-compact" onclick={pickAndUpload}>
        <Upload size={14} /> Upload files
      </button>
      <button class="secondary btn-compact" onclick={pickFolderAndUpload}>
        <Upload size={14} /> Upload folder
      </button>
      <button class="secondary btn-compact" onclick={onNewFile}>
        <Plus size={14} /> New file
      </button>
      <button class="secondary btn-compact" onclick={onNewDir}>
        <Plus size={14} /> New folder
      </button>
    </div>
    <div class="actions-divider"></div>
    <SftpBulkActionsBar
      inline
      selectedCount={selectedPaths.size}
      {moveMode}
      onDownload={handleBulkDownload}
      onDelete={handleBulkDelete}
      onMove={startMoveMode}
      onMoveHere={handleMoveHere}
      onCancelMove={() => (moveMode = false)}
      onClearSelection={clearSelection}
    />
  </div>

  <div class="search-panel glass" class:search-active={isSearchLoading && recursiveSearch}>
    <div class="search-panel-row">
      <div class="search-panel-input-row">
        <span class="search-leading-icon-wrapper">
          {#if isSearchLoading && recursiveSearch && searchQuery.trim()}
            <Loader2 size={14} class="spin" />
          {:else}
            <Search size={14} />
          {/if}
        </span>
        <input
          type="text"
          class="search-panel-input mono-val"
          placeholder={recursiveSearch
            ? "Search in subfolders…"
            : "Search in this folder…"}
          bind:value={searchQuery}
          oninput={onSearchInput}
          onkeydown={(e) => e.key === 'Enter' && recursiveSearch && runRecursiveSearch()}
        />
        {#if searchQuery}
          <button class="search-clear" type="button" onclick={clearSearch} title="Clear">
            <X size={13} />
          </button>
        {/if}
      </div>
      <div class="search-scope-toggle" role="radiogroup" aria-label="Search scope">
        <button
          type="button"
          class="scope-btn"
          class:active={!recursiveSearch}
          role="radio"
          aria-checked={!recursiveSearch}
          onclick={() => setSearchScope(false)}
        >
          This folder
        </button>
        <button
          type="button"
          class="scope-btn"
          class:active={recursiveSearch}
          role="radio"
          aria-checked={recursiveSearch}
          onclick={() => setSearchScope(true)}
        >
          Subfolders
        </button>
      </div>
      <label class="option-toggle" title="Hide files and folders starting with a dot">
        <input type="checkbox" bind:checked={hideHidden} onchange={onHideHiddenChange} />
        <span>Hide hidden</span>
      </label>
    </div>

    {#if recursiveSearch}
      <div class="search-scope-row">
        <span class="search-scope-label">Search from path:</span>
        <input
          type="text"
          class="search-scope-path mono-val"
          bind:value={searchRootPath}
          placeholder={lastLoadedPath}
          onkeydown={(e) => e.key === 'Enter' && runRecursiveSearch()}
        />
        <button
          class="secondary btn-compact scope-reset-btn"
          type="button"
          onclick={() => (searchRootPath = lastLoadedPath)}
          title="Current folder"
        >
          Current folder
        </button>
      </div>
    {:else if searchQuery.trim()}
      <p class="search-scope-hint">Filters list in <span class="mono-val">{lastLoadedPath}</span></p>
    {/if}
  </div>

  {#if recursiveSearch && searchQuery.trim()}
    <div class="search-status glass" class:searching={isSearchLoading} class:done={!isSearchLoading}>
      {#if isSearchLoading}
        <Loader2 size={16} class="spin status-icon" />
        <span>{`Searching in ${effectiveSearchRoot}…`}</span>
      {:else}
        <Search size={16} class="status-icon done-icon" />
        <span>
          {displayFiles.length === 1
            ? `Done — ${String(displayFiles.length)} result`
            : `Done — ${String(displayFiles.length)} results`}
          {#if displayFiles.length === 0}
            <span class="status-hint">{`(no matches for "${searchQuery}")`}</span>
          {/if}
        </span>
      {/if}
    </div>
  {/if}

  <div class="files-area">
    <div
      class="files-table-container glass"
      class:drop-active={externalDragOver}
      class:search-pending={isSearchLoading && recursiveSearch && searchQuery.trim()}
    >
    {#if externalDragOver}
      <div class="drop-overlay">
        <Upload size={32} />
        <p>Drop files to upload to server</p>
      </div>
    {/if}

    {#if isSearchLoading && recursiveSearch && searchQuery.trim()}
      <div class="loading-state overlay-loading">
        <Loader2 class="spin" size={32} />
        <p>{`Searching in ${effectiveSearchRoot}…`}</p>
        <p class="loading-hint">This may take a while for large directories</p>
      </div>
    {/if}

    {#if isLoading && files.length === 0 && !(isSearchLoading && recursiveSearch)}
      <div class="loading-state">
        <RefreshCw class="spin" size={32} />
        <p>Loading file structure…</p>
      </div>
    {:else}
      <table class="files-table">
        <thead>
          <tr>
            <th style="width: 36px;">
              <input
                type="checkbox"
                checked={allSelected}
                onchange={toggleSelectAll}
                title="Select all"
              />
            </th>
            <SortableTh label="Name" column="name" activeColumn={fileSort.column} direction={fileSort.direction} onsort={setFileSort} width={isRecursiveActive ? '28%' : '34%'} />
            <SortableTh label="Size" column="size" activeColumn={fileSort.column} direction={fileSort.direction} onsort={setFileSort} width="12%" />
            {#if isRecursiveActive}
              <th style="width: 18%; padding: 14px 16px; font-size: 0.8rem; text-transform: uppercase; color: var(--text-muted); font-weight: 600;">Path</th>
            {/if}
            <SortableTh label="Permissions" column="permissions" activeColumn={fileSort.column} direction={fileSort.direction} onsort={setFileSort} width="12%" />
            <SortableTh label="Modified" column="modified" activeColumn={fileSort.column} direction={fileSort.direction} onsort={setFileSort} width="14%" />
            <th style="width: {isRecursiveActive ? '20%' : '26%'}; text-align: right; padding: 14px 16px; font-size: 0.8rem; text-transform: uppercase; color: var(--text-muted); font-weight: 600;">Actions</th>
          </tr>
        </thead>
        <tbody>
          {#each displayFiles as file, index}
            {@const remotePath = fileRemotePath(file, lastLoadedPath)}
            {@const isSelected = selectedPaths.has(remotePath)}
            <tr
              class:folder-row={file.is_dir}
              class:file-row={!file.is_dir}
              class:selected={isSelected}
              class:drop-target={!isRecursiveActive && dragOverFolder === file.name}
              draggable={!isRecursiveActive}
              onclick={(e) => handleRowClick(file, remotePath, index, e)}
              ondblclick={(e) => handleRowDblClick(file, remotePath, e)}
              ondragstart={!isRecursiveActive ? (e) => handleDragStart(e, file) : undefined}
              oncontextmenu={(e) => handleContextMenu(e, file)}
              ondragover={file.is_dir && !isRecursiveActive ? (e) => handleDragOverFolder(e, file.name) : undefined}
              ondragleave={file.is_dir && !isRecursiveActive ? handleDragLeaveFolder : undefined}
              ondrop={file.is_dir && !isRecursiveActive ? (e) => handleDropOnFolder(e, file.name) : undefined}
            >
              <td onclick={(e) => e.stopPropagation()}>
                <input
                  type="checkbox"
                  checked={isSelected}
                  onclick={(e) => {
                    e.stopPropagation();
                    toggleSelect(remotePath, index, e);
                  }}
                />
              </td>
              <td class="file-name-cell" title="Double-click to open">
                {#if file.is_dir}
                  <Folder size={18} class="folder-icon" />
                {:else}
                  <File size={18} class="file-icon" />
                {/if}
                <span>{file.name}</span>
              </td>
              <td class="mono-val">{formatEntrySize(file)}</td>
              {#if isRecursiveActive}
                <td class="mono-val path-cell" title={remotePath}>{parentPath(remotePath)}</td>
              {/if}
              <td><span class="badge badge-permission">{formatPermissions(file.permissions)}</span></td>
              <td class="mono-val date-cell">{formatModified(file.modified)}</td>
              <td class="actions-cell" onclick={(e) => e.stopPropagation()}>
                {#if !file.is_dir}
                  <button class="btn-table" onclick={() => onEdit(remotePath, file)} title="Edit">
                    <Edit size={14} />
                  </button>
                {/if}
                <button class="btn-table" onclick={() => downloadSingle(file)} title="Download">
                  <Download size={14} />
                </button>
                <button class="btn-table" onclick={() => startMoveSingle(file)} title="Move">
                  <FolderInput size={14} />
                </button>
                <button class="btn-table" onclick={() => onChmod(file)} title="Permissions (chmod)">
                  <KeyRound size={14} />
                </button>
                <button class="btn-table" onclick={() => onRename(file)} title="Rename">
                  <CornerDownLeft size={14} />
                </button>
                <button class="btn-table danger-text" onclick={() => deleteSingle(file)} title="Delete">
                  <Trash2 size={14} />
                </button>
              </td>
            </tr>
          {/each}

          {#if displayFiles.length === 0 && !isLoading && !isSearchLoading}
            <tr>
              <td colspan={isRecursiveActive ? 7 : 6} class="empty-state">
                {#if isRecursiveActive}
                  {`No results for "${searchQuery}" in ${effectiveSearchRoot}`}
                {:else if searchQuery.trim()}
                  {`No results for "${searchQuery}"`}
                {:else}
                  Directory is empty — drag files to upload
                {/if}
              </td>
            </tr>
          {/if}
        </tbody>
      </table>
    {/if}
    </div>
  </div>
</div>

{#if showContextMenu && contextMenuFile}
  <div
    class="context-menu glass"
    style="top: {contextMenuY}px; left: {contextMenuX}px;"
    onclick={(e) => e.stopPropagation()}
  >
    {#if !contextMenuFile.is_dir}
      <button
        class="menu-item"
        onclick={() => {
          onEdit(fileRemotePath(contextMenuFile!, lastLoadedPath), contextMenuFile!);
          closeContextMenu();
        }}
      >
        <Edit size={14} /> <span>Edit file</span>
      </button>
    {/if}
    <button
      class="menu-item"
      onclick={() => {
        downloadSingle(contextMenuFile!);
        closeContextMenu();
      }}
    >
      <Download size={14} /> <span>Download</span>
    </button>
    <button
      class="menu-item"
      onclick={() => {
        startMoveSingle(contextMenuFile!);
        closeContextMenu();
      }}
    >
      <FolderInput size={14} /> <span>Move</span>
    </button>
    <button
      class="menu-item"
      onclick={() => {
        onChmod(contextMenuFile!);
        closeContextMenu();
      }}
    >
      <KeyRound size={14} /> <span>Permissions (chmod)</span>
    </button>
    <button
      class="menu-item"
      onclick={() => {
        onRename(contextMenuFile!);
        closeContextMenu();
      }}
    >
      <CornerDownLeft size={14} /> <span>Rename</span>
    </button>
    <hr class="menu-divider" />
    <button
      class="menu-item danger-text"
      onclick={() => {
        deleteSingle(contextMenuFile!);
        closeContextMenu();
      }}
    >
      <Trash2 size={14} /> <span>Delete</span>
    </button>
  </div>
{/if}

{#if showConflictModal}
  <div class="modal-overlay">
    <div class="modal-content glass">
      <h3>File name conflict</h3>
      <p class="conflict-desc">
        {`${String(conflictFiles.length)} file(s) already exist in this directory. What should we do?`}
      </p>
      <ul class="conflict-list">
        {#each conflictFiles.slice(0, 5) as name}
          <li class="mono-val">{name}</li>
        {/each}
        {#if conflictFiles.length > 5}
          <li>{`…and ${String(conflictFiles.length - 5)} more`}</li>
        {/if}
      </ul>
      <div class="conflict-options">
        <label>
          <input type="radio" bind:group={conflictAction} value="overwrite" />
          Overwrite existing
        </label>
        <label>
          <input type="radio" bind:group={conflictAction} value="skip" />
          Skip existing
        </label>
        <label>
          <input type="radio" bind:group={conflictAction} value="rename" />
          Rename (append .1)
        </label>
      </div>
      <div class="modal-actions">
        <button class="primary" onclick={resolveConflict}>Continue</button>
        <button
          class="secondary"
          onclick={() => {
            showConflictModal = false;
            pendingUploadPaths = [];
          }}
        >
          Cancel
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .browser {
    display: flex;
    flex-direction: column;
    gap: 8px;
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }

  .option-toggle {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 0.75rem;
    color: var(--text-secondary);
    cursor: pointer;
    user-select: none;
    white-space: nowrap;
    flex-shrink: 0;
  }

  .option-toggle input {
    accent-color: var(--accent-amber);
  }

  .search-panel {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 8px 10px;
    border-radius: var(--radius-md);
    flex-shrink: 0;
    border: 1px solid var(--border-color);
    transition: border-color 0.2s ease, box-shadow 0.2s ease;
  }

  .search-panel.search-active {
    border-color: rgba(245, 158, 11, 0.45);
    box-shadow: 0 0 0 1px rgba(245, 158, 11, 0.12);
  }

  .search-panel-row {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }

  .search-panel-input-row {
    position: relative;
    display: flex;
    align-items: center;
    flex: 1;
    min-width: 160px;
  }

  .search-leading-icon-wrapper {
    position: absolute;
    left: 10px;
    top: 50%;
    transform: translateY(-50%);
    color: var(--text-muted);
    flex-shrink: 0;
    pointer-events: none;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }

  .search-scope-toggle {
    display: inline-flex;
    padding: 2px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-color);
    background: rgba(0, 0, 0, 0.2);
    gap: 2px;
    flex-shrink: 0;
  }

  .scope-btn {
    border: none;
    background: transparent;
    color: var(--text-muted);
    font-size: 0.72rem;
    padding: 4px 8px;
    min-height: 28px;
    border-radius: calc(var(--radius-sm) - 2px);
    cursor: pointer;
    transition:
      color 0.15s ease,
      background 0.15s ease,
      transform 0.12s cubic-bezier(0.2, 0, 0, 1);
  }

  .scope-btn:hover:not(.active) {
    color: var(--text-secondary);
    background: rgba(255, 255, 255, 0.04);
  }

  .scope-btn.active {
    color: var(--accent-amber);
    background: rgba(245, 158, 11, 0.14);
    font-weight: 600;
  }

  .scope-btn:active {
    transform: scale(0.96);
  }

  .search-panel-input {
    width: 100%;
    padding: 6px 32px 6px 34px;
    font-size: 0.82rem;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-color);
    background: rgba(0, 0, 0, 0.25);
    color: var(--text-primary);
    transition: border-color 0.15s ease;
  }

  .search-panel-input:focus {
    outline: none;
    border-color: rgba(245, 158, 11, 0.45);
  }

  .search-scope-row {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
    padding-top: 2px;
  }

  .search-scope-label {
    font-size: 0.78rem;
    color: var(--text-muted);
    flex-shrink: 0;
  }

  .search-scope-path {
    flex: 1;
    min-width: 160px;
    padding: 7px 10px;
    font-size: 0.82rem;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-color);
    background: rgba(0, 0, 0, 0.2);
    color: var(--text-primary);
  }

  .scope-reset-btn {
    flex-shrink: 0;
    min-height: 36px;
  }

  .search-scope-hint {
    margin: 0;
    font-size: 0.78rem;
    color: var(--text-muted);
    text-wrap: pretty;
  }

  .search-status {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 10px;
    border-radius: var(--radius-md);
    font-size: 0.78rem;
    flex-shrink: 0;
    border: 1px solid var(--border-color);
    transition: border-color 0.2s ease, background 0.2s ease;
  }

  .search-status.searching {
    border-color: rgba(245, 158, 11, 0.45);
    background: rgba(245, 158, 11, 0.06);
    color: var(--accent-amber);
  }

  .search-status.done {
    border-color: rgba(16, 185, 129, 0.35);
    background: rgba(16, 185, 129, 0.06);
    color: var(--text-secondary);
  }

  .search-status .status-icon {
    flex-shrink: 0;
  }

  .search-status .done-icon {
    color: #10b981;
  }

  .search-status .status-hint {
    color: var(--text-muted);
    font-size: 0.8rem;
  }

  .files-table-container.search-pending {
    pointer-events: none;
    opacity: 0.55;
  }

  .overlay-loading .loading-hint {
    font-size: 0.8rem;
    color: var(--text-muted);
    margin-top: 4px;
  }

  .btn-sm {
    padding: 4px 10px;
    font-size: 0.75rem;
  }

  .path-cell {
    font-size: 0.78rem;
    color: var(--text-secondary);
    max-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .overlay-loading {
    position: absolute;
    inset: 0;
    z-index: 5;
    background: rgba(0, 0, 0, 0.35);
    backdrop-filter: blur(2px);
  }

  .nav-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    border-radius: var(--radius-md);
    flex-shrink: 0;
    position: relative;
    flex-wrap: wrap;
  }

  .nav-bar.drop-active,
  .files-table-container.drop-active {
    outline: 2px dashed var(--accent-amber);
    outline-offset: -2px;
  }

  .path-input {
    flex: 1;
    min-width: 120px;
    font-family: var(--font-mono);
    font-size: 0.8rem;
    padding: 6px 10px;
  }

  .actions-group {
    display: flex;
    gap: 6px;
    flex-shrink: 0;
    flex-wrap: wrap;
  }

  .search-clear {
    position: absolute;
    right: 6px;
    background: transparent;
    border: none;
    padding: 6px;
    min-width: 36px;
    min-height: 36px;
    color: var(--text-muted);
    cursor: pointer;
    border-radius: var(--radius-sm);
    display: flex;
    align-items: center;
    justify-content: center;
    transition:
      color 0.15s ease,
      background 0.15s ease,
      transform 0.12s cubic-bezier(0.2, 0, 0, 1);
  }

  .search-clear:hover {
    color: var(--text-primary);
    background: var(--bg-hover);
  }

  .search-clear:active {
    transform: scale(0.96);
  }

  .files-area {
    position: relative;
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }

  .files-table-container {
    flex: 1;
    overflow-y: auto;
    border-radius: var(--radius-md);
    position: relative;
  }

  .drop-overlay {
    position: absolute;
    inset: 0;
    z-index: 10;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 12px;
    background: rgba(0, 0, 0, 0.55);
    backdrop-filter: blur(2px);
    color: var(--accent-primary);
    pointer-events: none;
  }

  .files-table {
    width: 100%;
    border-collapse: collapse;
    text-align: left;
  }

  .files-table th,
  .files-table td {
    padding: 8px 12px;
    border-bottom: 1px solid var(--border-color);
  }

  .files-table th {
    font-size: 0.7rem;
    text-transform: uppercase;
    color: var(--text-muted);
    font-weight: 600;
    letter-spacing: 0.05em;
    position: sticky;
    top: 0;
    background: var(--bg-secondary);
    z-index: 1;
  }

  .files-table tr {
    transition: background 0.15s ease;
  }

  .folder-row {
    cursor: default;
  }

  .file-row {
    cursor: default;
  }

  .folder-row:hover,
  .file-row:hover {
    background: var(--bg-hover);
  }

  tr.selected {
    background: var(--accent-muted) !important;
  }

  tr.drop-target {
    outline: 2px solid var(--accent-primary);
    outline-offset: -2px;
    background: var(--accent-muted);
  }

  .file-name-cell {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 0.9rem;
    color: var(--text-primary);
    user-select: none;
  }

  .folder-icon {
    color: var(--accent-primary);
  }

  .file-icon {
    color: var(--text-secondary);
  }

  .date-cell {
    font-size: 0.8rem;
    color: var(--text-secondary);
    font-variant-numeric: tabular-nums;
  }

  .actions-cell {
    text-align: right;
    display: flex;
    justify-content: flex-end;
    gap: 4px;
  }

  .btn-table {
    background: transparent;
    border: none;
    padding: 6px;
    border-radius: 4px;
    color: var(--text-secondary);
    cursor: pointer;
    transition: var(--transition-fast);
  }

  .btn-table:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .danger-text:hover {
    color: var(--accent-red) !important;
    background: var(--accent-red-glow) !important;
  }

  .loading-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    gap: 16px;
    color: var(--text-secondary);
    min-height: 200px;
  }

  .empty-state {
    text-align: center;
    color: var(--text-muted);
    font-size: 0.9rem;
    padding: 40px !important;
  }

  .context-menu {
    position: fixed;
    z-index: 1000;
    min-width: 180px;
    background: rgba(20, 20, 25, 0.92);
    border: 1px solid var(--border-color);
    border-radius: var(--radius-md);
    box-shadow: 0 10px 25px rgba(0, 0, 0, 0.5);
    padding: 6px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .menu-item {
    background: transparent;
    border: none;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 12px;
    font-size: 0.88rem;
    color: var(--text-secondary);
    border-radius: var(--radius-sm);
    cursor: pointer;
    text-align: left;
    width: 100%;
  }

  .menu-item:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .menu-divider {
    border: none;
    border-top: 1px solid var(--border-color);
    margin: 4px 0;
  }

  .modal-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    backdrop-filter: blur(4px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 150;
  }

  .modal-content {
    width: 420px;
    padding: 24px;
    border-radius: var(--radius-md);
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .modal-content h3 {
    color: white;
    font-size: 1.1rem;
    margin: 0;
  }

  .conflict-desc {
    font-size: 0.88rem;
    color: var(--text-secondary);
  }

  .conflict-list {
    font-size: 0.82rem;
    color: var(--text-muted);
    padding-left: 18px;
    margin: 0;
  }

  .conflict-options {
    display: flex;
    flex-direction: column;
    gap: 8px;
    font-size: 0.85rem;
    color: var(--text-secondary);
  }

  .conflict-options label {
    display: flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
  }

  .modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
  }

  /* Bookmarks Dropdown Styles */
  .bookmarks-container {
    position: relative;
    display: inline-block;
  }

  .bookmark-btn.bookmarked {
    color: var(--accent-amber, #f59e0b);
    border-color: rgba(245, 158, 11, 0.3);
  }

  .bookmark-btn.bookmarked:hover {
    background: rgba(245, 158, 11, 0.1);
  }

  :global(.star-active) {
    fill: var(--accent-amber, #f59e0b) !important;
    color: var(--accent-amber, #f59e0b) !important;
  }

  .bookmarks-dropdown {
    position: absolute;
    top: calc(100% + 8px);
    left: 0;
    width: 280px;
    max-height: 320px;
    border-radius: var(--radius-md);
    border: 1px solid var(--border-color);
    box-shadow: 0 10px 25px rgba(0, 0, 0, 0.4);
    z-index: 200;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .bookmarks-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 12px;
    border-bottom: 1px solid var(--border-color);
    font-size: 0.8rem;
    font-weight: 600;
    background: rgba(255, 255, 255, 0.02);
  }

  .btn-text-action {
    background: transparent;
    border: none;
    color: var(--accent-color, #3b82f6);
    font-size: 0.75rem;
    font-weight: 500;
    cursor: pointer;
    padding: 2px 6px;
    border-radius: 4px;
    transition: background-color 0.15s;
  }

  .btn-text-action:hover {
    background: rgba(59, 130, 246, 0.1);
  }

  .bookmarks-list {
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    padding: 6px;
  }

  .bookmark-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 6px 8px;
    border-radius: var(--radius-sm);
    cursor: pointer;
    transition: background-color 0.15s;
  }

  .bookmark-item:hover {
    background: var(--bg-hover);
  }

  .bookmark-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
    overflow: hidden;
    flex: 1;
  }

  .bookmark-name {
    font-size: 0.85rem;
    font-weight: 500;
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .bookmark-path {
    font-size: 0.72rem;
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .delete-bookmark-btn {
    opacity: 0;
    transition: opacity 0.15s;
  }

  .bookmark-item:hover .delete-bookmark-btn {
    opacity: 1;
  }

  .no-bookmarks {
    padding: 16px;
    text-align: center;
    font-size: 0.8rem;
    color: var(--text-muted);
  }
</style>
