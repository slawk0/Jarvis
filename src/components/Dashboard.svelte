<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { Cpu, HardDrive, Info, Activity, ArrowDown, ArrowUp } from 'lucide-svelte';

  // Props in Svelte 5
  let { initialStats } = $props();

  let stats = $state(initialStats);
  let errorMsg = $state('');
  let cpuHistory = $state<number[]>([]);
  let ramHistory = $state<number[]>([]);
  let networkHistory = $state<{rx: number, tx: number}[]>([]);
  
  let prevRx = $state(initialStats.network_rx);
  let prevTx = $state(initialStats.network_tx);
  let downSpeed = $state('0 B/s');
  let upSpeed = $state('0 B/s');

  let intervalId: any;

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

      // Obliczanie przepustowości sieci
      const deltaRx = newStats.network_rx >= prevRx ? newStats.network_rx - prevRx : 0;
      const deltaTx = newStats.network_tx >= prevTx ? newStats.network_tx - prevTx : 0;
      
      // Nasz poller działa co 2 sekundy, więc dzielimy przez 2 dla wartości na sekundę
      downSpeed = formatBytes(deltaRx / 2) + '/s';
      upSpeed = formatBytes(deltaTx / 2) + '/s';

      prevRx = newStats.network_rx;
      prevTx = newStats.network_tx;

      // Aktualizuj historię
      cpuHistory = [...cpuHistory.slice(-19), newStats.cpu_usage];
      const ramUsagePct = (newStats.ram_used / newStats.ram_total) * 100;
      ramHistory = [...ramHistory.slice(-19), ramUsagePct];
      
    } catch (err: any) {
      errorMsg = 'Nie można pobrać statystyk: ' + err.toString();
    }
  }

  // Generowanie ścieżki SVG dla wykresów liniowych
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
    // Inicjalizacja historii
    cpuHistory = Array(15).fill(stats.cpu_usage);
    const initialRamPct = (stats.ram_used / stats.ram_total) * 100;
    ramHistory = Array(15).fill(initialRamPct);

    // Odpytywanie co 2 sekundy
    intervalId = setInterval(updateStats, 2000);
  });

  onDestroy(() => {
    clearInterval(intervalId);
  });
</script>

<div class="dashboard fade-in">
  <header class="dash-header">
    <div class="title-area">
      <h1>Panel Główny</h1>
      <p class="subtitle">Wgląd w parametry serwera w czasie rzeczywistym</p>
    </div>
    {#if errorMsg}
      <div class="error-badge">{errorMsg}</div>
    {/if}
  </header>

  <!-- Sekcja informacji systemowych -->
  <section class="system-info-panel glass">
    <div class="info-item">
      <Info class="info-icon" size={20} />
      <div>
        <span class="info-label">System Operacyjny</span>
        <span class="info-val">{stats.os}</span>
      </div>
    </div>
    <div class="info-item">
      <Activity class="info-icon" size={20} />
      <div>
        <span class="info-label">Czas Uruchomienia (Uptime)</span>
        <span class="info-val">{stats.uptime}</span>
      </div>
    </div>
    <div class="info-item">
      <Cpu class="info-icon" size={20} />
      <div>
        <span class="info-label">Nazwa Hosta</span>
        <span class="info-val">{stats.hostname}</span>
      </div>
    </div>
  </section>

  <!-- Sekcja Ring-Gauges (CPU, RAM, DYSK) -->
  <section class="metrics-grid">
    <!-- CPU -->
    <div class="metric-card glass">
      <div class="card-header">
        <h3>Procesor (CPU)</h3>
        <Cpu size={18} class="accent-blue-text" />
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
          <span class="val">{Math.round(stats.cpu_usage)}%</span>
        </div>
      </div>
    </div>

    <!-- RAM -->
    <div class="metric-card glass">
      <div class="card-header">
        <h3>Pamięć operacyjna</h3>
        <Activity size={18} class="accent-purple-text" />
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
          <span class="val">{Math.round((stats.ram_used / stats.ram_total) * 100)}%</span>
          <span class="desc">{stats.ram_used} MB / {stats.ram_total} MB</span>
        </div>
      </div>
    </div>

    <!-- DYSK -->
    <div class="metric-card glass">
      <div class="card-header">
        <h3>Przestrzeń dyskowa</h3>
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
          <span class="val">{Math.round((stats.disk_used / stats.disk_total) * 100)}%</span>
          <span class="desc">{(stats.disk_used / 1024).toFixed(1)} GB / {(stats.disk_total / 1024).toFixed(1)} GB</span>
        </div>
      </div>
    </div>
  </section>

  <!-- Wykresy historii oraz sieć -->
  <section class="charts-grid">
    <div class="chart-card glass">
      <div class="chart-header">
        <h3>Historia obciążenia CPU & RAM</h3>
        <div class="legend">
          <span class="legend-item"><span class="color-dot cpu"></span>CPU</span>
          <span class="legend-item"><span class="color-dot ram"></span>RAM</span>
        </div>
      </div>
      <div class="chart-container">
        <svg viewBox="0 0 500 120" preserveAspectRatio="none" class="chart-svg">
          <defs>
            <linearGradient id="cpu-grad" x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stop-color="var(--accent-blue)" stop-opacity="0.3"/>
              <stop offset="100%" stop-color="var(--accent-blue)" stop-opacity="0.0"/>
            </linearGradient>
            <linearGradient id="ram-grad" x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stop-color="var(--accent-purple)" stop-opacity="0.3"/>
              <stop offset="100%" stop-color="var(--accent-purple)" stop-opacity="0.0"/>
            </linearGradient>
          </defs>
          
          <!-- Siatka -->
          <line x1="0" y1="30" x2="500" y2="30" stroke="rgba(255,255,255,0.03)" />
          <line x1="0" y1="60" x2="500" y2="60" stroke="rgba(255,255,255,0.03)" />
          <line x1="0" y1="90" x2="500" y2="90" stroke="rgba(255,255,255,0.03)" />

          <!-- Obszar CPU -->
          {#if cpuHistory.length > 1}
            <path d={getSvgAreaPath(cpuHistory)} fill="url(#cpu-grad)" />
            <path d={getSvgPath(cpuHistory)} fill="none" stroke="var(--accent-blue)" stroke-width="2" stroke-linecap="round" />
          {/if}

          <!-- Obszar RAM -->
          {#if ramHistory.length > 1}
            <path d={getSvgAreaPath(ramHistory)} fill="url(#ram-grad)" />
            <path d={getSvgPath(ramHistory)} fill="none" stroke="var(--accent-purple)" stroke-width="2" stroke-dasharray="3,3" stroke-linecap="round" />
          {/if}
        </svg>
      </div>
    </div>

    <!-- Sieć -->
    <div class="network-card glass">
      <h3>Prędkość sieciowa (I/O)</h3>
      <div class="net-stats">
        <div class="net-dir">
          <ArrowDown size={32} class="net-icon down" />
          <div class="net-info">
            <span class="label">POBIERANIE</span>
            <span class="value down-val">{downSpeed}</span>
          </div>
        </div>
        <div class="net-dir">
          <ArrowUp size={32} class="net-icon up" />
          <div class="net-info">
            <span class="label">WYSYŁANIE</span>
            <span class="value up-val">{upSpeed}</span>
          </div>
        </div>
      </div>
    </div>
  </section>
</div>

<style>
  .dashboard {
    padding: 30px;
    display: flex;
    flex-direction: column;
    gap: 24px;
    height: 100%;
    overflow-y: auto;
  }

  .dash-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .title-area h1 {
    font-size: 2rem;
    color: white;
  }

  .subtitle {
    color: var(--text-secondary);
    font-size: 0.9rem;
    margin-top: 4px;
  }

  .error-badge {
    background: var(--accent-red-glow);
    border: 1px solid rgba(244, 63, 94, 0.3);
    padding: 8px 16px;
    border-radius: var(--radius-sm);
    color: #ff8595;
    font-size: 0.85rem;
  }

  /* Info panel */
  .system-info-panel {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 24px;
    padding: 20px;
    border-radius: var(--radius-md);
  }

  .info-item {
    display: flex;
    align-items: center;
    gap: 16px;
  }

  .info-icon {
    color: var(--accent-blue);
    background: rgba(0, 210, 255, 0.1);
    padding: 10px;
    border-radius: var(--radius-sm);
    box-shadow: 0 0 10px rgba(0, 210, 255, 0.1);
  }

  .info-label {
    display: block;
    font-size: 0.75rem;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .info-val {
    display: block;
    font-size: 0.95rem;
    color: var(--text-primary);
    font-weight: 500;
    margin-top: 2px;
  }

  /* Metrics Grid */
  .metrics-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 24px;
  }

  .metric-card {
    border-radius: var(--radius-md);
    padding: 24px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 20px;
  }

  .card-header {
    width: 100%;
    display: flex;
    justify-content: space-between;
    align-items: center;
    border-bottom: 1px solid var(--border-color);
    padding-bottom: 12px;
  }

  .card-header h3 {
    font-size: 1rem;
    color: var(--text-primary);
    font-weight: 500;
  }

  .accent-blue-text { color: var(--accent-blue); }
  .accent-purple-text { color: var(--accent-purple); }
  .accent-green-text { color: var(--accent-green); }

  /* Circular Progress Ring */
  .gauge-container {
    position: relative;
    width: 140px;
    height: 140px;
  }

  .ring-gauge {
    transform: rotate(-90deg);
    width: 140px;
    height: 140px;
  }

  .gauge-bg {
    fill: none;
    stroke: rgba(255, 255, 255, 0.03);
    stroke-width: 8;
  }

  .gauge-fill {
    fill: none;
    stroke-width: 8;
    stroke-linecap: round;
    transition: stroke-dashoffset 0.8s cubic-bezier(0.4, 0, 0.2, 1);
  }

  .gauge-fill.cpu { stroke: var(--accent-blue); filter: drop-shadow(0 0 5px var(--accent-blue-glow)); }
  .gauge-fill.ram { stroke: var(--accent-purple); filter: drop-shadow(0 0 5px rgba(168, 85, 247, 0.3)); }
  .gauge-fill.disk { stroke: var(--accent-green); filter: drop-shadow(0 0 5px var(--accent-green-glow)); }

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
    font-size: 1.6rem;
    font-family: var(--font-display);
    font-weight: 700;
    color: white;
  }

  .gauge-value .desc {
    font-size: 0.7rem;
    color: var(--text-secondary);
    margin-top: 4px;
    max-width: 110px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* Charts and network */
  .charts-grid {
    display: grid;
    grid-template-columns: 2fr 1fr;
    gap: 24px;
  }

  .chart-card {
    border-radius: var(--radius-md);
    padding: 24px;
    display: flex;
    flex-direction: column;
    gap: 20px;
  }

  .chart-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .chart-header h3 {
    font-size: 1rem;
    font-weight: 500;
  }

  .legend {
    display: flex;
    gap: 16px;
    font-size: 0.8rem;
  }

  .legend-item {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--text-secondary);
  }

  .color-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
  }
  .color-dot.cpu { background-color: var(--accent-blue); }
  .color-dot.ram { background-color: var(--accent-purple); }

  .chart-container {
    height: 120px;
    width: 100%;
    background: rgba(0, 0, 0, 0.15);
    border-radius: var(--radius-sm);
    padding: 10px;
    overflow: hidden;
  }

  .chart-svg {
    width: 100%;
    height: 100%;
    overflow: visible;
  }

  /* Network Card */
  .network-card {
    border-radius: var(--radius-md);
    padding: 24px;
    display: flex;
    flex-direction: column;
    gap: 20px;
  }

  .network-card h3 {
    font-size: 1rem;
    font-weight: 500;
  }

  .net-stats {
    display: flex;
    flex-direction: column;
    gap: 20px;
    justify-content: center;
    height: 100%;
  }

  .net-dir {
    display: flex;
    align-items: center;
    gap: 16px;
  }

  .net-icon {
    padding: 8px;
    border-radius: var(--radius-sm);
  }

  .net-icon.down {
    color: var(--accent-blue);
    background: rgba(0, 210, 255, 0.08);
  }

  .net-icon.up {
    color: var(--accent-purple);
    background: rgba(168, 85, 247, 0.08);
  }

  .net-info {
    display: flex;
    flex-direction: column;
  }

  .net-info .label {
    font-size: 0.7rem;
    color: var(--text-muted);
    font-weight: 600;
    letter-spacing: 0.05em;
  }

  .net-info .value {
    font-size: 1.3rem;
    font-family: var(--font-display);
    font-weight: 700;
    color: white;
    margin-top: 2px;
  }
</style>
