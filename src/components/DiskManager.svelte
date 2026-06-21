<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import {
    HardDrive, RefreshCw, FolderOpen, AlertCircle,
    KeyRound, Loader2, X, CheckCircle2,
    Database, Copy, Check, Maximize2
  } from 'lucide-svelte';
  import SudoModal from './SudoModal.svelte';
  import { get } from 'svelte/store';
    import { notifications } from '$lib/notifications.svelte';
  import { formatInvokeError } from '$lib/backendErrors';

  let { profileId = '', visible = true } = $props();

  export function refresh() { loadData(); }

  // State lists
  let dfList = $state<any[]>([]);
  let rawDevices = $state<any[]>([]);
  let isLoading = $state(false);
  let errorMsg = $state('');

  // View state
  let showSystemFs = $state(false);
  let showLoopDevices = $state(false);

  // Sudo auth state
  let showSudoModal = $state(false);
  let sudoModalTitle = $state<string | undefined>(undefined);
  let sudoModalDesc = $state<string | undefined>(undefined);
  let pendingSudoAction = $state<(() => Promise<void>) | null>(null);
  let pendingSudoCancel = $state<(() => void) | null>(null);

  // Mount modal state
  let showMountModal = $state(false);
  let mountDeviceName = $state('');
  let mountPath = $state('');
  let mountCreateDir = $state(true);
  let mountLoading = $state(false);

  // Partition modal state
  let showPartitionModal = $state(false);
  let partDiskName = $state('');
  let partLabelType = $state('gpt');
  let partFsType = $state('ext4');
  let partUseEntire = $state(true);
  let partConfirm = $state(false);
  let partLoading = $state(false);

  // Expand partition modal state
  let showExpandModal = $state(false);
  let expandPartName = $state('');
  let expandFsType = $state('');
  let expandMountPoint = $state('');
  let expandLoading = $state(false);

  // FSCK / Output modal state
  let showOutputModal = $state(false);
  let outputTitle = $state('');
  let outputText = $state('');
  let outputLoading = $state(false);

  // Clipboard copy state
  let copiedUuid = $state('');

  // Fetch all data
  async function loadData() {
    isLoading = true;
    errorMsg = '';
    try {
      // 1. Fetch block devices
      const lsblkOut = await invoke<string>('exec_custom_command', {
        cmd: 'lsblk -o NAME,FSTYPE,SIZE,MOUNTPOINT,LABEL,UUID,TYPE,MODEL -J',
        useSudo: false
      });
      try {
        const lsblkJson = JSON.parse(lsblkOut);
        rawDevices = lsblkJson.blockdevices || [];
      } catch (e) {
        console.error('Failed to parse lsblk JSON:', lsblkOut, e);
        errorMsg = 'lsblk JSON parse failed. Block devices cannot be listed.';
      }

      // 2. Fetch disk space
      const dfOut = await invoke<string>('exec_custom_command', {
        cmd: 'df -hT',
        useSudo: false
      }).catch(async () => {
        // Fallback to df -h
        return await invoke<string>('exec_custom_command', {
          cmd: 'df -h',
          useSudo: false
        });
      });
      dfList = parseDf(dfOut);
    } catch (err: any) {
      console.error(err);
      errorMsg = formatInvokeError(err);
    } finally {
      isLoading = false;
    }
  }

  // Parse df output
  function parseDf(output: string): any[] {
    const parsed = [];
    const lines = output.trim().split('\n');
    if (lines.length <= 1) return [];
    
    for (let i = 1; i < lines.length; i++) {
      const line = lines[i].trim();
      if (!line) continue;
      const parts = line.split(/\s+/);
      if (parts.length >= 6) {
        const hasTypeColumn = parts[1].includes('/') || 
          ['ext4', 'ext3', 'xfs', 'vfat', 'tmpfs', 'udev', 'overlay', 'btrfs'].includes(parts[1].toLowerCase()) ||
          parts[2].endsWith('G') || parts[2].endsWith('M') || parts[2].endsWith('K') || parts[2] === '0' || parts[2].endsWith('T');

        if (hasTypeColumn && parts.length >= 7) {
          parsed.push({
            filesystem: parts[0],
            type: parts[1],
            size: parts[2],
            used: parts[3],
            avail: parts[4],
            usePercent: parts[5],
            usePctNum: parseInt(parts[5].replace('%', '')) || 0,
            mountedOn: parts[6]
          });
        } else {
          parsed.push({
            filesystem: parts[0],
            type: 'unknown',
            size: parts[1],
            used: parts[2],
            avail: parts[3],
            usePercent: parts[4],
            usePctNum: parseInt(parts[4].replace('%', '')) || 0,
            mountedOn: parts[5]
          });
        }
      }
    }
    return parsed;
  }

  // Derived states to categorize raw devices
  const physicalDisks = $derived(
    rawDevices.filter(d => d.type === 'disk' && !d.name.startsWith('loop'))
  );

  const loopDevices = $derived(
    rawDevices.filter(d => d.type === 'loop' || d.name.startsWith('loop'))
  );

  const otherDevices = $derived(
    rawDevices.filter(d => d.type !== 'disk' && d.type !== 'loop' && !d.name.startsWith('loop'))
  );

  const filteredDf = $derived(
    showSystemFs 
      ? dfList 
      : dfList.filter(df => 
          !df.filesystem.startsWith('tmpfs') && 
          !df.filesystem.startsWith('udev') && 
          !df.filesystem.startsWith('devtmpfs') &&
          df.mountedOn !== '/boot/efi'
        )
  );

  // Helper to parse sizes (e.g. "931,5G", "931.5G", "100M", "2.1T") into bytes
  function parseSizeToBytes(sizeStr: string): number {
    if (!sizeStr) return 0;
    const clean = sizeStr.trim().toLowerCase();
    // Allow dots and commas in numeric part
    const match = clean.match(/^([0-9.,]+)\s*([a-z]*)/);
    if (!match) return 0;
    
    // Replace comma with dot for correct parseFloat parsing
    const rawNumStr = match[1].replace(',', '.');
    const num = parseFloat(rawNumStr) || 0;
    const unit = match[2] || '';
    
    if (unit.startsWith('t')) return num * 1024 * 1024 * 1024 * 1024;
    if (unit.startsWith('g')) return num * 1024 * 1024 * 1024;
    if (unit.startsWith('m')) return num * 1024 * 1024;
    if (unit.startsWith('k')) return num * 1024;
    return num;
  }

  // Helper to format bytes to human readable size
  function formatBytes(bytes: number): string {
    if (bytes <= 0) return '0 B';
    const units = ['B', 'KB', 'MB', 'GB', 'TB'];
    const idx = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
    const val = bytes / Math.pow(1024, idx);
    return `${val.toFixed(1)} ${units[idx]}`;
  }

  // Get visual segments for a disk's partition bar
  function getDiskSegments(disk: any) {
    const totalBytes = parseSizeToBytes(disk.size);
    if (totalBytes <= 0) return [];

    const segments: any[] = [];
    let allocatedBytes = 0;

    const children = disk.children || [];
    for (const child of children) {
      const childBytes = parseSizeToBytes(child.size);
      allocatedBytes += childBytes;
      segments.push({
        name: child.name,
        size: child.size,
        bytes: childBytes,
        fstype: child.fstype || 'unformatted',
        mountpoint: child.mountpoint || '',
        label: child.label || '',
        uuid: child.uuid || '',
        type: 'partition',
        pct: (childBytes / totalBytes) * 100
      });
    }

    const freeBytes = totalBytes - allocatedBytes;
    if (freeBytes > 10 * 1024 * 1024) { // More than 10MB
      segments.push({
        name: 'Free Space',
        size: formatBytes(freeBytes),
        bytes: freeBytes,
        fstype: '',
        mountpoint: '',
        label: '',
        uuid: '',
        type: 'free',
        pct: (freeBytes / totalBytes) * 100
      });
    }

    return segments;
  }

  // Async Sudo Execution helper returning a promise
  async function execSudo(cmd: string, errorTitle: string): Promise<void> {
    try {
      const hasSudo = await invoke<boolean>('has_sudo_password');
      if (hasSudo) {
        await invoke('exec_custom_command', { cmd, useSudo: true });
        return;
      }
    } catch { /* ignore */ }

    return new Promise<void>((resolve, reject) => {
      sudoModalTitle = "Disk Management";
      sudoModalDesc = undefined;
      pendingSudoAction = async () => {
        try {
          await invoke('exec_custom_command', { cmd, useSudo: true });
          resolve();
        } catch (sudoErr: unknown) {
          notifications.error(`${errorTitle}: ${formatInvokeError(sudoErr)}`);
          reject(sudoErr);
        }
      };
      pendingSudoCancel = () => {
        reject(new Error('Sudo authorization cancelled'));
      };
      showSudoModal = true;
    });
  }

  // Mount Device
  function openMountModal(devName: string) {
    mountDeviceName = devName;
    mountPath = `/mnt/${devName}`;
    showMountModal = true;
  }

  async function executeMount() {
    if (!mountPath.trim()) return;
    mountLoading = true;
    const devPath = `/dev/${mountDeviceName}`;
    
    // Command sequence: mkdir if selected, then mount wrapped in bash -c
    const cmd = mountCreateDir
      ? `bash -c 'mkdir -p "${mountPath}" && mount "${devPath}" "${mountPath}"'`
      : `mount "${devPath}" "${mountPath}"`;

    try {
      await execSudo(cmd, 'Mount failed');
      notifications.success("Device mounted successfully");
      showMountModal = false;
      await loadData();
    } catch (err) {
      console.error(err);
    } finally {
      mountLoading = false;
    }
  }

  // Unmount Device
  async function executeUmount(devName: string) {
    const devPath = `/dev/${devName}`;
    const cmd = `umount ${devPath}`;
    
    try {
      await execSudo(cmd, 'Unmount failed');
      notifications.success("Device unmounted successfully");
      await loadData();
    } catch (err) {
      console.error(err);
    }
  }

  // FSCK operation
  async function runFsck(devName: string) {
    outputTitle = `fsck: /dev/${devName}`;
    outputText = 'Running filesystem consistency check...';
    showOutputModal = true;
    outputLoading = true;

    const cmd = `fsck -y /dev/${devName}`;
    try {
      await execSudo(cmd, 'fsck execution failed');
      try {
        const res = await invoke<string>('exec_custom_command', { cmd, useSudo: true });
        outputText = res || 'fsck finished successfully with no stdout.';
      } catch (err) {
        outputText = `Fsck finished with warning/output:\n${formatInvokeError(err)}`;
      }
    } catch (err) {
      outputText = `Fsck failed:\n${formatInvokeError(err)}`;
    } finally {
      outputLoading = false;
      await loadData();
    }
  }

  // Deduce partition name (e.g. nvme0n1 -> nvme0n1p1, sdb -> sdb1)
  function getPartitionDeviceName(diskName: string, partNum: number = 1): string {
    const endsWithDigit = /\d$/.test(diskName);
    if (endsWithDigit) {
      return `${diskName}p${partNum}`;
    }
    return `${diskName}${partNum}`;
  }

  // Parse partition name to extract parent disk and partition index
  function parsePartitionName(partName: string) {
    // Check for nvme0n1p1 style (ends with p + digits, preceded by digit)
    const nvmeMatch = partName.match(/^([a-z0-9]+n\d+)p(\d+)$/i);
    if (nvmeMatch) {
      return { disk: nvmeMatch[1], partNum: parseInt(nvmeMatch[2]) || 1 };
    }
    // Check for sda1 style (ends with digits, preceded by letter)
    const sdMatch = partName.match(/^([a-z]+)(\d+)$/i);
    if (sdMatch) {
      return { disk: sdMatch[1], partNum: parseInt(sdMatch[2]) || 1 };
    }
    // Fallback
    return { disk: partName, partNum: 1 };
  }

  // Partition disk
  function openPartitionModal(diskName: string) {
    partDiskName = diskName;
    partLabelType = 'gpt';
    partFsType = 'ext4';
    partUseEntire = true;
    partConfirm = false;
    showPartitionModal = true;
  }

  async function executePartition() {
    if (!partConfirm) return;
    partLoading = true;
    const diskPath = `/dev/${partDiskName}`;
    const partPath = `/dev/${getPartitionDeviceName(partDiskName, 1)}`;

    // Command chain wrapped in bash -c so all parts run as root
    const createLabelCmd = `parted -s ${diskPath} mklabel ${partLabelType}`;
    const createPartCmd = `parted -s ${diskPath} mkpart primary ${partFsType} 1MiB 100%`;
    const formatCmd = `mkfs.${partFsType} ${partPath}`;
    const fullCmd = `bash -c '${createLabelCmd} && ${createPartCmd} && sleep 1 && ${formatCmd}'`;

    try {
      await execSudo(fullCmd, 'Partition creation failed');
      notifications.success("Partition created successfully");
      showPartitionModal = false;
      await loadData();
    } catch (err) {
      console.error(err);
    } finally {
      partLoading = false;
    }
  }

  // Expand partition
  function openExpandModal(partName: string, fstype: string, mountpoint: string) {
    expandPartName = partName;
    expandFsType = fstype;
    expandMountPoint = mountpoint;
    showExpandModal = true;
  }

  async function executeExpand() {
    if (expandFsType === 'xfs' && !expandMountPoint) {
      notifications.error('XFS filesystems must be mounted to be expanded. Please mount the partition first.');
      return;
    }
    expandLoading = true;
    const { disk, partNum } = parsePartitionName(expandPartName);
    const diskPath = `/dev/${disk}`;
    const partPath = `/dev/${expandPartName}`;

    // Grow partition table boundary using growpart (safer on mounted disks) or parted resizepart fallback
    const resizePartCmd = `if command -v growpart >/dev/null 2>&1; then growpart ${diskPath} ${partNum}; else parted -s ${diskPath} resizepart ${partNum} 100%; fi`;

    // Grow filesystem size to fill the partition
    let growFsCmd = '';
    const fs = expandFsType.toLowerCase();
    if (fs === 'ext4' || fs === 'ext3') {
      growFsCmd = `resize2fs ${partPath}`;
    } else if (fs === 'xfs') {
      growFsCmd = expandMountPoint ? `xfs_growfs "${expandMountPoint}"` : `xfs_growfs ${partPath}`;
    } else if (fs === 'btrfs') {
      growFsCmd = expandMountPoint ? `btrfs filesystem resize max "${expandMountPoint}"` : `btrfs filesystem resize max ${partPath}`;
    }

    const cmd = growFsCmd 
      ? `bash -c '${resizePartCmd} && sleep 1 && ${growFsCmd}'`
      : `bash -c '${resizePartCmd}'`;

    try {
      await execSudo(cmd, 'Expand partition failed');
      notifications.success(`Partition /dev/${expandPartName} expanded successfully.`);
      showExpandModal = false;
      await loadData();
    } catch (err) {
      console.error(err);
    } finally {
      expandLoading = false;
    }
  }

  function copyToClipboard(text: string) {
    navigator.clipboard.writeText(text);
    copiedUuid = text;
    setTimeout(() => {
      if (copiedUuid === text) copiedUuid = '';
    }, 2000);
  }

  onMount(() => {
    loadData();
  });
</script>

<SudoModal
  bind:open={showSudoModal}
  title={sudoModalTitle}
  description={sudoModalDesc}
  onSuccess={() => {
    if (pendingSudoAction) pendingSudoAction();
  }}
  onCancel={() => {
    if (pendingSudoCancel) pendingSudoCancel();
  }}
/>

<div class="disk-manager manager-shell fade-in">
  <header class="manager-header">
    <div style="display: flex; align-items: center; gap: 10px;">
      <HardDrive size={24} class="accent-amber-text" />
      <h1 class="page-title">Disk Management</h1>
    </div>
  </header>

  <div class="ops-bar glass">
    <div style="display: flex; align-items: center; gap: 20px;">
      <label class="toggle-checkbox" style="display: flex; align-items: center; gap: 8px; cursor: pointer; font-size: 0.9rem;">
        <input type="checkbox" bind:checked={showSystemFs} />
        <span>Show system filesystems</span>
      </label>
      {#if loopDevices.length > 0}
        <label class="toggle-checkbox" style="display: flex; align-items: center; gap: 8px; cursor: pointer; font-size: 0.9rem;">
          <input type="checkbox" bind:checked={showLoopDevices} />
          <span>Show loop devices ({loopDevices.length})</span>
        </label>
      {/if}
    </div>
  </div>

  {#if isLoading && dfList.length === 0}
    <div class="loading-state glass" style="margin: 40px 0; padding: 60px; text-align: center;">
      <RefreshCw class="spin" size={48} />
      <p style="margin-top: 16px; color: var(--text-secondary);">Querying system storage info...</p>
    </div>
  {:else}
    <div class="disk-layout">
      <!-- 1. Disk usage cards -->
      <section class="disk-section">
        <h2 class="section-title">Disk Usage</h2>
        <div class="usage-grid">
          {#each filteredDf as df}
            <div class="usage-card glass">
              <div class="usage-card-header">
                <span class="fs-mount font-semibold">{df.mountedOn}</span>
                <span class="fs-type badge secondary">{df.type}</span>
              </div>
              <div class="fs-path mono-val text-muted">{df.filesystem}</div>
              <div class="progress-container">
                <div class="progress-label">
                  <span>{df.used} / {df.size}</span>
                  <span style="color: {df.usePctNum > 85 ? 'var(--color-danger, #ef4444)' : 'var(--text-secondary)'}">{df.usePercent}</span>
                </div>
                <div class="progress-bar-bg">
                  <div 
                    class="progress-bar" 
                    style="width: {df.usePctNum}%; background: {df.usePctNum > 85 ? 'var(--color-danger, #ef4444)' : df.usePctNum > 65 ? 'var(--color-warning, #f59e0b)' : 'var(--color-success, #10b981)'};"
                  ></div>
                </div>
              </div>
            </div>
          {/each}
        </div>
      </section>

      <!-- 2. Physical Disks & Partitions -->
      <section class="disk-section">
        <h2 class="section-title">Block Devices</h2>
        <div class="disks-list">
          {#each physicalDisks as disk}
            {@const segments = getDiskSegments(disk)}
            <div class="disk-card glass">
              <!-- Disk Header -->
              <div class="disk-card-header">
                <div class="disk-info-left">
                  <div class="disk-icon-box">
                    <Database size={20} class="accent-amber-text" />
                  </div>
                  <div style="display: flex; flex-direction: column;">
                    <div style="display: flex; align-items: center; gap: 8px;">
                      <span class="disk-name font-semibold">/dev/{disk.name}</span>
                      {#if disk.model}
                        <span class="disk-model text-secondary">({disk.model})</span>
                      {/if}
                    </div>
                    {#if disk.label}
                      <span class="badge secondary" style="align-self: flex-start; margin-top: 4px;">{disk.label}</span>
                    {/if}
                  </div>
                </div>
                <div class="disk-info-right" style="display: flex; align-items: center; gap: 12px;">
                  <span class="disk-size mono-val">{disk.size}</span>
                  <button class="btn-compact primary" onclick={() => openPartitionModal(disk.name)}>
                    + Create Partition
                  </button>
                </div>
              </div>

              <!-- Visual Partition Bar -->
              {#if segments.length > 0}
                <div class="partition-bar">
                  {#each segments as seg}
                    <div 
                      class="partition-segment {seg.type}" 
                      style="width: {seg.pct}%;"
                      title="{seg.name} ({seg.size}) - {seg.fstype || 'unallocated'} {seg.mountpoint ? 'mounted on ' + seg.mountpoint : ''}"
                    >
                      <span class="segment-label">{seg.name} <span class="segment-size">({seg.size})</span></span>
                    </div>
                  {/each}
                </div>
              {/if}

              <!-- Partitions Table -->
              <div class="partition-table-wrapper">
                <table class="partition-table">
                  <thead>
                    <tr>
                      <th>Device</th>
                      <th>Size</th>
                      <th>Filesystem</th>
                      <th>Mount Point</th>
                      <th>Label / UUID</th>
                      <th style="text-align: right;">Actions</th>
                    </tr>
                  </thead>
                  <tbody>
                    {#each disk.children || [] as part}
                      <tr class="part-row">
                        <td class="mono-val font-semibold">/dev/{part.name}</td>
                        <td class="mono-val tabular-nums">{part.size}</td>
                        <td><span class="badge secondary">{part.fstype || 'raw'}</span></td>
                        <td class="mono-val" style="font-size: 0.82rem; color: var(--text-primary);">{part.mountpoint || '—'}</td>
                        <td>
                          <div class="uuid-cell">
                            {#if part.label}
                              <span class="part-label-badge" title="Label">{part.label}</span>
                            {/if}
                            {#if part.uuid}
                              <span class="uuid-badge mono-val" title={part.uuid}>
                                {part.uuid.substring(0, 8)}...
                                <button class="copy-btn" onclick={() => copyToClipboard(part.uuid)} title="Copy UUID">
                                  {#if copiedUuid === part.uuid}
                                    <Check size={12} style="color: var(--color-success, #10b981);" />
                                  {:else}
                                    <Copy size={12} />
                                  {/if}
                                </button>
                              </span>
                            {/if}
                            {#if !part.label && !part.uuid}
                              <span class="text-muted">—</span>
                            {/if}
                          </div>
                        </td>
                        <td class="actions-cell" style="text-align: right;">
                          <button class="btn-action" onclick={() => openExpandModal(part.name, part.fstype, part.mountpoint)} title="Expand Partition">
                            <Maximize2 size={12} /> Expand
                          </button>
                          {#if part.mountpoint}
                            <button class="btn-action danger-text" onclick={() => executeUmount(part.name)}>
                              Unmount
                            </button>
                          {:else}
                            <button class="btn-action" onclick={() => openMountModal(part.name)}>
                              Mount
                            </button>
                            {#if part.fstype}
                              <button class="btn-action" onclick={() => runFsck(part.name)}>
                                Check FS (fsck)
                              </button>
                            {/if}
                          {/if}
                        </td>
                      </tr>

                      <!-- Nested LVM / crypt / children -->
                      {#if part.children && part.children.length > 0}
                        {#each part.children as sub}
                          <tr class="part-row sub-part-row">
                            <td class="mono-val text-muted" style="padding-left: 24px;">
                              <span style="font-family: monospace;">└─</span> /dev/{sub.name}
                            </td>
                            <td class="mono-val tabular-nums text-muted">{sub.size}</td>
                            <td><span class="badge info">{sub.fstype || sub.type || 'lvm'}</span></td>
                            <td class="mono-val" style="font-size: 0.82rem; color: var(--text-primary);">{sub.mountpoint || '—'}</td>
                            <td>
                              {#if sub.uuid}
                                <span class="uuid-badge mono-val" title={sub.uuid}>{sub.uuid.substring(0, 8)}...</span>
                              {:else}
                                <span class="text-muted">—</span>
                              {/if}
                            </td>
                            <td class="actions-cell" style="text-align: right;">
                              {#if sub.mountpoint}
                                <button class="btn-action danger-text" onclick={() => executeUmount(sub.name)}>
                                  Unmount
                                </button>
                              {:else}
                                <button class="btn-action" onclick={() => openMountModal(sub.name)}>
                                  Mount
                                </button>
                              {/if}
                            </td>
                          </tr>
                        {/each}
                      {/if}
                    {/each}

                    <!-- Free space row in table if available -->
                    {#each segments.filter(s => s.type === 'free') as freeSeg}
                      <tr class="free-space-row">
                        <td class="text-muted font-semibold" style="font-style: italic; padding-left: 12px;">Unallocated Space</td>
                        <td class="mono-val tabular-nums text-muted">{freeSeg.size}</td>
                        <td><span class="badge secondary" style="opacity: 0.5;">free</span></td>
                        <td class="text-muted">—</td>
                        <td class="text-muted">—</td>
                        <td style="text-align: right;">
                          <button class="btn-action success-text" onclick={() => openPartitionModal(disk.name)}>
                            + Create Partition
                          </button>
                        </td>
                      </tr>
                    {/each}

                    {#if (!disk.children || disk.children.length === 0) && segments.length === 0}
                      <tr>
                        <td colspan="6" class="empty-state" style="text-align: center; padding: 20px; color: var(--text-muted);">
                          No partition table detected on this disk.
                        </td>
                      </tr>
                    {/if}
                  </tbody>
                </table>
              </div>
            </div>
          {/each}
          {#if physicalDisks.length === 0 && !isLoading}
            <div class="empty-state glass" style="padding: 40px; text-align: center;">No physical disks detected.</div>
          {/if}
        </div>
      </section>

      <!-- Loop Devices Section -->
      {#if loopDevices.length > 0 && showLoopDevices}
        <section class="disk-section">
          <h2 class="section-title">Loop / Virtual Devices</h2>
          <div class="table-container glass">
            <table class="data-table">
              <thead>
                <tr>
                  <th>Device</th>
                  <th>Size</th>
                  <th>Filesystem</th>
                  <th>Mount Point</th>
                  <th style="text-align: right;">Actions</th>
                </tr>
              </thead>
              <tbody>
                {#each loopDevices as dev}
                  <tr>
                    <td class="mono-val">/dev/{dev.name}</td>
                    <td class="mono-val">{dev.size}</td>
                    <td><span class="badge secondary">{dev.fstype || 'loop'}</span></td>
                    <td class="mono-val">{dev.mountpoint || '—'}</td>
                    <td style="text-align: right;">
                      {#if dev.mountpoint}
                        <button class="btn-table danger-text" onclick={() => executeUmount(dev.name)}>Unmount</button>
                      {/if}
                    </td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        </section>
      {/if}

      <!-- Other/Uncategorized Devices -->
      {#if otherDevices.length > 0}
        <section class="disk-section">
          <h2 class="section-title">Other Block Devices</h2>
          <div class="table-container glass">
            <table class="data-table">
              <thead>
                <tr>
                  <th>Device</th>
                  <th>Type</th>
                  <th>Size</th>
                  <th>Filesystem</th>
                  <th>Mount Point</th>
                  <th style="text-align: right;">Actions</th>
                </tr>
              </thead>
              <tbody>
                {#each otherDevices as dev}
                  <tr>
                    <td class="mono-val">/dev/{dev.name}</td>
                    <td><span class="badge secondary">{dev.type}</span></td>
                    <td class="mono-val">{dev.size}</td>
                    <td><span class="badge secondary">{dev.fstype || '—'}</span></td>
                    <td class="mono-val">{dev.mountpoint || '—'}</td>
                    <td style="text-align: right;">
                      {#if dev.mountpoint}
                        <button class="btn-table danger-text" onclick={() => executeUmount(dev.name)}>Unmount</button>
                      {:else}
                        <button class="btn-table" onclick={() => openMountModal(dev.name)}>Mount</button>
                      {/if}
                    </td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        </section>
      {/if}
    </div>
  {/if}
</div>

<!-- Mount Path Modal -->
{#if showMountModal}
  <div class="modal-overlay" onclick={() => showMountModal = false} role="presentation">
    <div class="modal-content glass" onclick={(e) => e.stopPropagation()} role="dialog">
      <div class="modal-header-icon" style="margin-bottom: 0;">
        <FolderOpen size={32} class="accent-amber-text" />
      </div>
      <h3 style="text-align: center; margin: 0;">Mount: /dev/{mountDeviceName}</h3>
      
      <div class="form-group" style="margin-top: 10px;">
        <label for="mount-path-input">Mount Path</label>
        <input 
          id="mount-path-input"
          type="text" 
          placeholder="e.g. /mnt/storage" 
          bind:value={mountPath} 
          class="mono-val"
        />
      </div>

      <label class="toggle-checkbox" style="display: flex; align-items: center; gap: 8px; cursor: pointer; font-size: 0.9rem;">
        <input type="checkbox" bind:checked={mountCreateDir} />
        <span>Create directory if it does not exist</span>
      </label>

      <div class="modal-actions">
        <button class="primary" onclick={executeMount} disabled={mountLoading}>
          {#if mountLoading}
            <Loader2 class="spin" size={14} /> Executing...
          {:else}
            Mount
          {/if}
        </button>
        <button class="secondary" onclick={() => showMountModal = false}>Cancel</button>
      </div>
    </div>
  </div>
{/if}

<!-- Create Partition Modal -->
{#if showPartitionModal}
  <div class="modal-overlay" onclick={() => showPartitionModal = false} role="presentation">
    <div class="modal-content glass" style="width: 480px;" onclick={(e) => e.stopPropagation()} role="dialog">
      <div class="modal-header-icon" style="color: var(--color-danger, #ef4444);">
        <AlertCircle size={32} />
      </div>
      <h3 style="text-align: center; margin: 0;">Create Partition: /dev/{partDiskName}</h3>
      
      <div class="alert-box danger-box" style="margin: 10px 0;">
        <strong>Destructive Action Warning</strong>
        <p style="font-size: 0.8rem; margin: 4px 0 0 0;">Creating partition table and formatting will erase all data on the selected drive!</p>
      </div>

      <div class="form-row">
        <div class="form-group flex-1">
          <label for="partition-table-select">Partition Table</label>
          <select id="partition-table-select" bind:value={partLabelType}>
            <option value="gpt">GPT (Recommended)</option>
            <option value="msdos">MBR (Legacy)</option>
          </select>
        </div>
        <div class="form-group flex-1">
          <label for="filesystem-type-select">Filesystem Type</label>
          <select id="filesystem-type-select" bind:value={partFsType}>
            <option value="ext4">ext4 (Linux default)</option>
            <option value="xfs">xfs (Enterprise)</option>
            <option value="vfat">fat32 (Cross-platform)</option>
          </select>
        </div>
      </div>

      <label class="toggle-checkbox" style="display: flex; align-items: center; gap: 8px; cursor: pointer; font-size: 0.9rem;">
        <input type="checkbox" bind:checked={partUseEntire} disabled />
        <span>Use Entire Disk (100%)</span>
      </label>

      <label class="toggle-checkbox" style="display: flex; align-items: center; gap: 8px; cursor: pointer; font-size: 0.9rem; margin-top: 10px; color: var(--color-danger, #ef4444); font-weight: 600;">
        <input type="checkbox" bind:checked={partConfirm} />
        <span>I understand, create partition</span>
      </label>

      <div class="modal-actions" style="margin-top: 15px;">
        <button class="danger" onclick={executePartition} disabled={!partConfirm || partLoading}>
          {#if partLoading}
            <Loader2 class="spin" size={14} /> Executing...
          {:else}
            Create Partition
          {/if}
        </button>
        <button class="secondary" onclick={() => showPartitionModal = false}>Cancel</button>
      </div>
    </div>
  </div>
{/if}

<!-- Expand Partition Modal -->
{#if showExpandModal}
  <div class="modal-overlay" onclick={() => showExpandModal = false} role="presentation">
    <div class="modal-content glass" style="width: 450px;" onclick={(e) => e.stopPropagation()} role="dialog">
      <div class="modal-header-icon" style="color: var(--accent-amber);">
        <Maximize2 size={32} class="accent-amber-text" />
      </div>
      <h3 style="text-align: center; margin: 0;">Expand Partition: /dev/{expandPartName}</h3>
      
      <div style="margin: 15px 0; font-size: 0.9rem; line-height: 1.4; color: var(--text-secondary);">
        <p>This will automatically resize the partition <strong>/dev/{expandPartName}</strong> and its <strong>{expandFsType}</strong> filesystem to occupy all available unallocated space on the disk.</p>
        <p style="margin-top: 8px; font-weight: 500; color: var(--color-warning, #f59e0b);">Warning: While this operation is generally safe and runs online, it is always recommended to have a backup of critical data before modifying partition tables.</p>
      </div>

      <div class="modal-actions">
        <button class="primary" onclick={executeExpand} disabled={expandLoading}>
          {#if expandLoading}
            <Loader2 class="spin" size={14} /> Executing...
          {:else}
            Expand Partition
          {/if}
        </button>
        <button class="secondary" onclick={() => showExpandModal = false}>Cancel</button>
      </div>
    </div>
  </div>
{/if}

<!-- FSCK Output Log Modal -->
{#if showOutputModal}
  <div class="modal-overlay">
    <div class="modal-content glass" style="width: 550px;" role="dialog">
      <div class="modal-header" style="display: flex; justify-content: space-between; align-items: center; border-bottom: 1px solid var(--border-color); padding-bottom: 10px;">
        <h3 style="margin: 0; display: flex; align-items: center; gap: 8px;">
          {#if outputLoading}
            <Loader2 size={16} class="spin" />
          {:else}
            <CheckCircle2 size={16} style="color: var(--color-success, #10b981);" />
          {/if}
          {outputTitle}
        </h3>
        {#if !outputLoading}
          <button class="icon-btn-compact" onclick={() => showOutputModal = false} style="padding: 4px; border: none; background: transparent; cursor: pointer;">
            <X size={16} />
          </button>
        {/if}
      </div>

      <pre class="console-log mono-val" style="background: #000; color: #fff; padding: 14px; border-radius: var(--radius-sm); max-height: 250px; overflow-y: auto; font-size: 0.82rem; white-space: pre-wrap; word-break: break-all; margin: 10px 0;">{outputText}</pre>

      <div class="modal-actions" style="margin-top: 10px;">
        <button class="secondary" onclick={() => showOutputModal = false} disabled={outputLoading}>Close</button>
      </div>
    </div>
  </div>
{/if}

<style>
  /* Modals */
  .modal-overlay {
    position: fixed;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    background: rgba(0, 0, 0, 0.6);
    backdrop-filter: blur(4px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }

  .modal-content {
    width: 400px;
    padding: 24px;
    border-radius: var(--radius-md, 8px);
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
  }

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
    padding: 8px 12px;
    border-radius: var(--radius-sm, 4px);
    border: 1px solid var(--border-color);
    background: var(--bg-input, rgba(255,255,255,0.03));
    color: var(--text-primary, white);
    font-size: 0.9rem;
    outline: none;
    transition: border-color 0.15s ease;
    width: 100%;
    box-sizing: border-box;
  }

  .form-group input:focus,
  .form-group select:focus {
    border-color: var(--accent-amber);
  }

  .form-row {
    display: flex;
    gap: 12px;
  }

  .flex-1 {
    flex: 1;
  }

  .disk-layout {
    display: flex;
    flex-direction: column;
    gap: 28px;
  }

  .disk-section {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .section-title {
    font-size: 1.1rem;
    font-weight: 600;
    margin: 0;
    color: var(--text-primary);
  }

  /* Usage cards grid */
  .usage-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: 16px;
  }

  .usage-card {
    padding: 16px;
    border-radius: var(--radius-md);
    display: flex;
    flex-direction: column;
    gap: 8px;
    border: 1px solid var(--border-color);
  }

  .usage-card-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .fs-mount {
    font-size: 0.95rem;
    color: var(--text-primary);
  }

  .fs-path {
    font-size: 0.76rem;
    word-break: break-all;
  }

  .progress-container {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-top: 6px;
  }

  .progress-label {
    display: flex;
    justify-content: space-between;
    font-size: 0.8rem;
    font-weight: 500;
  }

  .progress-bar-bg {
    width: 100%;
    height: 8px;
    background: rgba(255, 255, 255, 0.05);
    border-radius: 4px;
    overflow: hidden;
  }

  .progress-bar {
    height: 100%;
    border-radius: 4px;
  }

  /* Disks List Cards */
  .disks-list {
    display: flex;
    flex-direction: column;
    gap: 20px;
  }

  .disk-card {
    padding: 20px;
    border-radius: var(--radius-lg, 12px);
    border: 1px solid var(--border-color);
    display: flex;
    flex-direction: column;
    gap: 16px;
    background: rgba(18, 22, 28, 0.4);
    box-shadow: 0 4px 20px rgba(0, 0, 0, 0.2);
  }

  .disk-card-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    border-bottom: 1px solid rgba(255, 255, 255, 0.04);
    padding-bottom: 12px;
  }

  .disk-info-left {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .disk-icon-box {
    width: 40px;
    height: 40px;
    border-radius: var(--radius-md, 8px);
    background: rgba(245, 158, 11, 0.08);
    display: flex;
    align-items: center;
    justify-content: center;
    border: 1px solid rgba(245, 158, 11, 0.15);
  }

  .disk-name {
    font-size: 1.05rem;
    color: white;
  }

  .disk-model {
    font-size: 0.85rem;
    color: var(--text-secondary);
  }

  .disk-info-right {
    display: flex;
    align-items: center;
    gap: 16px;
  }

  .disk-size {
    font-size: 1.1rem;
    font-weight: 600;
    color: white;
  }

  /* Partition Bar */
  .partition-bar {
    display: flex;
    height: 26px;
    width: 100%;
    background: rgba(255, 255, 255, 0.02);
    border-radius: var(--radius-sm, 6px);
    border: 1px solid var(--border-color);
    overflow: hidden;
    gap: 2px;
  }

  .partition-segment {
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 0.72rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    transition: filter 0.15s ease, transform 0.1s ease;
    cursor: default;
    box-sizing: border-box;
  }

  .partition-segment:hover {
    filter: brightness(1.2);
  }

  .partition-segment.partition {
    background: rgba(59, 130, 246, 0.16);
    border-right: 1px solid rgba(59, 130, 246, 0.3);
    color: #93c5fd;
  }

  .partition-segment.free {
    background: repeating-linear-gradient(45deg, rgba(255, 255, 255, 0.02), rgba(255, 255, 255, 0.02) 10px, transparent 10px, transparent 20px);
    border: 1px dashed rgba(255, 255, 255, 0.15);
    color: var(--text-muted);
  }

  .segment-label {
    padding: 0 8px;
    font-weight: 500;
    pointer-events: none;
  }

  .segment-size {
    opacity: 0.7;
    font-variant-numeric: tabular-nums;
  }

  /* Partition Table */
  .partition-table-wrapper {
    overflow-x: auto;
    border-radius: var(--radius-md, 8px);
  }

  .partition-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.85rem;
    text-align: left;
  }

  .partition-table th, 
  .partition-table td {
    padding: 10px 14px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.04);
  }

  .partition-table th {
    color: var(--text-muted);
    font-size: 0.75rem;
    text-transform: uppercase;
    font-weight: 600;
    letter-spacing: 0.05em;
  }

  .part-row:hover {
    background: rgba(255, 255, 255, 0.02);
  }

  .sub-part-row {
    background: rgba(0, 0, 0, 0.1);
  }

  .free-space-row {
    background: rgba(245, 158, 11, 0.02);
  }

  .free-space-row td {
    border-bottom: 1px dashed rgba(245, 158, 11, 0.1);
  }

  /* Compact buttons & actions */
  .btn-compact {
    padding: 6px 12px;
    font-size: 0.8rem;
    border-radius: var(--radius-sm, 6px);
    font-weight: 500;
    cursor: pointer;
    border: none;
    transition: all 0.1s ease;
  }

  .btn-compact.primary {
    background: var(--accent-amber);
    color: #000;
  }

  .btn-compact.primary:hover {
    filter: brightness(1.1);
  }

  .btn-action {
    background: transparent;
    border: 1px solid var(--border-color);
    color: var(--text-secondary);
    padding: 4px 10px;
    font-size: 0.75rem;
    border-radius: 4px;
    cursor: pointer;
    transition: all 0.1s ease;
    margin-left: 6px;
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }

  .btn-action:hover {
    background: var(--bg-hover);
    color: white;
    border-color: rgba(255, 255, 255, 0.2);
  }

  .btn-action.danger-text {
    color: var(--color-danger, #ef4444);
    border-color: rgba(239, 68, 68, 0.2);
  }

  .btn-action.danger-text:hover {
    background: rgba(239, 68, 68, 0.1);
    border-color: rgba(239, 68, 68, 0.3);
  }

  .btn-action.success-text {
    color: var(--color-success, #10b981);
    border-color: rgba(16, 185, 129, 0.2);
  }

  .btn-action.success-text:hover {
    background: rgba(16, 185, 129, 0.1);
    border-color: rgba(16, 185, 129, 0.3);
  }

  /* UUID Cell and Badge */
  .uuid-cell {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }

  .part-label-badge {
    padding: 2px 6px;
    border-radius: 4px;
    font-size: 0.72rem;
    font-weight: 500;
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: white;
  }

  .uuid-badge {
    padding: 2px 6px;
    border-radius: 4px;
    font-size: 0.72rem;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.06);
    color: var(--text-secondary);
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }

  .copy-btn {
    background: transparent;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    padding: 2px;
    display: inline-flex;
    align-items: center;
    border-radius: 3px;
  }

  .copy-btn:hover {
    color: white;
    background: rgba(255, 255, 255, 0.05);
  }

  /* Alerts */
  .alert-box {
    padding: 12px;
    border-radius: var(--radius-sm);
    font-size: 0.85rem;
  }

  .danger-box {
    background: rgba(239, 68, 68, 0.1);
    border: 1px solid rgba(239, 68, 68, 0.2);
    color: var(--color-danger, #ef4444);
  }

  /* Console logs */
  .console-log {
    border: 1px solid rgba(255, 255, 255, 0.1);
  }

  /* Sudo dialog icon */
  .modal-header-icon {
    display: flex;
    justify-content: center;
    margin-bottom: 8px;
  }
</style>
