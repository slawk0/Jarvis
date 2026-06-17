<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { Folder, File } from 'lucide-svelte';
  import type { FileInfo } from '$lib/sftp/types';

  interface Props {
    value: string;
    placeholder?: string;
    class?: string;
    onlyDirs?: boolean;
    onSelect?: (value: string) => void;
    onEnter?: () => void;
  }

  let {
    value = $bindable(),
    placeholder = '',
    class: className = '',
    onlyDirs = true,
    onSelect,
    onEnter
  }: Props = $props();

  let containerEl = $state<HTMLDivElement | null>(null);
  let dropdownEl = $state<HTMLUListElement | null>(null);
  let suggestions = $state<FileInfo[]>([]);
  let showDropdown = $state(false);
  let activeIndex = $state(-1);
  let isLoading = $state(false);
  let currentLoadedDir = $state('');

  // Scroll active item into view during keyboard navigation
  $effect(() => {
    if (showDropdown && activeIndex >= 0 && dropdownEl) {
      const activeLi = dropdownEl.children[activeIndex] as HTMLElement;
      if (activeLi) {
        activeLi.scrollIntoView({ block: 'nearest' });
      }
    }
  });

  // Helper to parse path into parent directory and last segment query
  function parsePath(fullPath: string) {
    const lastSlashIndex = fullPath.lastIndexOf('/');
    if (lastSlashIndex === -1) {
      return { dir: '', query: fullPath };
    } else {
      const dir = fullPath.slice(0, lastSlashIndex + 1);
      const query = fullPath.slice(lastSlashIndex + 1);
      return { dir, query };
    }
  }

  const parsed = $derived(parsePath(value));
  
  // Filter loaded items by the query
  const filteredSuggestions = $derived(
    suggestions.filter(item => {
      // If we only want directories, exclude files
      if (onlyDirs && !item.is_dir) return false;
      // Do not suggest "." or ".."
      if (item.name === '.' || item.name === '..') return false;
      // Filter by query prefix
      return item.name.toLowerCase().startsWith(parsed.query.toLowerCase()) && item.name !== parsed.query;
    })
  );

  const isMono = $derived(className.includes('mono-val'));

  // Load items when the directory part changes
  $effect(() => {
    const dir = parsed.dir;
    if (showDropdown && dir !== currentLoadedDir) {
      loadSuggestions(dir);
    }
  });

  async function loadSuggestions(dir: string) {
    // Check if the directory path is absolute
    if (!dir.startsWith('/')) {
      suggestions = [];
      currentLoadedDir = dir;
      return;
    }
    
    isLoading = true;
    try {
      const result = await invoke<FileInfo[]>('sftp_list', { path: dir });
      suggestions = result || [];
      currentLoadedDir = dir;
    } catch (err) {
      // Fail silently for suggestions to avoid interrupting the user
      console.warn('Failed to load path suggestions:', err);
      suggestions = [];
    } finally {
      isLoading = false;
    }
  }

  function handleSelect(item: FileInfo) {
    const dir = parsed.dir;
    const newPath = dir + item.name + (item.is_dir ? '/' : '');
    value = newPath;
    showDropdown = false;
    activeIndex = -1;
    if (onSelect) {
      onSelect(newPath);
    }
  }

  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === 'Enter' || e.key === 'Tab') {
      if (showDropdown && activeIndex >= 0 && activeIndex < filteredSuggestions.length) {
        e.preventDefault();
        handleSelect(filteredSuggestions[activeIndex]);
      } else if (e.key === 'Enter') {
        showDropdown = false;
        if (onEnter) {
          onEnter();
        }
      }
    } else if (e.key === 'ArrowDown') {
      if (!showDropdown) {
        showDropdown = true;
        activeIndex = 0;
      } else if (filteredSuggestions.length > 0) {
        e.preventDefault();
        activeIndex = (activeIndex + 1) % filteredSuggestions.length;
      }
    } else if (e.key === 'ArrowUp') {
      if (showDropdown && filteredSuggestions.length > 0) {
        e.preventDefault();
        activeIndex = (activeIndex - 1 + filteredSuggestions.length) % filteredSuggestions.length;
      }
    } else if (e.key === 'Escape') {
      showDropdown = false;
      activeIndex = -1;
    }
  }

  function handleFocus() {
    showDropdown = true;
    if (parsed.dir !== currentLoadedDir) {
      loadSuggestions(parsed.dir);
    }
  }

  function handleFocusOut(e: FocusEvent) {
    // Close dropdown if focus shifts outside this component container
    if (containerEl && !containerEl.contains(e.relatedTarget as Node)) {
      showDropdown = false;
      activeIndex = -1;
    }
  }

  function handleClickOutside(e: MouseEvent) {
    if (containerEl && !containerEl.contains(e.target as Node)) {
      showDropdown = false;
      activeIndex = -1;
    }
  }
</script>

<svelte:window onclick={handleClickOutside} />

<div
  class="autocomplete-container"
  bind:this={containerEl}
  onfocusout={handleFocusOut}
>
  <input
    type="text"
    placeholder={placeholder}
    class={className}
    bind:value={value}
    onkeydown={handleKeyDown}
    onfocus={handleFocus}
  />
  
  {#if showDropdown && filteredSuggestions.length > 0}
    <ul bind:this={dropdownEl} class="autocomplete-dropdown glass" class:mono-val={isMono}>
      {#each filteredSuggestions as item, index}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
        <li
          class="autocomplete-item"
          class:active={index === activeIndex}
          onclick={() => handleSelect(item)}
        >
          <span class="item-icon">
            {#if item.is_dir}
              <Folder size={14} />
            {:else}
              <File size={14} />
            {/if}
          </span>
          <span class="item-name">{item.name}</span>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .autocomplete-container {
    position: relative;
    display: flex;
    flex: 1;
    min-width: 120px;
  }

  .autocomplete-container :global(input) {
    width: 100%;
    flex: 1;
  }

  .autocomplete-dropdown {
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    width: 100%;
    max-height: 200px;
    overflow-y: auto;
    z-index: 1000;
    margin: 0;
    padding: 4px 0;
    list-style: none;
    border-radius: var(--radius-sm);
    background: var(--bg-element) !important;
    border: 1px solid var(--border-white) !important;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.5) !important;
  }

  .autocomplete-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 12px;
    font-size: 0.82rem;
    cursor: pointer;
    transition: background-color var(--transition-fast), color var(--transition-fast);
    color: var(--text-primary);
  }

  .autocomplete-item:hover,
  .autocomplete-item.active {
    background-color: var(--accent-muted) !important;
    color: var(--text-primary);
  }

  .item-icon {
    display: inline-flex;
    align-items: center;
    color: var(--text-muted);
  }

  .autocomplete-item:hover .item-icon,
  .autocomplete-item.active .item-icon {
    color: var(--text-primary);
  }

  .item-name {
    font-family: var(--font-sans);
  }

  .mono-val .item-name {
    font-family: var(--font-mono);
  }
</style>
