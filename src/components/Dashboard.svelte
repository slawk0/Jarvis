<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { Cpu, HardDrive, Info, Activity, ArrowDown, ArrowUp, Globe, Shield, Gauge, Layers } from 'lucide-svelte';
  import { formatCompact, getCountryName, type CountryTraffic } from '$lib/geo/countryUtils';
  import { checkResourceAlerts } from '$lib/alerts/monitor';
  import type { ExtendedServerStats, ProfileExtras } from '$lib/admin/types';
  import { DEFAULT_PROFILE_EXTRAS } from '$lib/admin/types';
  import { get } from 'svelte/store';
  import { LL } from '$lib/i18n/i18n-svelte';
  import { formatInvokeError } from '$lib/i18n/backendErrors';

  let {
    initialStats,
    profileId = '',
    profileLabel = '',
  } = $props();

  const displayProfileLabel = $derived(profileLabel || get(LL).shell.defaultServerLabel());

  let stats = $state(initialStats);
  let extended = $state<ExtendedServerStats | null>(null);
  let alertConfig = $state<ProfileExtras['alert_thresholds']>({ ...DEFAULT_PROFILE_EXTRAS.alert_thresholds });
  let errorMsg = $state('');
  let cpuHistory = $state<number[]>([]);
  let ramHistory = $state<number[]>([]);
  let networkHistory = $state<{rx: number, tx: number}[]>([]);
  
  let prevRx = $state(initialStats.network_rx);
  let prevTx = $state(initialStats.network_tx);
  let downSpeed = $state('0 B/s');
  let upSpeed = $state('0 B/s');

  let proxyStats = $state({
    configured: false,
    loading: false,
    totalRequests: 0,
    totalBlocked: 0,
    topCountries: [] as CountryTraffic[],
    error: ''
  });

  let intervalId: any;
  let extendedIntervalId: any;

  async function loadAlertConfig() {
    if (!profileId) return;
    try {
      const extras: ProfileExtras = await invoke('get_profile_extras', { profileId });
      alertConfig = extras.alert_thresholds;
    } catch {
      /* defaults */
    }
  }

  async function loadExtendedStats() {
    try {
      extended = await invoke<ExtendedServerStats>('get_extended_server_stats');
    } catch {
      extended = null;
    }
  }

  async function loadProxyStats() {
    proxyStats.loading = true;
    proxyStats.error = '';
    try {
      const config: any = await invoke('get_pangolin_config');
      if (!config?.has_api_key || !config?.org_id) {
        proxyStats = { ...proxyStats, configured: false, loading: false };
        return;
      }

      const end = new Date();
      const start = new Date();
      start.setDate(start.getDate() - 7);

      const res: any = await invoke('pangolin_api_request', {
        method: 'GET',
        path: `/v1/org/${config.org_id}/logs/analytics`,
        queryParams: {
          timeStart: start.toISOString(),
          timeEnd: end.toISOString()
        },
        body: null
      });

      const data = res?.data || {};
      const countries = (data.requestsPerCountry || []) as CountryTraffic[];
      proxyStats = {
        configured: true,
        loading: false,
        totalRequests: data.totalRequests || 0,
        totalBlocked: data.totalBlocked || 0,
        topCountries: [...countries].sort((a, b) => (b.count || 0) - (a.count || 0)).slice(0, 3),
        error: ''
      };
    } catch (err: any) {
      proxyStats = {
        ...proxyStats,
        configured: false,
        loading: false,
        error: err.toString()
      };
    }
  }

  const proxyAllowed = $derived(Math.max(proxyStats.totalRequests - proxyStats.totalBlocked, 0));
  const proxyBlockRate = $derived(
    proxyStats.totalRequests > 0
      ? ((proxyStats.totalBlocked / proxyStats.totalRequests) * 100).toFixed(1)
      : '0'
  );

  function formatBytes(bytes: number, decimals = 1) {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const dm = decimals < 0 ? 0 : decimals;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(dm)) + ' ' + sizes[i];
  }

  async function updateStats() {
    try {
      const newStats: any = await invoke('get_server_stats');
      stats = newStats;
      errorMsg = '';

      await checkResourceAlerts(displayProfileLabel, newStats, alertConfig);

      // Network throughput calculation
      const deltaRx = newStats.network_rx >= prevRx ? newStats.network_rx - prevRx : 0;
      const deltaTx = newStats.network_tx >= prevTx ? newStats.network_tx - prevTx : 0;
      
      // Our poller runs every 2 seconds, so we divide by 2 for the per-second value
      downSpeed = formatBytes(deltaRx / 2) + '/s';
      upSpeed = formatBytes(deltaTx / 2) + '/s';

      prevRx = newStats.network_rx;
      prevTx = newStats.network_tx;

      // Update history
      cpuHistory = [...cpuHistory.slice(-19), newStats.cpu_usage];
      const ramUsagePct = (newStats.ram_used / newStats.ram_total) * 100;
      ramHistory = [...ramHistory.slice(-19), ramUsagePct];
      
    } catch (err: unknown) {
      errorMsg = get(LL).alerts.statsLoadFailed({ error: formatInvokeError(err) });
    }
  }

  // Generating SVG path for line charts
  function getSvgPath(history: number[]) {
    if (history.length < 2) return '';
    const width = 500;
    const height = 120;
    const step = width / 20;
    
    return history.map((val, idx) => {
      const x = idx * step;
      const y = height - (val / 100) * height * 0.8 - 10; // zostawiamy margines
      return `${idx === 0 ? 'M' : 'L'} ${x} ${y}`;
    }).join(' ');
  }

  function getSvgAreaPath(history: number[]) {
    if (history.length < 2) return '';
    const width = 500;
    const height = 120;
    const step = width / 20;
    const linePath = getSvgPath(history);
    const lastX = (history.length - 1) * step;
    
    return `${linePath} L ${lastX} ${height} L 0 ${height} Z`;
  }

  onMount(() => {
    cpuHistory = Array(15).fill(stats.cpu_usage);
    const initialRamPct = (stats.ram_used / stats.ram_total) * 100;
    ramHistory = Array(15).fill(initialRamPct);

    loadProxyStats();
    loadAlertConfig();
    loadExtendedStats();

    intervalId = setInterval(updateStats, 2000);
    extendedIntervalId = setInterval(loadExtendedStats, 10000);
  });

  $effect(() => {
    if (profileId) loadAlertConfig();
  });

  onDestroy(() => {
    clearInterval(intervalId);
    clearInterval(extendedIntervalId);
  });

  async function saveAlertConfig() {
    if (!profileId) return;
    try {
      const extras: ProfileExtras = await invoke('get_profile_extras', { profileId });
      extras.alert_thresholds = alertConfig;
      await invoke('save_profile_extras', { profileId, extras });
    } catch (err: unknown) {
      errorMsg = get(LL).alerts.saveFailed({ error: formatInvokeError(err) });
    }
  }
</script>

<div class="dashboard manager-shell scrollable fade-in">
  <header class="manager-header">
    <h1 class="page-title">{$LL.dashboard.title()}</h1>
    {#if errorMsg}
      <div class="error-badge">{errorMsg}</div>
    {/if}
  </header>

  <!-- Sekcja informacji systemowych -->
  <section class="system-info-panel glass">
    <div class="info-item">
      <Info class="info-icon animate-pulse" size={20} />
      <div>
        <span class="info-label">{$LL.dashboard.os()}</span>
        <span class="info-val">{stats.os}</span>
      </div>
    </div>
    <div class="info-item">
      <Activity class="info-icon" size={20} />
      <div>
        <span class="info-label">{$LL.dashboard.uptime()}</span>
        <span class="info-val mono-val">{stats.uptime}</span>
      </div>
    </div>
    <div class="info-item">
      <Cpu class="info-icon" size={20} />
      <div>
        <span class="info-label">{$LL.dashboard.hostname()}</span>
        <span class="info-val">{stats.hostname}</span>
      </div>
    </div>
  </section>

  <!-- Sekcja Ring-Gauges (CPU, RAM, DYSK) -->
  <section class="metrics-grid">
    <!-- CPU -->
    <div class="metric-card glass">
      <div class="card-header">
        <h3>{$LL.dashboard.cpu()}</h3>
        <Cpu size={18} class="accent-amber-text" />
      </div>
      <div class="gauge-container">
        <svg class="ring-gauge" viewBox="0 0 100 100">
          <circle class="gauge-bg" cx="50" cy="50" r="40" />
          <circle 
            class="gauge-fill cpu" 
            cx="50" cy="50" r="40" 
            style="stroke-dasharray: 251.2; stroke-dashoffset: {251.2 - (251.2 * stats.cpu_usage) / 100}"
          />
        </svg>
        <div class="gauge-value">
          <span class="val mono-val">{Math.round(stats.cpu_usage)}%</span>
        </div>
      </div>
    </div>

    <!-- RAM -->
    <div class="metric-card glass">
      <div class="card-header">
        <h3>{$LL.dashboard.ram()}</h3>
        <Activity size={18} class="accent-rust-text" />
      </div>
      <div class="gauge-container">
        <svg class="ring-gauge" viewBox="0 0 100 100">
          <circle class="gauge-bg" cx="50" cy="50" r="40" />
          <circle 
            class="gauge-fill ram" 
            cx="50" cy="50" r="40" 
            style="stroke-dasharray: 251.2; stroke-dashoffset: {251.2 - (251.2 * (stats.ram_used / stats.ram_total * 100)) / 100}"
          />
        </svg>
        <div class="gauge-value">
          <span class="val mono-val">{Math.round((stats.ram_used / stats.ram_total) * 100)}%</span>
          <span class="desc mono-val">{stats.ram_used} MB / {stats.ram_total} MB</span>
        </div>
      </div>
    </div>

    <!-- DYSK -->
    <div class="metric-card glass">
      <div class="card-header">
        <h3>{$LL.dashboard.disk()}</h3>
        <HardDrive size={18} class="accent-green-text" />
      </div>
      <div class="gauge-container">
        <svg class="ring-gauge" viewBox="0 0 100 100">
          <circle class="gauge-bg" cx="50" cy="50" r="40" />
          <circle 
            class="gauge-fill disk" 
            cx="50" cy="50" r="40" 
            style="stroke-dasharray: 251.2; stroke-dashoffset: {251.2 - (251.2 * (stats.disk_used / stats.disk_total * 100)) / 100}"
          />
        </svg>
        <div class="gauge-value">
          <span class="val mono-val">{Math.round((stats.disk_used / stats.disk_total) * 100)}%</span>
          <span class="desc mono-val">{(stats.disk_used / 1024).toFixed(1)} GB / {(stats.disk_total / 1024).toFixed(1)} GB</span>
        </div>
      </div>
    </div>
  </section>

  <!-- History charts and network -->
  <section class="charts-grid">
    <div class="chart-card glass">
      <div class="chart-header">
        <h3>{$LL.dashboard.chartTitle()}</h3>
        <div class="legend">
          <span class="legend-item"><span class="color-dot cpu"></span>{$LL.dashboard.cpu()}</span>
          <span class="legend-item"><span class="color-dot ram"></span>{$LL.dashboard.ram()}</span>
        </div>
      </div>
      <div class="chart-container">
        <svg viewBox="0 0 500 120" preserveAspectRatio="none" class="chart-svg">
          <defs>
            <linearGradient id="cpu-grad" x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stop-color="var(--accent-amber)" stop-opacity="0.2"/>
              <stop offset="100%" stop-color="var(--accent-amber)" stop-opacity="0.0"/>
            </linearGradient>
            <linearGradient id="ram-grad" x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stop-color="var(--accent-rust)" stop-opacity="0.1"/>
              <stop offset="100%" stop-color="var(--accent-rust)" stop-opacity="0.0"/>
            </linearGradient>
          </defs>
          
          <!-- Siatka oscyloskopowa -->
          <line x1="0" y1="20" x2="500" y2="20" stroke="rgba(245, 158, 11, 0.05)" stroke-width="0.5" />
          <line x1="0" y1="40" x2="500" y2="40" stroke="rgba(245, 158, 11, 0.05)" stroke-width="0.5" />
          <line x1="0" y1="60" x2="500" y2="60" stroke="rgba(245, 158, 11, 0.05)" stroke-width="0.5" />
          <line x1="0" y1="80" x2="500" y2="80" stroke="rgba(245, 158, 11, 0.05)" stroke-width="0.5" />
          <line x1="0" y1="100" x2="500" y2="100" stroke="rgba(245, 158, 11, 0.05)" stroke-width="0.5" />
          
          <line x1="100" y1="0" x2="100" y2="120" stroke="rgba(245, 158, 11, 0.05)" stroke-width="0.5" />
          <line x1="200" y1="0" x2="200" y2="120" stroke="rgba(245, 158, 11, 0.05)" stroke-width="0.5" />
          <line x1="300" y1="0" x2="300" y2="120" stroke="rgba(245, 158, 11, 0.05)" stroke-width="0.5" />
          <line x1="400" y1="0" x2="400" y2="120" stroke="rgba(245, 158, 11, 0.05)" stroke-width="0.5" />

          <!-- Obszar CPU -->
          {#if cpuHistory.length > 1}
            <path d={getSvgAreaPath(cpuHistory)} fill="url(#cpu-grad)" />
            <path d={getSvgPath(cpuHistory)} fill="none" stroke="var(--accent-amber)" stroke-width="1.5" stroke-linecap="round" />
          {/if}

          <!-- Obszar RAM -->
          {#if ramHistory.length > 1}
            <path d={getSvgAreaPath(ramHistory)} fill="url(#ram-grad)" />
            <path d={getSvgPath(ramHistory)} fill="none" stroke="var(--accent-rust)" stroke-width="1.5" stroke-dasharray="3,3" stroke-linecap="round" />
          {/if}
        </svg>
      </div>
    </div>

    <!-- Network -->
    <div class="network-card glass">
      <h3>{$LL.dashboard.networkSpeed()}</h3>
      <div class="net-stats">
        <div class="net-dir">
          <ArrowDown size={28} class="net-icon down" />
          <div class="net-info">
            <span class="label">{$LL.dashboard.download()}</span>
            <span class="value down-val mono-val">{downSpeed}</span>
          </div>
        </div>
        <div class="net-dir">
          <ArrowUp size={28} class="net-icon up" />
          <div class="net-info">
            <span class="label">{$LL.dashboard.upload()}</span>
            <span class="value up-val mono-val">{upSpeed}</span>
          </div>
        </div>
      </div>
    </div>
  </section>

  {#if extended}
    <section class="extended-grid">
      <div class="ext-card glass">
        <div class="ext-header"><Gauge size={16} /> {$LL.dashboard.loadAverage()}</div>
        <div class="load-vals mono-val">
          <span>{extended.load_1.toFixed(2)}</span>
          <span class="sep">/</span>
          <span>{extended.load_5.toFixed(2)}</span>
          <span class="sep">/</span>
          <span>{extended.load_15.toFixed(2)}</span>
        </div>
        <span class="ext-label">{$LL.dashboard.loadPeriods()}</span>
      </div>
      <div class="ext-card glass">
        <div class="ext-header"><Layers size={16} /> {$LL.dashboard.swap()}</div>
        <div class="ext-val mono-val">
          {extended.swap_used_mb} / {extended.swap_total_mb} MB
        </div>
        <span class="ext-label">
          {$LL.dashboard.swapUsage({
            pct: String(
              extended.swap_total_mb > 0
                ? Math.round((extended.swap_used_mb / extended.swap_total_mb) * 100)
                : 0
            ),
          })}
        </span>
      </div>
    </section>

    {#if extended.disk_mounts.length > 0}
      <section class="mounts-panel glass">
        <h3>{$LL.dashboard.diskPartitions()}</h3>
        <div class="mounts-table">
          <div class="mount-row head">
            <span>{$LL.dashboard.mount()}</span><span>{$LL.dashboard.usage()}</span><span>{$LL.dashboard.inode()}</span>
          </div>
          {#each extended.disk_mounts as m}
            <div class="mount-row">
              <span class="mono-val">{m.mount}</span>
              <span class="mono-val {m.use_pct >= 85 ? 'warn' : ''}">{m.use_pct}% ({Math.round(m.used_mb / 1024)}G/{Math.round(m.total_mb / 1024)}G)</span>
              <span class="mono-val {m.inode_use_pct >= 85 ? 'warn' : ''}">{m.inode_use_pct}%</span>
            </div>
          {/each}
        </div>
      </section>
    {/if}

    {#if extended.top_processes.length > 0}
      <section class="procs-panel glass">
        <h3>{$LL.dashboard.topProcesses()}</h3>
        <div class="procs-table">
          <div class="proc-row head">
            <span>{$LL.dashboard.pid()}</span><span>{$LL.dashboard.user()}</span><span>{$LL.dashboard.cpu()}</span><span>{$LL.dashboard.ram()}</span><span>{$LL.dashboard.command()}</span>
          </div>
          {#each extended.top_processes as p}
            <div class="proc-row">
              <span class="mono-val">{p.pid}</span>
              <span>{p.user}</span>
              <span class="mono-val">{p.cpu}%</span>
              <span class="mono-val">{p.mem}%</span>
              <span class="cmd" title={p.command}>{p.command}</span>
            </div>
          {/each}
        </div>
      </section>
    {/if}
  {/if}

  <section class="alerts-panel glass">
    <div class="alerts-header">
      <h3>{$LL.alerts.panelTitle()}</h3>
      <label class="toggle-row">
        <input type="checkbox" bind:checked={alertConfig.enabled} onchange={saveAlertConfig} />
        {$LL.alerts.enabled()}
      </label>
    </div>
    {#if alertConfig.enabled}
      <div class="alert-thresholds">
        <label>{$LL.alerts.cpuThreshold()}
          <input type="number" min="50" max="100" bind:value={alertConfig.cpu_pct} onchange={saveAlertConfig} />
        </label>
        <label>{$LL.alerts.ramThreshold()}
          <input type="number" min="50" max="100" bind:value={alertConfig.ram_pct} onchange={saveAlertConfig} />
        </label>
        <label>{$LL.alerts.diskThreshold()}
          <input type="number" min="50" max="100" bind:value={alertConfig.disk_pct} onchange={saveAlertConfig} />
        </label>
      </div>
    {/if}
  </section>

  <!-- Pangolin Proxy Stats -->
  <section class="proxy-section glass">
    <div class="proxy-header">
      <h3><Globe size={18} class="accent-amber-text" /> {$LL.dashboard.proxyTitle()}</h3>
      {#if proxyStats.configured}
        <span class="proxy-badge">{$LL.dashboard.proxyConnected()}</span>
      {/if}
    </div>

    {#if proxyStats.loading}
      <p class="proxy-muted">{$LL.dashboard.proxyLoading()}</p>
    {:else if !proxyStats.configured}
      <p class="proxy-muted">
        {$LL.dashboard.proxyNotConfigured()}
      </p>
    {:else}
      <div class="proxy-grid">
        <div class="proxy-stat">
          <span class="proxy-label">{$LL.dashboard.requests()}</span>
          <span class="proxy-val mono-val">{formatCompact(proxyStats.totalRequests)}</span>
        </div>
        <div class="proxy-stat">
          <span class="proxy-label">{$LL.dashboard.allowed()}</span>
          <span class="proxy-val mono-val text-green">{formatCompact(proxyAllowed)}</span>
        </div>
        <div class="proxy-stat">
          <span class="proxy-label">{$LL.dashboard.blocked()}</span>
          <span class="proxy-val mono-val text-red">{formatCompact(proxyStats.totalBlocked)}</span>
        </div>
        <div class="proxy-stat">
          <span class="proxy-label">{$LL.dashboard.blockRate()}</span>
          <span class="proxy-val mono-val">{proxyBlockRate}%</span>
        </div>
      </div>

      {#if proxyStats.topCountries.length > 0}
        <div class="proxy-countries">
          <span class="proxy-label">{$LL.dashboard.topCountries()}</span>
          <div class="proxy-country-list">
            {#each proxyStats.topCountries as country}
              <div class="proxy-country-item">
                <Shield size={14} class="accent-amber-text" />
                <span class="country-code">{country.code}</span>
                <span class="country-name">{getCountryName(country)}</span>
                <span class="country-count mono-val">{formatCompact(country.count || 0)}</span>
              </div>
            {/each}
          </div>
        </div>
      {/if}
    {/if}
  </section>
</div>

<style>
  .dashboard {
    /* uses .manager-shell */
  }

  .error-badge {
    background: var(--accent-red-glow);
    border: 1px solid rgba(239, 68, 68, 0.3);
    padding: 8px 16px;
    border-radius: var(--radius-sm);
    color: #ff8585;
    font-size: 0.85rem;
  }

  /* Info panel */
  .system-info-panel {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 10px;
    padding: 10px 12px;
    border-radius: var(--radius-sm);
  }

  .info-item {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .info-icon {
    color: var(--accent-amber);
    background: rgba(245, 158, 11, 0.08);
    padding: 6px;
    border-radius: var(--radius-sm);
    border: 1px solid rgba(245, 158, 11, 0.15);
  }

  .info-label {
    display: block;
    font-size: 0.65rem;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .info-val {
    display: block;
    font-size: 0.82rem;
    color: var(--text-primary);
    font-weight: 500;
    margin-top: 1px;
  }

  /* Metrics Grid */
  .metrics-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 20px;
  }

  .metric-card {
    border-radius: var(--radius-sm);
    padding: 20px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 16px;
  }

  .card-header {
    width: 100%;
    display: flex;
    justify-content: space-between;
    align-items: center;
    border-bottom: 1px solid var(--border-color);
    padding-bottom: 10px;
  }

  .card-header h3 {
    font-size: 0.9rem;
    color: var(--text-primary);
    font-weight: 500;
  }

  .accent-amber-text { color: var(--accent-amber); }
  .accent-rust-text { color: var(--accent-rust); }
  .accent-green-text { color: var(--accent-green); }

  /* Circular Progress Ring */
  .gauge-container {
    position: relative;
    width: 120px;
    height: 120px;
  }

  .ring-gauge {
    transform: rotate(-90deg);
    width: 120px;
    height: 120px;
  }

  .gauge-bg {
    fill: none;
    stroke: rgba(255, 255, 255, 0.02);
    stroke-width: 7;
  }

  .gauge-fill {
    fill: none;
    stroke-width: 7;
    stroke-linecap: round;
    transition: stroke-dashoffset 0.8s cubic-bezier(0.4, 0, 0.2, 1);
  }

  .gauge-fill.cpu { stroke: var(--accent-amber); filter: drop-shadow(0 0 4px var(--accent-amber-glow)); }
  .gauge-fill.ram { stroke: var(--accent-rust); filter: drop-shadow(0 0 4px var(--accent-rust-glow)); }
  .gauge-fill.disk { stroke: var(--accent-green); filter: drop-shadow(0 0 4px var(--accent-green-glow)); }

  .gauge-value {
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
    padding: 10px;
  }

  .gauge-value .val {
    font-size: 1.4rem;
    font-family: var(--font-mono);
    font-weight: 700;
    color: white;
  }

  .gauge-value .desc {
    font-size: 0.65rem;
    font-family: var(--font-mono);
    color: var(--text-secondary);
    margin-top: 4px;
    max-width: 100px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* Charts and network */
  .charts-grid {
    display: grid;
    grid-template-columns: 2fr 1fr;
    gap: 20px;
  }

  .chart-card {
    border-radius: var(--radius-sm);
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .chart-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .chart-header h3 {
    font-size: 0.9rem;
    font-weight: 500;
  }

  .legend {
    display: flex;
    gap: 12px;
    font-size: 0.75rem;
  }

  .legend-item {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--text-secondary);
  }

  .color-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
  }
  .color-dot.cpu { background-color: var(--accent-amber); }
  .color-dot.ram { background-color: var(--accent-rust); }

  .chart-container {
    height: 120px;
    width: 100%;
    background: rgba(0, 0, 0, 0.2);
    border: 1px solid var(--border-color);
    border-radius: var(--radius-sm);
    padding: 8px;
    overflow: hidden;
  }

  .chart-svg {
    width: 100%;
    height: 100%;
    overflow: visible;
  }

  /* Network Card */
  .network-card {
    border-radius: var(--radius-sm);
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .network-card h3 {
    font-size: 0.9rem;
    font-weight: 500;
  }

  .net-stats {
    display: flex;
    flex-direction: column;
    gap: 16px;
    justify-content: center;
    height: 100%;
  }

  .net-dir {
    display: flex;
    align-items: center;
    gap: 14px;
  }

  .net-icon {
    padding: 6px;
    border-radius: var(--radius-sm);
  }

  .net-icon.down {
    color: var(--accent-amber);
    background: rgba(245, 158, 11, 0.08);
    border: 1px solid rgba(245, 158, 11, 0.15);
  }

  .net-icon.up {
    color: var(--accent-rust);
    background: rgba(194, 65, 12, 0.08);
    border: 1px solid rgba(194, 65, 12, 0.15);
  }

  .net-info {
    display: flex;
    flex-direction: column;
  }

  .net-info .label {
    font-size: 0.65rem;
    color: var(--text-muted);
    font-weight: 600;
    letter-spacing: 0.05em;
  }

  .net-info .value {
    font-size: 1.15rem;
    font-family: var(--font-mono);
    font-weight: 700;
    color: white;
    margin-top: 2px;
  }

  /* Pangolin Proxy section */
  .proxy-section {
    border-radius: var(--radius-sm);
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .proxy-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .proxy-header h3 {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 0.9rem;
    font-weight: 500;
    margin: 0;
  }

  .proxy-badge {
    font-size: 0.7rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--accent-green);
    background: rgba(34, 197, 94, 0.1);
    border: 1px solid rgba(34, 197, 94, 0.25);
    padding: 4px 8px;
    border-radius: var(--radius-sm);
  }

  .proxy-muted {
    color: var(--text-muted);
    font-size: 0.85rem;
    margin: 0;
  }

  .proxy-grid {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 12px;
  }

  @media (max-width: 900px) {
    .proxy-grid {
      grid-template-columns: repeat(2, 1fr);
    }
  }

  .proxy-stat {
    background: rgba(0, 0, 0, 0.2);
    border: 1px solid var(--border-color);
    border-radius: var(--radius-sm);
    padding: 12px 14px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .proxy-label {
    font-size: 0.65rem;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .proxy-val {
    font-size: 1.2rem;
    font-weight: 700;
    color: white;
  }

  .text-green { color: var(--accent-green); }
  .text-red { color: var(--accent-red); }

  .proxy-countries {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .proxy-country-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .proxy-country-item {
    display: grid;
    grid-template-columns: auto 28px 1fr auto;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    background: rgba(0, 0, 0, 0.15);
    border-radius: var(--radius-sm);
    font-size: 0.82rem;
  }

  .country-code {
    font-family: var(--font-mono);
    font-weight: 700;
    color: var(--accent-amber);
    font-size: 0.75rem;
  }

  .country-name {
    color: var(--text-secondary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .country-count {
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }

  .extended-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
  }

  .ext-card {
    padding: 14px;
    border-radius: var(--radius-sm);
  }

  .ext-header {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 0.85rem;
    color: var(--text-secondary);
    margin-bottom: 8px;
  }

  .load-vals, .ext-val {
    font-size: 1.3rem;
    font-weight: 700;
    color: white;
  }

  .load-vals .sep { color: var(--text-muted); margin: 0 4px; }

  .ext-label {
    font-size: 0.7rem;
    color: var(--text-muted);
    margin-top: 4px;
    display: block;
  }

  .mounts-panel, .procs-panel {
    padding: 14px;
    border-radius: var(--radius-sm);
  }

  .mounts-panel h3, .procs-panel h3 {
    font-size: 0.9rem;
    color: white;
    margin-bottom: 10px;
  }

  .mounts-table, .procs-table {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 0.78rem;
  }

  .mount-row, .proc-row {
    display: grid;
    grid-template-columns: 1fr 1.2fr 0.5fr;
    gap: 8px;
    padding: 6px 8px;
    border-radius: var(--radius-sm);
    background: rgba(0, 0, 0, 0.15);
  }

  .proc-row {
    grid-template-columns: 60px 80px 50px 50px 1fr;
  }

  .mount-row.head, .proc-row.head {
    background: transparent;
    color: var(--text-muted);
    font-size: 0.7rem;
    text-transform: uppercase;
  }

  .warn { color: var(--accent-red); }

  .cmd {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-secondary);
  }

  .alerts-panel {
    padding: 14px;
    border-radius: var(--radius-sm);
  }

  .alerts-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 10px;
  }

  .alerts-header h3 {
    font-size: 0.9rem;
    color: white;
  }

  .toggle-row {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 0.8rem;
    color: var(--text-secondary);
  }

  .alert-thresholds {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 10px;
  }

  .alert-thresholds label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 0.75rem;
    color: var(--text-muted);
  }

  .alert-thresholds input {
    padding: 6px 10px;
    font-size: 0.85rem;
  }
</style>
