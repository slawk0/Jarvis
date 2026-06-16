<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import { 
    Settings, Globe, Shield, Activity, ShieldAlert, Key, Plus, Trash2, 
    Edit2, RefreshCw, UserCheck, UserPlus, ListFilter, Info, Check, 
    X, MapPin, Search, ChevronRight, ChevronLeft, Calendar, User, 
    Link, Radio, Laptop, Users
  } from 'lucide-svelte';
  import PangolinWorldMap from './PangolinWorldMap.svelte';
  import {
    formatCompact,
    getCountryName,
    trafficBarGradient,
    trafficIntensity,
    type CountryTraffic
  } from '$lib/geo/countryUtils';

  // Navigation state
  let activeSubTab = $state('dashboard'); // 'dashboard', 'logs', 'tunnels', 'priv_resources', 'pub_resources', 'access', 'clients', 'settings'
  
  // Settings State
  let config = $state({ api_url: 'https://api.pangolin.net', org_id: '', has_api_key: false });
  let apiKeyInput = $state('');
  let orgs = $state<any[]>([]);
  let isConfigLoading = $state(true);
  let isSavingConfig = $state(false);
  let configMsg = $state({ text: '', type: 'info' });
  let isConnectedPangolin = $state(false);

  // Dashboard / Analytics State
  let timeRange = $state('7d'); // '24h', '7d', '30d'
  let isDashboardLoading = $state(false);
  let dashboardStats = $state({
    totalRequests: 0,
    totalBlocked: 0,
    requestsPerDay: [] as any[],
    requestsPerCountry: [] as any[]
  });

  // Logs State
  let isLogsLoading = $state(false);
  let logsList = $state<any[]>([]);
  let logFilters = $state({
    limit: 50,
    offset: 0,
    action: '', // 'allowed', 'blocked', ''
    method: '',
    host: '',
    path: '',
    actor: ''
  });
  let logsPagination = $state({ total: 0, limit: 50, offset: 0 });
  let selectedLogDetail = $state<any | null>(null);
  let uniqueFilters = $state({
    actors: [] as string[],
    hosts: [] as string[],
    paths: [] as string[],
    locations: [] as string[]
  });

  // Tunnels / Sites State
  let isSitesLoading = $state(false);
  let sitesList = $state<any[]>([]);
  let showCreateSiteModal = $state(false);
  let newSiteData = $state({
    name: '',
    type: 'wireguard', // 'wireguard' | 'newt' | 'local'
    subnet: '',
    niceId: ''
  });

  // Private Resources State
  let isPrivResourcesLoading = $state(false);
  let privResourcesList = $state<any[]>([]);
  let showCreatePrivResModal = $state(false);
  let isEditingPrivRes = $state(false);
  let selectedPrivResId = $state<number | null>(null);
  let privResForm = $state({
    name: '',
    mode: 'host', // 'host' | 'cidr' | 'http' | 'ssh'
    destination: '',
    destinationPort: 80,
    scheme: 'http',
    ssl: false,
    niceId: '',
    tcpPortRangeString: '',
    udpPortRangeString: '',
    disableIcmp: false,
    siteIds: [] as number[],
    userIds: [] as string[],
    roleIds: [] as number[],
    clientIds: [] as number[]
  });

  // Public Resources State
  let isPubResourcesLoading = $state(false);
  let pubResourcesList = $state<any[]>([]);
  let showCreatePubResModal = $state(false);
  let isEditingPubRes = $state(false);
  let selectedPubResId = $state<string | null>(null);
  let pubResForm = $state({
    name: '',
    domainId: '',
    subdomain: '',
    mode: 'http', // 'http' | 'ssh' | 'rdp' | 'vnc' | 'tcp' | 'udp'
    stickySession: false,
    postAuthPath: '',
    proxyPort: 0,
    targetSiteId: '' as string | number,
    targetIp: '',
    targetPort: 80
  });
  let domainsList = $state<any[]>([]);

  let isPubResFormInvalid = $derived(
    !pubResForm.name || 
    !pubResForm.targetSiteId || 
    !pubResForm.targetIp || 
    !pubResForm.targetPort ||
    ((pubResForm.mode !== 'tcp' && pubResForm.mode !== 'udp') && !pubResForm.domainId) ||
    ((pubResForm.mode === 'tcp' || pubResForm.mode === 'udp') && !pubResForm.proxyPort)
  );

  async function handleOpenUrl(url: string) {
    try {
      await openUrl(url);
    } catch (err) {
      console.error("Failed to open URL:", err);
    }
  }

  // Access Control State (Users, Roles, IDPs, Invitations)
  let activeAccessSubTab = $state('users'); // 'users', 'roles', 'idps', 'invitations'
  let isAccessLoading = $state(false);
  let usersList = $state<any[]>([]);
  let rolesList = $state<any[]>([]);
  let idpsList = $state<any[]>([]);
  let invitationsList = $state<any[]>([]);

  let showInviteModal = $state(false);
  let inviteForm = $state({
    email: '',
    roleIds: [] as number[]
  });

  let showCreateRoleModal = $state(false);
  let isEditingRole = $state(false);
  let selectedRoleId = $state<number | null>(null);
  let roleForm = $state({
    name: '',
    description: '',
    requireDeviceApproval: false,
    allowSsh: false,
    sshSudoMode: 'none', // 'none' | 'full' | 'commands'
    sshSudoCommands: [] as string[],
    sshCreateHomeDir: true,
    sshUnixGroups: [] as string[]
  });
  let newSudoCommand = $state('');
  let newUnixGroup = $state('');

  // Clients / Devices State
  let isClientsLoading = $state(false);
  let clientsList = $state<any[]>([]);
  let activeClientsTab = $state('devices'); // 'devices', 'tokens'
  let accessTokensList = $state<any[]>([]);

  // Helpers for API requests
  async function apiCall(method: string, path: string, query: any = null, body: any = null) {
    // Ensure path is prefixed with /v1
    const cleanPath = path.startsWith('/v1/') ? path : `/v1/${path.startsWith('/') ? path.slice(1) : path}`;
    try {
      const res = await invoke<any>('pangolin_api_request', {
        method,
        path: cleanPath,
        queryParams: query,
        body
      });
      return res;
    } catch (err: any) {
      console.error(`API Call failed (${method} ${cleanPath}):`, err);
      throw err;
    }
  }

  async function verifyConnection(): Promise<boolean> {
    try {
      const res = await apiCall('GET', '/orgs');
      if (res && res.data) {
        orgs = res.data;
        if (!config.org_id && orgs.length > 0) {
          config.org_id = orgs[0].orgId;
          await invoke('save_pangolin_config', {
            apiUrl: config.api_url,
            orgId: config.org_id,
            apiKey: null
          });
        }
        isConnectedPangolin = true;
        configMsg = { text: 'Połączenie pomyślne! Wykryto i wczytano organizacje.', type: 'success' };
        return true;
      }
    } catch (err: any) {
      const errMsg = err.toString();
      console.warn('Failed to list organizations:', err);
      
      // Fallback for org-scoped API keys: they won't list all orgs
      const isAuthOrScopedError = 
        errMsg.includes('401') || 
        errMsg.includes('403') || 
        errMsg.includes('Unauthorized') || 
        errMsg.includes('Forbidden') || 
        errMsg.includes('root access');

      if (isAuthOrScopedError) {
        if (config.org_id) {
          try {
            await apiCall('GET', `/org/${config.org_id}/sites`);
            isConnectedPangolin = true;
            configMsg = { text: 'Połączenie pomyślne dla organizacji: ' + config.org_id, type: 'success' };
            return true;
          } catch (orgErr: any) {
            isConnectedPangolin = false;
            configMsg = { 
              text: 'Nieprawidłowy klucz API lub ID organizacji (brak dostępu). Upewnij się, że dane są poprawne.', 
              type: 'error' 
            };
            return false;
          }
        } else {
          isConnectedPangolin = false;
          configMsg = { 
            text: 'Wykryto klucz o ograniczonym zasięgu (brak dostępu administratora root). Wpisz ID organizacji ręcznie w polu poniżej.', 
            type: 'warning' 
          };
          return false;
        }
      } else {
        isConnectedPangolin = false;
        configMsg = { text: 'Błąd połączenia: ' + errMsg, type: 'error' };
        return false;
      }
    }
    isConnectedPangolin = false;
    return false;
  }

  // Load configuration on mount
  async function loadConfig() {
    isConfigLoading = true;
    configMsg = { text: '', type: 'info' };
    try {
      const c = await invoke<any>('get_pangolin_config');
      config = {
        api_url: c.api_url || 'https://api.pangolin.net',
        org_id: c.org_id || '',
        has_api_key: c.has_api_key || false
      };
      
      if (config.has_api_key) {
        const success = await verifyConnection();
        if (success) {
          await loadTabData();
        } else {
          activeSubTab = 'settings';
        }
      } else {
        activeSubTab = 'settings';
        isConnectedPangolin = false;
        configMsg = { text: 'Brak klucza API. Skonfiguruj połączenie.', type: 'warning' };
      }
    } catch (err: any) {
      isConnectedPangolin = false;
      configMsg = { text: 'Błąd wczytywania konfiguracji: ' + err.toString(), type: 'error' };
    } finally {
      isConfigLoading = false;
    }
  }

  async function handleSaveConfig() {
    isSavingConfig = true;
    configMsg = { text: 'Zapisywanie i weryfikacja połączenia...', type: 'info' };
    try {
      await invoke('save_pangolin_config', {
        apiUrl: config.api_url,
        orgId: config.org_id || null,
        apiKey: apiKeyInput ? apiKeyInput : null
      });
      
      apiKeyInput = '';
      config.has_api_key = true;
      
      const success = await verifyConnection();
      if (success) {
        await loadTabData();
      }
    } catch (err: any) {
      isConnectedPangolin = false;
      configMsg = { text: 'Nie udało się zapisać: ' + err.toString(), type: 'error' };
    } finally {
      isSavingConfig = false;
    }
  }

  // Load Data depending on active tab
  async function loadTabData() {
    if (!config.has_api_key) return;
    if (!isConnectedPangolin) return;

    if (activeSubTab === 'dashboard') {
      await loadDashboardData();
    } else if (activeSubTab === 'logs') {
      await loadLogsData();
    } else if (activeSubTab === 'resources') {
      await loadSitesList();
      await loadPrivResources();
      await loadPubResources();
      await loadDomains();
    } else if (activeSubTab === 'access') {
      await loadAccessData();
    } else if (activeSubTab === 'clients') {
      await loadClientsData();
    }
  }

  $effect(() => {
    if (activeSubTab) {
      loadTabData();
    }
  });

  // 1. Dashboard Tab Functions
  async function loadDashboardData() {
    if (!config.org_id) return;
    isDashboardLoading = true;
    try {
      // Calculate start time based on range
      const end = new Date();
      const start = new Date();
      if (timeRange === '24h') start.setHours(start.getHours() - 24);
      else if (timeRange === '30d') start.setDate(start.getDate() - 30);
      else start.setDate(start.getDate() - 7); // '7d'

      const res = await apiCall('GET', `/org/${config.org_id}/logs/analytics`, {
        timeStart: start.toISOString(),
        timeEnd: end.toISOString()
      });

      if (res && res.data) {
        dashboardStats = {
          totalRequests: res.data.totalRequests || 0,
          totalBlocked: res.data.totalBlocked || 0,
          requestsPerDay: res.data.requestsPerDay || [],
          requestsPerCountry: res.data.requestsPerCountry || []
        };
      }
    } catch (err: any) {
      console.error('Failed to load dashboard statistics:', err);
    } finally {
      isDashboardLoading = false;
    }
  }

  // Skala wykresu oparta na sumie serii na dany dzień (dla wykresu skumulowanego)
  let maxChartValue = $derived.by(() => {
    const days = dashboardStats.requestsPerDay;
    if (days.length === 0) return 1;
    const peak = Math.max(
      ...days.map(d => (d.allowedCount || 0) + (d.blockedCount || 0)),
      1
    );
    return Math.ceil(peak * 1.15); // 15% zapasu na górze
  });

  let hoveredMapCode = $state<string | null>(null);
  let hoveredCountryRow = $state<string | null>(null);
  let activeTooltip = $state<{
    geo: any;
    pct: string;
    rect: DOMRect;
  } | null>(null);
  let activeChartTooltip = $state<{
    day: any;
    rect: DOMRect;
  } | null>(null);

  const sortedCountries = $derived(
    [...(dashboardStats.requestsPerCountry as CountryTraffic[])].sort(
      (a, b) => (b.count || 0) - (a.count || 0)
    )
  );

  const maxCountryCount = $derived(
    sortedCountries.length > 0
      ? Math.max(...sortedCountries.map(c => c.count || 0), 1)
      : 1
  );

  const chartYTicks = $derived.by(() => {
    const max = maxChartValue;
    if (max <= 4) return [max, Math.max(1, Math.ceil(max * 0.66)), Math.max(0, Math.ceil(max * 0.33)), 0];
    return [max, Math.round(max * 0.66), Math.round(max * 0.33), 0];
  });

  const CHART_PLOT_HEIGHT = 168;
  const CHART_SLOT_VB = 44;
  const CHART_BAR_RATIO = 0.42;

  const chartLayout = {
    plotHeight: CHART_PLOT_HEIGHT,
    pad: { top: 8, bottom: 4, left: 16, right: 16 }
  };

  const chartViewWidth = $derived.by(() => {
    const days = Math.max(dashboardStats.requestsPerDay.length, 1);
    return chartLayout.pad.left + chartLayout.pad.right + days * CHART_SLOT_VB;
  });

  const chartDisplayMaxWidth = $derived.by(() => {
    const days = dashboardStats.requestsPerDay.length;
    if (days === 0) return undefined;
    return Math.min(days * 46 + 52, 680);
  });

  function formatDayLabel(day: string): string {
    if (!day) return '';
    const d = new Date(day);
    if (Number.isNaN(d.getTime())) return day.slice(-5);
    return d.toLocaleDateString('pl-PL', { day: 'numeric', month: 'short' });
  }

  function getCountryTooltip(geo: CountryTraffic) {
    const total = geo.count || 0;
    const pct = dashboardStats.totalRequests > 0
      ? ((total / dashboardStats.totalRequests) * 100).toFixed(1)
      : '0';
    const allowed = geo.allowedCount ?? (total - (geo.blockedCount || 0));
    const blocked = geo.blockedCount ?? 0;
    return `${getCountryName(geo)}: ${formatCompact(total)} zapytań (${pct}%) · dozwolone: ${formatCompact(allowed)}, zablokowane: ${formatCompact(blocked)}`;
  }

  // 2. Logs Tab Functions
  async function loadLogsData() {
    if (!config.org_id) return;
    isLogsLoading = true;
    try {
      const end = new Date();
      const start = new Date();
      // default 7 days for request logs
      start.setDate(start.getDate() - 7);

      const qParams: any = {
        timeStart: start.toISOString(),
        timeEnd: end.toISOString(),
        limit: logFilters.limit.toString(),
        offset: logFilters.offset.toString()
      };

      if (logFilters.action === 'allowed') qParams.action = 'true';
      if (logFilters.action === 'blocked') qParams.action = 'false';
      if (logFilters.method) qParams.method = logFilters.method;
      if (logFilters.host) qParams.host = logFilters.host;
      if (logFilters.path) qParams.path = logFilters.path;
      if (logFilters.actor) qParams.actor = logFilters.actor;

      const res = await apiCall('GET', `/org/${config.org_id}/logs/request`, qParams);
      if (res && res.data) {
        logsList = res.data.log || [];
        logsPagination = res.data.pagination || { total: 0, limit: 50, offset: 0 };
        
        // Extract filter options from filterAttributes
        if (res.data.filterAttributes) {
          const fa = res.data.filterAttributes;
          uniqueFilters = {
            actors: fa.actors?.map((a: any) => a.actor).filter(Boolean) || [],
            hosts: fa.hosts?.map((h: any) => h.hosts).filter(Boolean) || [],
            paths: fa.paths?.map((p: any) => p.paths).filter(Boolean) || [],
            locations: fa.locations?.map((l: any) => l.locations).filter(Boolean) || []
          };
        }
      }
    } catch (err: any) {
      console.error('Failed to query request logs:', err);
    } finally {
      isLogsLoading = false;
    }
  }

  function formatTime(timestamp: number): string {
    return new Date(timestamp * 1000).toLocaleString('pl-PL', {
      hour: '2-digit', minute: '2-digit', second: '2-digit',
      day: '2-digit', month: '2-digit', year: 'numeric'
    });
  }

  // 3. Tunnels / Sites Functions
  async function loadSitesList() {
    if (!config.org_id) return;
    isSitesLoading = true;
    try {
      const res = await apiCall('GET', `/org/${config.org_id}/sites`);
      if (res && res.data) {
        sitesList = res.data.sites || [];
      }
    } catch (err) {
      console.error(err);
    } finally {
      isSitesLoading = false;
    }
  }

  async function handleCreateSite() {
    if (!config.org_id || !newSiteData.name) return;
    try {
      await apiCall('PUT', `/org/${config.org_id}/site`, null, {
        name: newSiteData.name,
        type: newSiteData.type,
        subnet: newSiteData.subnet || undefined,
        niceId: newSiteData.niceId || undefined
      });
      showCreateSiteModal = false;
      newSiteData = { name: '', type: 'wireguard', subnet: '', niceId: '' };
      await loadSitesList();
    } catch (err: any) {
      alert('Nie udało się utworzyć tunelu: ' + err.toString());
    }
  }

  async function handleDeleteSite(siteId: number) {
    if (!confirm('Czy na pewno chcesz usunąć ten tunel? Wszystkie powiązane zasoby zostaną rozłączone.')) return;
    try {
      await apiCall('DELETE', `/site/${siteId}`);
      await loadSitesList();
    } catch (err: any) {
      alert('Nie udało się usunąć tunelu: ' + err.toString());
    }
  }

  // 4. Private Resources Functions
  async function loadPrivResources() {
    if (!config.org_id) return;
    isPrivResourcesLoading = true;
    try {
      const res = await apiCall('GET', `/org/${config.org_id}/site-resources`);
      if (res && res.data) {
        privResourcesList = res.data.siteResources || [];
      }
    } catch (err) {
      console.error(err);
    } finally {
      isPrivResourcesLoading = false;
    }
  }

  async function handleSavePrivRes() {
    if (!config.org_id || !privResForm.name || !privResForm.destination) return;
    
    const payload = {
      name: privResForm.name,
      mode: privResForm.mode,
      destination: privResForm.destination,
      destinationPort: privResForm.mode === 'http' || privResForm.mode === 'ssh' ? Number(privResForm.destinationPort) : undefined,
      scheme: privResForm.mode === 'http' ? privResForm.scheme : undefined,
      ssl: privResForm.mode === 'http' ? privResForm.ssl : undefined,
      niceId: privResForm.niceId || undefined,
      tcpPortRangeString: privResForm.mode === 'host' || privResForm.mode === 'cidr' ? privResForm.tcpPortRangeString || undefined : undefined,
      udpPortRangeString: privResForm.mode === 'host' || privResForm.mode === 'cidr' ? privResForm.udpPortRangeString || undefined : undefined,
      disableIcmp: privResForm.mode === 'cidr' ? privResForm.disableIcmp : undefined,
      siteIds: privResForm.siteIds,
      userIds: privResForm.userIds,
      roleIds: privResForm.roleIds,
      clientIds: privResForm.clientIds
    };

    try {
      if (isEditingPrivRes && selectedPrivResId !== null) {
        await apiCall('POST', `/site-resource/${selectedPrivResId}`, null, payload);
      } else {
        await apiCall('PUT', `/org/${config.org_id}/site-resource`, null, payload);
      }
      showCreatePrivResModal = false;
      resetPrivResForm();
      await loadPrivResources();
    } catch (err: any) {
      alert('Nie udało się zapisać zasobu: ' + err.toString());
    }
  }

  function editPrivRes(res: any) {
    isEditingPrivRes = true;
    selectedPrivResId = res.siteResourceId;
    privResForm = {
      name: res.name || '',
      mode: res.mode || 'host',
      destination: res.destination || '',
      destinationPort: res.destinationPort || 80,
      scheme: res.scheme || 'http',
      ssl: res.ssl || false,
      niceId: res.niceId || '',
      tcpPortRangeString: res.tcpPortRangeString || '',
      udpPortRangeString: res.udpPortRangeString || '',
      disableIcmp: res.disableIcmp || false,
      siteIds: res.sites?.map((s: any) => s.siteId) || [],
      userIds: res.users?.map((u: any) => u.userId) || [],
      roleIds: res.roles?.map((r: any) => r.roleId) || [],
      clientIds: res.clients?.map((c: any) => c.clientId) || []
    };
    showCreatePrivResModal = true;
  }

  async function handleDeletePrivRes(resId: number) {
    if (!confirm('Czy na pewno chcesz usunąć ten zasób prywatny?')) return;
    try {
      await apiCall('DELETE', `/site-resource/${resId}`);
      await loadPrivResources();
    } catch (err: any) {
      alert('Nie udało się usunąć zasobu: ' + err.toString());
    }
  }

  function resetPrivResForm() {
    privResForm = {
      name: '', mode: 'host', destination: '', destinationPort: 80, scheme: 'http', ssl: false, niceId: '',
      tcpPortRangeString: '', udpPortRangeString: '', disableIcmp: false, siteIds: [], userIds: [], roleIds: [], clientIds: []
    };
    isEditingPrivRes = false;
    selectedPrivResId = null;
  }

  // 5. Public Resources Functions
  async function loadPubResources() {
    if (!config.org_id) return;
    isPubResourcesLoading = true;
    try {
      const res = await apiCall('GET', `/org/${config.org_id}/resources`);
      if (res && res.data) {
        pubResourcesList = res.data.resources || [];
      }
    } catch (err) {
      console.error(err);
    } finally {
      isPubResourcesLoading = false;
    }
  }

  async function loadDomains() {
    if (!config.org_id) return;
    try {
      const res = await apiCall('GET', `/org/${config.org_id}/domains`);
      if (res && res.data) {
        domainsList = res.data.domains || [];
      }
    } catch (err) {
      console.error(err);
    }
  }

  async function handleSavePubRes() {
    if (!config.org_id || !pubResForm.name) return;

    // Build payload according to Zod specs
    const isProxyPortMode = pubResForm.mode === 'tcp' || pubResForm.mode === 'udp';
    const payload: any = {
      name: pubResForm.name,
      mode: pubResForm.mode,
    };

    if (isProxyPortMode) {
      payload.proxyPort = Number(pubResForm.proxyPort);
    } else {
      payload.domainId = pubResForm.domainId;
      payload.subdomain = pubResForm.subdomain || null;
      payload.stickySession = pubResForm.stickySession;
      payload.postAuthPath = pubResForm.postAuthPath || null;
    }

    try {
      let resourceId: number;
      if (isEditingPubRes && selectedPubResId !== null) {
        await apiCall('POST', `/resource/${selectedPubResId}`, null, payload);
        resourceId = Number(selectedPubResId);
      } else {
        const res = await apiCall('PUT', `/org/${config.org_id}/resource`, null, payload);
        resourceId = res.data?.resourceId || res.resourceId;
      }

      // Save Target
      if (resourceId && pubResForm.targetSiteId && pubResForm.targetIp) {
        const matchedRes = pubResourcesList.find(r => r.resourceId === resourceId);
        const firstTarget = matchedRes?.targets && matchedRes.targets.length > 0 ? matchedRes.targets[0] : null;

        const targetPayload = {
          siteId: Number(pubResForm.targetSiteId),
          ip: pubResForm.targetIp,
          port: Number(pubResForm.targetPort),
          enabled: true
        };

        if (firstTarget && firstTarget.targetId) {
          await apiCall('POST', `/target/${firstTarget.targetId}`, null, targetPayload);
        } else {
          await apiCall('PUT', `/resource/${resourceId}/target`, null, targetPayload);
        }
      }

      showCreatePubResModal = false;
      resetPubResForm();
      await loadPubResources();
    } catch (err: any) {
      alert('Nie udało się zapisać zasobu publicznego: ' + err.toString());
    }
  }

  function editPubRes(res: any) {
    isEditingPubRes = true;
    selectedPubResId = res.resourceId;

    const matchedDomain = domainsList.find(d => d.domainId === res.domainId);
    let extractedSubdomain = '';
    if (matchedDomain && res.fullDomain) {
      const base = matchedDomain.baseDomain;
      if (res.fullDomain === base) {
        extractedSubdomain = '';
      } else if (res.fullDomain.endsWith('.' + base)) {
        extractedSubdomain = res.fullDomain.slice(0, -(base.length + 1));
      }
    }

    // Get target info if present
    const firstTarget = res.targets && res.targets.length > 0 ? res.targets[0] : null;
    const targetSiteId = firstTarget ? firstTarget.siteId : '';
    const targetIp = firstTarget ? firstTarget.ip : '';
    const targetPort = firstTarget ? firstTarget.port : 80;

    pubResForm = {
      name: res.name || '',
      domainId: res.domainId || '',
      subdomain: extractedSubdomain,
      mode: res.mode || 'http',
      stickySession: res.stickySession || false,
      postAuthPath: res.postAuthPath || '',
      proxyPort: res.proxyPort || 0,
      targetSiteId,
      targetIp,
      targetPort
    };
    showCreatePubResModal = true;
  }

  async function handleDeletePubRes(resId: string) {
    if (!confirm('Czy na pewno chcesz usunąć ten zasób publiczny?')) return;
    try {
      await apiCall('DELETE', `/resource/${resId}`);
      await loadPubResources();
    } catch (err: any) {
      alert('Nie udało się usunąć: ' + err.toString());
    }
  }

  function resetPubResForm() {
    pubResForm = { 
      name: '', domainId: '', subdomain: '', mode: 'http', stickySession: false, postAuthPath: '', proxyPort: 0,
      targetSiteId: '', targetIp: '', targetPort: 80
    };
    isEditingPubRes = false;
    selectedPubResId = null;
  }

  // 6. Access Control Functions
  async function loadAccessData() {
    if (!config.org_id) return;
    isAccessLoading = true;
    try {
      if (activeAccessSubTab === 'users') {
        const res = await apiCall('GET', `/org/${config.org_id}/users`);
        if (res && res.data) usersList = res.data.users || [];
      } else if (activeAccessSubTab === 'roles') {
        const res = await apiCall('GET', `/org/${config.org_id}/roles`);
        if (res && res.data) rolesList = res.data.roles || [];
      } else if (activeAccessSubTab === 'idps') {
        const res = await apiCall('GET', `/org/${config.org_id}/idp`);
        if (res && res.data) idpsList = res.data.idps || res.data.idp || [];
      } else if (activeAccessSubTab === 'invitations') {
        const res = await apiCall('GET', `/org/${config.org_id}/invitations`);
        if (res && res.data) invitationsList = res.data.invitations || [];
      }
    } catch (err) {
      console.error(err);
    } finally {
      isAccessLoading = false;
    }
  }

  $effect(() => {
    if (activeSubTab === 'access' && activeAccessSubTab) {
      loadAccessData();
    }
  });

  async function handleSendInvite() {
    if (!config.org_id || !inviteForm.email) return;
    try {
      await apiCall('POST', `/org/${config.org_id}/create-invite`, null, {
        email: inviteForm.email,
        roleIds: inviteForm.roleIds
      });
      showInviteModal = false;
      inviteForm = { email: '', roleIds: [] };
      if (activeAccessSubTab === 'invitations') await loadAccessData();
    } catch (err: any) {
      alert('Błąd zaproszenia: ' + err.toString());
    }
  }

  async function handleCancelInvite(inviteId: string) {
    if (!config.org_id || !confirm('Czy chcesz anulować to zaproszenie?')) return;
    try {
      await apiCall('DELETE', `/org/${config.org_id}/invitations/${inviteId}`);
      await loadAccessData();
    } catch (err: any) {
      alert('Błąd anulowania: ' + err.toString());
    }
  }

  async function handleSaveRole() {
    if (!config.org_id || !roleForm.name) return;
    try {
      const payload = {
        name: roleForm.name,
        description: roleForm.description || undefined,
        requireDeviceApproval: roleForm.requireDeviceApproval,
        allowSsh: roleForm.allowSsh,
        sshSudoMode: roleForm.allowSsh ? roleForm.sshSudoMode : 'none',
        sshSudoCommands: roleForm.allowSsh && roleForm.sshSudoMode === 'commands' ? roleForm.sshSudoCommands : [],
        sshCreateHomeDir: roleForm.allowSsh ? roleForm.sshCreateHomeDir : undefined,
        sshUnixGroups: roleForm.allowSsh ? roleForm.sshUnixGroups : []
      };

      if (isEditingRole && selectedRoleId !== null) {
        await apiCall('POST', `/role/${selectedRoleId}`, null, payload);
      } else {
        await apiCall('PUT', `/org/${config.org_id}/role`, null, payload);
      }
      showCreateRoleModal = false;
      resetRoleForm();
      await loadAccessData();
    } catch (err: any) {
      alert('Błąd zapisu roli: ' + err.toString());
    }
  }

  function editRole(role: any) {
    isEditingRole = true;
    selectedRoleId = role.roleId;
    roleForm = {
      name: role.name || '',
      description: role.description || '',
      requireDeviceApproval: role.requireDeviceApproval || false,
      allowSsh: role.allowSsh || false,
      sshSudoMode: role.sshSudoMode || 'none',
      sshSudoCommands: role.sshSudoCommands || [],
      sshCreateHomeDir: role.sshCreateHomeDir ?? true,
      sshUnixGroups: role.sshUnixGroups || []
    };
    showCreateRoleModal = true;
  }

  async function handleDeleteRole(roleId: number) {
    if (!confirm('Czy na pewno chcesz usunąć tę rolę?')) return;
    try {
      await apiCall('DELETE', `/role/${roleId}`);
      await loadAccessData();
    } catch (err: any) {
      alert('Błąd usuwania roli: ' + err.toString());
    }
  }

  function addSudoCommand() {
    if (newSudoCommand && !roleForm.sshSudoCommands.includes(newSudoCommand)) {
      roleForm.sshSudoCommands = [...roleForm.sshSudoCommands, newSudoCommand];
      newSudoCommand = '';
    }
  }

  function removeSudoCommand(cmd: string) {
    roleForm.sshSudoCommands = roleForm.sshSudoCommands.filter(c => c !== cmd);
  }

  function addUnixGroup() {
    if (newUnixGroup && !roleForm.sshUnixGroups.includes(newUnixGroup)) {
      roleForm.sshUnixGroups = [...roleForm.sshUnixGroups, newUnixGroup];
      newUnixGroup = '';
    }
  }

  function removeUnixGroup(grp: string) {
    roleForm.sshUnixGroups = roleForm.sshUnixGroups.filter(g => g !== grp);
  }

  function resetRoleForm() {
    roleForm = {
      name: '', description: '', requireDeviceApproval: false, allowSsh: false,
      sshSudoMode: 'none', sshSudoCommands: [], sshCreateHomeDir: true, sshUnixGroups: []
    };
    isEditingRole = false;
    selectedRoleId = null;
  }

  // 7. Clients / Devices Functions
  async function loadClientsData() {
    if (!config.org_id) return;
    isClientsLoading = true;
    try {
      if (activeClientsTab === 'devices') {
        const res = await apiCall('GET', `/org/${config.org_id}/user-devices`);
        if (res && res.data) clientsList = res.data.devices || [];
      } else {
        const res = await apiCall('GET', `/org/${config.org_id}/access-tokens`);
        if (res && res.data) accessTokensList = res.data.accessTokens || [];
      }
    } catch (err) {
      console.error(err);
    } finally {
      isClientsLoading = false;
    }
  }

  $effect(() => {
    if (activeSubTab === 'clients' && activeClientsTab) {
      loadClientsData();
    }
  });

  async function toggleBlockClient(client: any) {
    const isBlocked = client.blocked;
    const action = isBlocked ? 'unblock' : 'block';
    if (!confirm(`Czy na pewno chcesz ${isBlocked ? 'odblokować' : 'zablokować'} urządzenie ${client.name}?`)) return;
    try {
      await apiCall('POST', `/client/${client.clientId}/${action}`);
      await loadClientsData();
    } catch (err: any) {
      alert(`Błąd ${action}owania: ` + err.toString());
    }
  }

  async function toggleArchiveClient(client: any) {
    const isArchived = client.archived;
    const action = isArchived ? 'unarchive' : 'archive';
    if (!confirm(`Czy na pewno chcesz ${isArchived ? 'przywrócić' : 'zarchiwizować'} urządzenie ${client.name}?`)) return;
    try {
      await apiCall('POST', `/client/${client.clientId}/${action}`);
      await loadClientsData();
    } catch (err: any) {
      alert(`Błąd archiwizacji: ` + err.toString());
    }
  }

  async function handleDeleteClient(clientId: number) {
    if (!confirm('Czy na pewno chcesz bezpowrotnie usunąć to urządzenie?')) return;
    try {
      await apiCall('DELETE', `/client/${clientId}`);
      await loadClientsData();
    } catch (err: any) {
      alert('Błąd usuwania urządzenia: ' + err.toString());
    }
  }

  async function handleDeleteToken(tokenId: string) {
    if (!confirm('Czy chcesz usunąć i unieważnić ten token dostępu?')) return;
    try {
      await apiCall('DELETE', `/access-token/${tokenId}`);
      await loadClientsData();
    } catch (err: any) {
      alert('Błąd usuwania tokenu: ' + err.toString());
    }
  }

  onMount(() => {
    loadConfig();
  });
</script>

<div class="pangolin-container">
  <header class="tab-header">
    <div class="header-left">
      <Globe class="header-icon" />
      <div class="title-block">
        <h2>Pangolin Zero-Trust Proxy</h2>
        <span class="subtitle">Brama tożsamości i zabezpieczony tunel zdalny</span>
      </div>
    </div>
    <div class="tab-navbar">
      <button 
        class="nav-btn" 
        class:active={activeSubTab === 'dashboard'} 
        onclick={() => activeSubTab = 'dashboard'}
        disabled={!isConnectedPangolin}
        title={!isConnectedPangolin ? "Skonfiguruj i zweryfikuj połączenie z Pangolin, aby odblokować tę zakładkę" : ""}
      >
        <Activity size={16} /> Statystyki
      </button>
      <button 
        class="nav-btn" 
        class:active={activeSubTab === 'logs'} 
        onclick={() => activeSubTab = 'logs'}
        disabled={!isConnectedPangolin}
        title={!isConnectedPangolin ? "Skonfiguruj i zweryfikuj połączenie z Pangolin, aby odblokować tę zakładkę" : ""}
      >
        <RefreshCw size={16} /> Audyt
      </button>
      <button 
        class="nav-btn" 
        class:active={activeSubTab === 'resources'} 
        onclick={() => activeSubTab = 'resources'}
        disabled={!isConnectedPangolin}
        title={!isConnectedPangolin ? "Skonfiguruj i zweryfikuj połączenie z Pangolin, aby odblokować tę zakładkę" : ""}
      >
        <Radio size={16} /> TUNELE / ZASOBY
      </button>
      <button 
        class="nav-btn" 
        class:active={activeSubTab === 'access'} 
        onclick={() => activeSubTab = 'access'}
        disabled={!isConnectedPangolin}
        title={!isConnectedPangolin ? "Skonfiguruj i zweryfikuj połączenie z Pangolin, aby odblokować tę zakładkę" : ""}
      >
        <Shield size={16} /> Kontrola Dostępu
      </button>
      <button 
        class="nav-btn" 
        class:active={activeSubTab === 'clients'} 
        onclick={() => activeSubTab = 'clients'}
        disabled={!isConnectedPangolin}
        title={!isConnectedPangolin ? "Skonfiguruj i zweryfikuj połączenie z Pangolin, aby odblokować tę zakładkę" : ""}
      >
        <Laptop size={16} /> Urządzenia
      </button>
      <button class="nav-btn" class:active={activeSubTab === 'settings'} onclick={() => activeSubTab = 'settings'}>
        <Settings size={16} /> Ustawienia API
      </button>
    </div>
  </header>

  <div class="tab-content scrollable">
    {#if isConfigLoading}
      <div class="loading-state">
        <RefreshCw class="spin" size={32} />
        <p>Wczytywanie konfiguracji Pangolin...</p>
      </div>
    {:else}
      <!-- 1. DASHBOARD VIEW -->
      {#if activeSubTab === 'dashboard'}
        <div class="dashboard-view">
          <div class="stats-row">
            <div class="stat-card glass glow-amber">
              <span class="card-label">Łącznie</span>
              <span class="card-val tabular-nums">{formatCompact(dashboardStats.totalRequests)}</span>
            </div>
            <div class="stat-card glass glow-red">
              <span class="card-label">Zablokowane</span>
              <span class="card-val tabular-nums text-red">{formatCompact(dashboardStats.totalBlocked)}</span>
            </div>
            <div class="stat-card glass glow-green">
              <span class="card-label">Zaakceptowane</span>
              <span class="card-val tabular-nums text-green">{formatCompact(dashboardStats.totalRequests - dashboardStats.totalBlocked)}</span>
            </div>
            <div class="stat-card glass glow-orange">
              <span class="card-label">Blokady</span>
              <span class="card-val tabular-nums text-orange">
                {dashboardStats.totalRequests > 0 
                  ? ((dashboardStats.totalBlocked / dashboardStats.totalRequests) * 100).toFixed(1) + '%' 
                  : '0%'}
              </span>
            </div>
          </div>

          <!-- Chart Area -->
          <div class="chart-section glass">
            <div class="chart-section-header">
              <h3>Ruch w czasie</h3>
              <div class="filter-controls">
                <select id="time-range-select" bind:value={timeRange} onchange={loadDashboardData} aria-label="Przedział czasowy">
                  <option value="24h">24h</option>
                  <option value="7d">7 dni</option>
                  <option value="30d">30 dni</option>
                </select>
                <button class="secondary btn-icon-compact" onclick={loadDashboardData} disabled={isDashboardLoading} aria-label="Odśwież wykres">
                  <RefreshCw class={isDashboardLoading ? 'spin' : ''} size={14} />
                </button>
              </div>
            </div>
            {#if isDashboardLoading}
              <div class="loading-state">
                <RefreshCw class="spin" size={24} />
              </div>
            {:else if dashboardStats.requestsPerDay.length === 0}
              <div class="empty-state">
                <Info size={24} />
                <p>Brak danych analitycznych dla wybranego okresu.</p>
              </div>
            {:else}
              <div
                class="svg-chart-wrapper"
                style="--chart-plot-height: {CHART_PLOT_HEIGHT}px; max-width: {chartDisplayMaxWidth}px"
              >
                <div class="chart-y-axis-container">
                  <div class="chart-y-axis" aria-hidden="true">
                    {#each chartYTicks as tick, i}
                      <span class="chart-axis-label" style="top: {(i / 3) * 100}%">
                        {formatCompact(tick)}
                      </span>
                    {/each}
                  </div>
                </div>
                <div class="chart-main-container">
                  <svg
                    viewBox="0 0 {chartViewWidth} {chartLayout.plotHeight}"
                    preserveAspectRatio="none"
                    class="svg-chart bar-chart chart-plot"
                  >
                    <defs>
                      <linearGradient id="allowedGrad" x1="0" y1="0" x2="0" y2="1">
                        <stop offset="0%" stop-color="#34d399" />
                        <stop offset="100%" stop-color="#059669" />
                      </linearGradient>
                      <linearGradient id="blockedGrad" x1="0" y1="0" x2="0" y2="1">
                        <stop offset="0%" stop-color="#f472b6" />
                        <stop offset="100%" stop-color="#db2777" />
                      </linearGradient>
                    </defs>

                    <!-- Linie pomocnicze siatki (oś Y) -->
                    {#each chartYTicks as tick, i}
                      {@const y = chartLayout.pad.top + (i / 3) * (chartLayout.plotHeight - chartLayout.pad.top - chartLayout.pad.bottom)}
                      <line
                        x1="0"
                        y1={y}
                        x2={chartViewWidth}
                        y2={y}
                        stroke="rgba(255,255,255,0.05)"
                      />
                    {/each}

                    <!-- Słupki danych -->
                    {#if dashboardStats.requestsPerDay.length > 0}
                      {@const chartW = chartViewWidth - chartLayout.pad.left - chartLayout.pad.right}
                      {@const chartH = chartLayout.plotHeight - chartLayout.pad.top - chartLayout.pad.bottom}
                      {@const step = chartW / dashboardStats.requestsPerDay.length}
                      {@const barW = Math.max(4, Math.min(step * CHART_BAR_RATIO, CHART_SLOT_VB * CHART_BAR_RATIO))}
                      {@const gap = step - barW}

                      {#each dashboardStats.requestsPerDay as day, idx}
                        {@const x = chartLayout.pad.left + idx * step + gap / 2}
                        {@const allowedCount = day.allowedCount || 0}
                        {@const blockedCount = day.blockedCount || 0}
                        {@const allowedH = (allowedCount / maxChartValue) * chartH}
                        {@const blockedH = (blockedCount / maxChartValue) * chartH}
                        {@const totalH = allowedH + blockedH}
                        {@const isHovered = activeChartTooltip?.day.day === day.day}

                        <!-- Podświetlenie kolumny przy hoverze -->
                        {#if isHovered}
                          <rect
                            x={chartLayout.pad.left + idx * step}
                            y={chartLayout.pad.top - 4}
                            width={step}
                            height={chartH + 8}
                            fill="rgba(255, 255, 255, 0.03)"
                            rx="4"
                          />
                        {/if}

                        <!-- Skumulowane segmenty słupków -->
                        {#if totalH > 0}
                          <!-- Segment Dozwolone (Dół) -->
                          {#if allowedH > 0}
                            <rect
                              x={x}
                              y={chartLayout.pad.top + chartH - allowedH}
                              width={barW}
                              height={allowedH}
                              fill="url(#allowedGrad)"
                              rx={blockedH > 0 ? 0 : 3}
                            />
                          {/if}

                          <!-- Segment Zablokowane (Góra) -->
                          {#if blockedH > 0}
                            <rect
                              x={x}
                              y={chartLayout.pad.top + chartH - totalH}
                              width={barW}
                              height={blockedH}
                              fill="url(#blockedGrad)"
                              rx="3"
                            />
                          {/if}
                        {/if}

                        <!-- Niewidzialny obszar do łatwego najechania myszką -->
                        <rect
                          x={chartLayout.pad.left + idx * step}
                          y={chartLayout.pad.top}
                          width={step}
                          height={chartH}
                          fill="transparent"
                          style="cursor: pointer;"
                          onmouseenter={(e) => {
                            const rect = e.currentTarget.getBoundingClientRect();
                            activeChartTooltip = { day, rect };
                          }}
                          onmouseleave={() => {
                            activeChartTooltip = null;
                          }}
                        />
                      {/each}
                    {/if}
                  </svg>
                  <div class="chart-x-axis">
                    {#each dashboardStats.requestsPerDay as day, idx}
                      {@const stepPct = 100 / dashboardStats.requestsPerDay.length}
                      {@const leftPct = (idx + 0.5) * stepPct}
                      <span class="chart-axis-label chart-x-label" style="left: {leftPct}%">
                        {formatDayLabel(day.day)}
                      </span>
                    {/each}
                  </div>
                </div>
              </div>
              <div class="chart-legend">
                <span class="legend-item"><span class="legend-dot green"></span> Dozwolone</span>
                <span class="legend-item"><span class="legend-dot pink"></span> Zablokowane</span>
              </div>
            {/if}
          </div>

          <div class="stats-grids">
            <div class="stats-panel glass geo-map-panel">
              <h3>Żądania według kraju</h3>
              {#if isDashboardLoading}
                <div class="loading-state"><RefreshCw class="spin" size={20} /></div>
              {:else}
                <PangolinWorldMap
                  countries={sortedCountries}
                  totalRequests={dashboardStats.totalRequests}
                  bind:hoveredCode={hoveredMapCode}
                />
              {/if}
            </div>

            <div class="stats-panel glass top-countries-panel">
              <h3>Top kraje</h3>
              {#if isDashboardLoading}
                <div class="loading-state"><RefreshCw class="spin" size={20} /></div>
              {:else if sortedCountries.length === 0}
                <p class="empty-msg">Brak danych geograficznych.</p>
              {:else}
                <div class="top-countries-header">
                  <span>Kraj</span>
                  <span>Łącznie</span>
                  <span>%</span>
                </div>
                <div class="top-countries-list">
                  {#each sortedCountries as geo}
                    {@const intensity = trafficIntensity(geo.count || 0, maxCountryCount)}
                    {@const pct = dashboardStats.totalRequests > 0
                      ? ((geo.count / dashboardStats.totalRequests) * 100).toFixed(0)
                      : '0'}
                    {@const isHovered = hoveredCountryRow === geo.code || hoveredMapCode === geo.code?.toUpperCase()}
                    <div
                      class="country-row"
                      class:hovered={isHovered}
                      style="background: {trafficBarGradient(intensity)}"
                      title={getCountryTooltip(geo)}
                      onmouseenter={(e) => {
                        hoveredCountryRow = geo.code;
                        hoveredMapCode = geo.code?.toUpperCase() || null;
                        const rect = e.currentTarget.getBoundingClientRect();
                        activeTooltip = { geo, pct, rect };
                      }}
                      onmouseleave={() => {
                        hoveredCountryRow = null;
                        hoveredMapCode = null;
                        activeTooltip = null;
                      }}
                      role="listitem"
                    >
                      <div class="country-label">
                        <span class="country-code">{geo.code || '??'}</span>
                        <span class="country-name">{getCountryName(geo)}</span>
                      </div>
                      <span class="country-total mono-stats">{formatCompact(geo.count || 0)}</span>
                      <span class="country-pct mono-stats">{pct}%</span>
                    </div>
                  {/each}
                </div>
              {/if}
            </div>
          </div>
          {#if activeTooltip}
            {@const showOnLeft = activeTooltip.rect.right + 250 > window.innerWidth}
            <div
              class="country-tooltip-fixed"
              style="
                left: {showOnLeft ? (activeTooltip.rect.left - 250) : (activeTooltip.rect.right + 10)}px;
                top: {activeTooltip.rect.top + activeTooltip.rect.height / 2}px;
                transform: translateY(-50%);
              "
            >
              <strong>{getCountryName(activeTooltip.geo)}</strong>
              <span>{formatCompact(activeTooltip.geo.count || 0)} zapytań · {activeTooltip.pct}% ruchu</span>
              <span class="tooltip-detail">
                Dozwolone: {formatCompact(activeTooltip.geo.allowedCount ?? ((activeTooltip.geo.count || 0) - (activeTooltip.geo.blockedCount || 0)))}
                · Zablokowane: {formatCompact(activeTooltip.geo.blockedCount ?? 0)}
              </span>
            </div>
          {/if}

          {#if activeChartTooltip}
            {@const total = (activeChartTooltip.day.allowedCount || 0) + (activeChartTooltip.day.blockedCount || 0)}
            {@const blockedPct = total > 0 ? ((activeChartTooltip.day.blockedCount / total) * 100).toFixed(1) : '0.0'}
            {@const showOnLeft = activeChartTooltip.rect.right + 230 > window.innerWidth}
            <div
              class="chart-tooltip-fixed"
              style="
                left: {showOnLeft ? (activeChartTooltip.rect.left - 240) : (activeChartTooltip.rect.right + 10)}px;
                top: {activeChartTooltip.rect.top + activeChartTooltip.rect.height / 2}px;
                transform: translateY(-50%);
              "
            >
              <div class="tooltip-header">{formatDayLabel(activeChartTooltip.day.day)}</div>
              <div class="tooltip-body">
                <div class="tooltip-row">
                  <span class="dot total"></span>
                  <span class="label">Łącznie:</span>
                  <span class="val">{formatCompact(total)}</span>
                </div>
                <div class="tooltip-row">
                  <span class="dot green"></span>
                  <span class="label">Dozwolone:</span>
                  <span class="val">{formatCompact(activeChartTooltip.day.allowedCount || 0)}</span>
                </div>
                <div class="tooltip-row">
                  <span class="dot pink"></span>
                  <span class="label">Zablokowane:</span>
                  <span class="val text-pink">{formatCompact(activeChartTooltip.day.blockedCount || 0)} ({blockedPct}%)</span>
                </div>
              </div>
            </div>
          {/if}
        </div>
      {/if}

      <!-- 2. AUDIT LOGS VIEW -->
      {#if activeSubTab === 'logs'}
        <div class="logs-view">
          <div class="filters-bar glass">
            <div class="filter-col">
              <label for="log-status-select">Status</label>
              <select id="log-status-select" bind:value={logFilters.action} onchange={loadLogsData}>
                <option value="">Wszystkie</option>
                <option value="allowed">Dozwolone</option>
                <option value="blocked">Zablokowane</option>
              </select>
            </div>
            <div class="filter-col">
              <label for="log-host-select">Host</label>
              <select id="log-host-select" bind:value={logFilters.host} onchange={loadLogsData}>
                <option value="">Wszystkie</option>
                {#each uniqueFilters.hosts as h}
                  <option value={h}>{h}</option>
                {/each}
              </select>
            </div>
            <div class="filter-col">
              <label for="log-method-select">Metoda</label>
              <select id="log-method-select" bind:value={logFilters.method} onchange={loadLogsData}>
                <option value="">Wszystkie</option>
                <option value="GET">GET</option>
                <option value="POST">POST</option>
                <option value="PUT">PUT</option>
                <option value="DELETE">DELETE</option>
                <option value="PATCH">PATCH</option>
              </select>
            </div>
            <div class="filter-col flex-grow">
              <label for="log-actor-input">Użytkownik / Aktor</label>
              <input id="log-actor-input" type="text" placeholder="Filtruj po e-mailu/ID" bind:value={logFilters.actor} onkeydown={(e) => e.key === 'Enter' && loadLogsData()} />
            </div>
            <button class="primary" onclick={loadLogsData}>Filtruj</button>
            <button class="secondary" onclick={() => {
              logFilters = { limit: 50, offset: 0, action: '', method: '', host: '', path: '', actor: '' };
              loadLogsData();
            }}>Reset</button>
          </div>

          <div class="logs-table-wrapper glass">
            <table class="telemetry-table">
              <thead>
                <tr>
                  <th>Czas</th>
                  <th>Metoda</th>
                  <th>Zasób / Host</th>
                  <th>IP / Kraj</th>
                  <th>Aktor</th>
                  <th>Status</th>
                  <th>Powód</th>
                  <th></th>
                </tr>
              </thead>
              <tbody>
                {#if isLogsLoading}
                  <tr>
                    <td colspan="8" class="text-center py-6">
                      <RefreshCw class="spin" size={24} />
                      <p>Wczytywanie logów audytu...</p>
                    </td>
                  </tr>
                {:else if logsList.length === 0}
                  <tr>
                    <td colspan="8" class="text-center py-6 text-muted">Brak wpisów logów pasujących do filtrów.</td>
                  </tr>
                {:else}
                  {#each logsList as log}
                    <tr class="log-row" onclick={() => selectedLogDetail = log}>
                      <td class="mono-stats font-xs">{formatTime(log.timestamp)}</td>
                      <td>
                        <span class="method-tag {log.method?.toLowerCase()}">{log.method}</span>
                      </td>
                      <td class="truncate-cell" title="{log.host}{log.path}">{log.host}{log.path}</td>
                      <td>
                        <div class="ip-block">
                          <span>{log.ip}</span>
                          {#if log.location}
                            <span class="country-badge">{log.location}</span>
                          {/if}
                        </div>
                      </td>
                      <td class="truncate-cell" title={log.actor}>{log.actor || 'Anonimowy'}</td>
                      <td>
                        <span class="status-badge {log.action ? 'allowed' : 'blocked'}">
                          {log.action ? 'Dozwolony' : 'Zablokowany'}
                        </span>
                      </td>
                      <td class="truncate-cell text-muted" title={log.reason}>{log.reason || '-'}</td>
                      <td>
                        <ChevronRight size={14} />
                      </td>
                    </tr>
                  {/each}
                {/if}
              </tbody>
            </table>
          </div>

          <!-- Pagination -->
          <div class="pagination-bar">
            <span class="total-count">Łącznie: <b>{logsPagination.total}</b> logów</span>
            <div class="pagination-controls">
              <button class="secondary btn-icon-compact" disabled={logFilters.offset === 0} onclick={() => {
                logFilters.offset = Math.max(0, logFilters.offset - logFilters.limit);
                loadLogsData();
              }}>
                <ChevronLeft size={16} />
              </button>
              <span class="page-indicator">Offset: {logFilters.offset} - {logFilters.offset + logsList.length}</span>
              <button class="secondary btn-icon-compact" disabled={logFilters.offset + logFilters.limit >= logsPagination.total} onclick={() => {
                logFilters.offset += logFilters.limit;
                loadLogsData();
              }}>
                <ChevronRight size={16} />
              </button>
            </div>
          </div>
        </div>

        <!-- Details Modal -->
        {#if selectedLogDetail}
          <div class="modal-backdrop" onclick={() => selectedLogDetail = null}>
            <div class="modal-content log-details glass" onclick={(e) => e.stopPropagation()}>
              <header class="modal-header">
                <h3>Szczegóły zapytania audytu</h3>
                <button class="btn-icon-compact secondary" onclick={() => selectedLogDetail = null}><X size={16} /></button>
              </header>
              <div class="modal-body scrollable">
                <div class="grid-details">
                  <div class="detail-row">
                    <span class="lbl">Czas zapytania:</span>
                    <span class="val mono-stats">{formatTime(selectedLogDetail.timestamp)}</span>
                  </div>
                  <div class="detail-row">
                    <span class="lbl">ID transakcji:</span>
                    <span class="val mono-stats">{selectedLogDetail.id}</span>
                  </div>
                  <div class="detail-row">
                    <span class="lbl">Pełny URL:</span>
                    <span class="val text-orange truncate-text">{selectedLogDetail.scheme}://{selectedLogDetail.host}{selectedLogDetail.path}{selectedLogDetail.query || ''}</span>
                  </div>
                  <div class="detail-row">
                    <span class="lbl">Metoda:</span>
                    <span class="val"><span class="method-tag {selectedLogDetail.method?.toLowerCase()}">{selectedLogDetail.method}</span></span>
                  </div>
                  <div class="detail-row">
                    <span class="lbl">Autoryzacja:</span>
                    <span class="val">
                      <span class="status-badge {selectedLogDetail.action ? 'allowed' : 'blocked'}">
                        {selectedLogDetail.action ? 'ZAAKCEPTOWANE' : 'ZABLOKOWANE'}
                      </span>
                    </span>
                  </div>
                  <div class="detail-row">
                    <span class="lbl">Powód decyzji:</span>
                    <span class="val text-orange">{selectedLogDetail.reason || 'Brak dodatkowego uzasadnienia'}</span>
                  </div>
                  <div class="detail-row">
                    <span class="lbl">Aktor / Użytkownik:</span>
                    <span class="val">{selectedLogDetail.actor || 'Anonimowy Gość'} (Typ: {selectedLogDetail.actorType || 'N/A'}, ID: {selectedLogDetail.actorId || 'N/A'})</span>
                  </div>
                  <div class="detail-row">
                    <span class="lbl">Adres IP źródłowy:</span>
                    <span class="val mono-stats">{selectedLogDetail.ip} (Kraj: {selectedLogDetail.location || 'Nieznany'})</span>
                  </div>
                  <div class="detail-row">
                    <span class="lbl">Zasób powiązany:</span>
                    <span class="val">ID zasobu publicznego: {selectedLogDetail.resourceId || 'Brak'} | ID zasobu prywatnego: {selectedLogDetail.siteResourceId || 'Brak'}</span>
                  </div>
                  <div class="detail-row">
                    <span class="lbl">User Agent:</span>
                    <span class="val font-xs">{selectedLogDetail.userAgent || '-'}</span>
                  </div>
                </div>

                {#if selectedLogDetail.headers}
                  <div class="json-section">
                    <h4>Nagłówki HTTP (Headers)</h4>
                    <pre class="json-block font-xs">{typeof selectedLogDetail.headers === 'string' ? selectedLogDetail.headers : JSON.stringify(selectedLogDetail.headers, null, 2)}</pre>
                  </div>
                {/if}

                {#if selectedLogDetail.tls}
                  <div class="json-section">
                    <h4>Informacje o sesji TLS</h4>
                    <pre class="json-block font-xs">{typeof selectedLogDetail.tls === 'string' ? selectedLogDetail.tls : JSON.stringify(selectedLogDetail.tls, null, 2)}</pre>
                  </div>
                {/if}
              </div>
            </div>
          </div>
        {/if}
      {/if}

      <!-- 3. TUNNELS & RESOURCES VIEW -->
      {#if activeSubTab === 'resources'}
        <div class="resources-view">
          <!-- Sub-tabs header -->
          <div class="section-tabs">
            <h3 class="section-title">Konfiguracja Tuneli i Zasobów</h3>
          </div>

          <!-- Section 1: Sites (WireGuard Tunnels) -->
          <div class="mgmt-section glass">
            <header class="section-header">
              <div class="sec-title">
                <Radio class="text-orange" size={18} />
                <h3>Tunele WireGuard / Exit Nodes (Sites)</h3>
              </div>
              <button class="primary btn-sm" onclick={() => showCreateSiteModal = true}>
                <Plus size={16} /> Nowy Tunel
              </button>
            </header>

            <table class="telemetry-table">
              <thead>
                <tr>
                  <th>Etykieta (Nazwa)</th>
                  <th>Nice ID</th>
                  <th>Typ</th>
                  <th>Podsieć / Subnet</th>
                  <th>Adres tunelu</th>
                  <th>Klucz publiczny</th>
                  <th>Operacje</th>
                </tr>
              </thead>
              <tbody>
                {#if isSitesLoading}
                  <tr><td colspan="7" class="text-center py-4"><RefreshCw class="spin" size={16} /> Wczytywanie...</td></tr>
                {:else if sitesList.length === 0}
                  <tr><td colspan="7" class="text-center py-4 text-muted">Brak skonfigurowanych tuneli.</td></tr>
                {:else}
                  {#each sitesList as site}
                    <tr>
                      <td class="font-semibold">{site.name}</td>
                      <td class="mono-stats">{site.niceId || '-'}</td>
                      <td><span class="type-tag">{site.type}</span></td>
                      <td class="mono-stats">{site.subnet || '-'}</td>
                      <td class="mono-stats">{site.address || '-'}</td>
                      <td class="mono-stats font-xs truncate-cell" title={site.pubKey}>{site.pubKey || '-'}</td>
                      <td>
                        <button class="secondary btn-icon-compact text-red" onclick={() => handleDeleteSite(site.siteId)}>
                          <Trash2 size={14} />
                        </button>
                      </td>
                    </tr>
                  {/each}
                {/if}
              </tbody>
            </table>
          </div>

          <!-- Section 2: Private Resources (Site Resources) -->
          <div class="mgmt-section glass">
            <header class="section-header">
              <div class="sec-title">
                <Shield class="text-orange" size={18} />
                <h3>Zasoby Prywatne (Site Resources)</h3>
              </div>
              <button class="primary btn-sm" onclick={() => { resetPrivResForm(); showCreatePrivResModal = true; }}>
                <Plus size={16} /> Nowy Zasób Prywatny
              </button>
            </header>

            <table class="telemetry-table">
              <thead>
                <tr>
                  <th>Nazwa</th>
                  <th>Nice ID</th>
                  <th>Tryb</th>
                  <th>Cel (Destination)</th>
                  <th>Porty TCP</th>
                  <th>Porty UDP</th>
                  <th>Status</th>
                  <th>Operacje</th>
                </tr>
              </thead>
              <tbody>
                {#if isPrivResourcesLoading}
                  <tr><td colspan="8" class="text-center py-4"><RefreshCw class="spin" size={16} /> Wczytywanie...</td></tr>
                {:else if privResourcesList.length === 0}
                  <tr><td colspan="8" class="text-center py-4 text-muted">Brak skonfigurowanych zasobów prywatnych.</td></tr>
                {:else}
                  {#each privResourcesList as res}
                    <tr>
                      <td class="font-semibold">{res.name}</td>
                      <td class="mono-stats">{res.niceId || '-'}</td>
                      <td><span class="mode-tag">{res.mode}</span></td>
                      <td class="mono-stats">{res.destination}{res.destinationPort ? `:${res.destinationPort}` : ''}</td>
                      <td class="mono-stats">{res.tcpPortRangeString || '*'}</td>
                      <td class="mono-stats">{res.udpPortRangeString || '*'}</td>
                      <td>
                        <span class="status-dot {res.enabled ? 'nominal' : 'inactive'}"></span>
                        {res.enabled ? 'Aktywny' : 'Wyłączony'}
                      </td>
                      <td>
                        <div class="flex-actions">
                          <button class="secondary btn-icon-compact" onclick={() => editPrivRes(res)}>
                            <Edit2 size={14} />
                          </button>
                          <button class="secondary btn-icon-compact text-red" onclick={() => handleDeletePrivRes(res.siteResourceId)}>
                            <Trash2 size={14} />
                          </button>
                        </div>
                      </td>
                    </tr>
                  {/each}
                {/if}
              </tbody>
            </table>
          </div>

          <!-- Section 3: Public Resources -->
          <div class="mgmt-section glass">
            <header class="section-header">
              <div class="sec-title">
                <Link class="text-orange" size={18} />
                <h3>Zasoby Publiczne (Chronione Domeny HTTP / TCP / UDP)</h3>
              </div>
              <button class="primary btn-sm" onclick={() => { resetPubResForm(); showCreatePubResModal = true; }}>
                <Plus size={16} /> Nowy Zasób Publiczny
              </button>
            </header>

            <table class="telemetry-table">
              <thead>
                <tr>
                  <th>Nazwa zasobu</th>
                  <th>Metoda / Tryb</th>
                  <th>Domeny / Subdomena</th>
                  <th>Port Proxy</th>
                  <th>Sticky Session</th>
                  <th>Operacje</th>
                </tr>
              </thead>
              <tbody>
                {#if isPubResourcesLoading}
                  <tr><td colspan="6" class="text-center py-4"><RefreshCw class="spin" size={16} /> Wczytywanie...</td></tr>
                {:else if pubResourcesList.length === 0}
                  <tr><td colspan="6" class="text-center py-4 text-muted">Brak skonfigurowanych domen publicznych.</td></tr>
                {:else}
                  {#each pubResourcesList as res}
                    <tr>
                      <td class="font-semibold">{res.name}</td>
                      <td><span class="mode-tag {res.mode}">{res.mode}</span></td>
                      <td>
                        {#if res.fullDomain}
                          <button 
                            class="link-btn text-orange hover-underline" 
                            onclick={() => handleOpenUrl((res.ssl ? 'https://' : 'http://') + res.fullDomain)}
                            title="Kliknij, aby otworzyć w przeglądarce"
                          >
                            {res.fullDomain}
                          </button>
                        {:else}
                          <span class="text-muted">-</span>
                        {/if}
                      </td>
                      <td class="mono-stats">{res.proxyPort || '-'}</td>
                      <td>{res.stickySession ? 'Tak' : 'Nie'}</td>
                      <td>
                        <div class="flex-actions">
                          <button class="secondary btn-icon-compact" onclick={() => editPubRes(res)}>
                            <Edit2 size={14} />
                          </button>
                          <button class="secondary btn-icon-compact text-red" onclick={() => handleDeletePubRes(res.resourceId)}>
                            <Trash2 size={14} />
                          </button>
                        </div>
                      </td>
                    </tr>
                  {/each}
                {/if}
              </tbody>
            </table>
          </div>
        </div>

        <!-- 3A. CREATE SITE MODAL -->
        {#if showCreateSiteModal}
          <div class="modal-backdrop" onclick={() => showCreateSiteModal = false}>
            <div class="modal-content glass dialog-sm" onclick={(e) => e.stopPropagation()}>
              <header class="modal-header">
                <h3>Utwórz Nowy Tunel (Site)</h3>
                <button class="btn-icon-compact secondary" onclick={() => showCreateSiteModal = false}><X size={16} /></button>
              </header>
              <div class="modal-body">
                <div class="form-group">
                  <label for="site-name">Nazwa tunelu *</label>
                  <input id="site-name" type="text" placeholder="Np. Serwer-Produkcja" bind:value={newSiteData.name} />
                </div>
                <div class="form-group">
                  <label for="site-type">Typ połączenia *</label>
                  <select id="site-type" bind:value={newSiteData.type}>
                    <option value="wireguard">WireGuard Standard</option>
                    <option value="newt">Newt Zero-Config</option>
                    <option value="local">Local Agent</option>
                  </select>
                </div>
                <div class="form-group">
                  <label for="site-nice">Unikalny identyfikator (Nice ID)</label>
                  <input id="site-nice" type="text" placeholder="serwer-prod" bind:value={newSiteData.niceId} />
                </div>
                <div class="form-group">
                  <label for="site-subnet">Adresacja / Podsieć IP (Subnet)</label>
                  <input id="site-subnet" type="text" placeholder="10.8.0.0/24" bind:value={newSiteData.subnet} />
                </div>
              </div>
              <footer class="modal-footer">
                <button class="secondary" onclick={() => showCreateSiteModal = false}>Anuluj</button>
                <button class="primary" onclick={handleCreateSite} disabled={!newSiteData.name}>Utwórz Tunel</button>
              </footer>
            </div>
          </div>
        {/if}

        <!-- 3B. CREATE/EDIT PRIV RESOURCE MODAL -->
        {#if showCreatePrivResModal}
          <div class="modal-backdrop" onclick={() => showCreatePrivResModal = false}>
            <div class="modal-content glass dialog-md" onclick={(e) => e.stopPropagation()}>
              <header class="modal-header">
                <h3>{isEditingPrivRes ? 'Edytuj' : 'Utwórz'} Zasób Prywatny</h3>
                <button class="btn-icon-compact secondary" onclick={() => showCreatePrivResModal = false}><X size={16} /></button>
              </header>
              <div class="modal-body scrollable max-h-400">
                <div class="form-grid-2">
                  <div class="form-group">
                    <label for="priv-res-name">Nazwa zasobu *</label>
                    <input id="priv-res-name" type="text" placeholder="Np. Baza Danych PG" bind:value={privResForm.name} />
                  </div>
                  <div class="form-group">
                    <label for="priv-res-nice">Nice ID</label>
                    <input id="priv-res-nice" type="text" placeholder="nice-db-id" bind:value={privResForm.niceId} />
                  </div>
                  <div class="form-group">
                    <label for="priv-res-mode">Tryb zasobu *</label>
                    <select id="priv-res-mode" bind:value={privResForm.mode}>
                      <option value="host">Pojedynczy Host (TCP/UDP)</option>
                      <option value="cidr">Podsieć CIDR (TCP/UDP)</option>
                      <option value="http">Serwer WWW (HTTP/HTTPS)</option>
                      <option value="ssh">Połączenie konsoli (SSH)</option>
                    </select>
                  </div>
                  <div class="form-group">
                    <label for="priv-res-dest">Adres docelowy (IP / Domena) *</label>
                    <input id="priv-res-dest" type="text" placeholder="192.168.1.50" bind:value={privResForm.destination} />
                  </div>
                </div>

                {#if privResForm.mode === 'http' || privResForm.mode === 'ssh'}
                  <div class="form-grid-2 mt-4">
                    <div class="form-group">
                      <label for="priv-res-port">Port docelowy *</label>
                      <input id="priv-res-port" type="number" bind:value={privResForm.destinationPort} />
                    </div>
                    {#if privResForm.mode === 'http'}
                      <div class="form-group">
                        <label for="priv-res-scheme">Protokół (Scheme)</label>
                        <select id="priv-res-scheme" bind:value={privResForm.scheme}>
                          <option value="http">HTTP</option>
                          <option value="https">HTTPS</option>
                        </select>
                      </div>
                      <div class="checkbox-group">
                        <input id="priv-res-ssl" type="checkbox" bind:checked={privResForm.ssl} />
                        <label for="priv-res-ssl">Ignoruj błędy SSL backendu</label>
                      </div>
                    {/if}
                  </div>
                {:else}
                  <div class="form-grid-2 mt-4">
                    <div class="form-group">
                      <label for="priv-res-tcp">Porty TCP (Np. 80,443,8000-9000)</label>
                      <input id="priv-res-tcp" type="text" placeholder="*" bind:value={privResForm.tcpPortRangeString} />
                    </div>
                    <div class="form-group">
                      <label for="priv-res-udp">Porty UDP</label>
                      <input id="priv-res-udp" type="text" placeholder="*" bind:value={privResForm.udpPortRangeString} />
                    </div>
                  </div>
                  {#if privResForm.mode === 'cidr'}
                    <div class="checkbox-group mt-2">
                      <input id="priv-res-icmp" type="checkbox" bind:checked={privResForm.disableIcmp} />
                      <label for="priv-res-icmp">Zablokuj ruch ICMP (Ping)</label>
                    </div>
                  {/if}
                {/if}

                <!-- Associations (Tunnels) -->
                <div class="form-group mt-4">
                  <span class="group-label">Przypisz do tuneli (Exit Nodes) *</span>
                  <div class="checkbox-list">
                    {#each sitesList as site}
                      <label class="check-item">
                        <input type="checkbox" value={site.siteId} 
                          checked={privResForm.siteIds.includes(site.siteId)}
                          onchange={(e) => {
                            const checked = (e.target as HTMLInputElement).checked;
                            if (checked) privResForm.siteIds = [...privResForm.siteIds, site.siteId];
                            else privResForm.siteIds = privResForm.siteIds.filter(id => id !== site.siteId);
                          }}
                        />
                        <span>{site.name}</span>
                      </label>
                    {/each}
                  </div>
                </div>
              </div>
              <footer class="modal-footer">
                <button class="secondary" onclick={() => showCreatePrivResModal = false}>Anuluj</button>
                <button class="primary" onclick={handleSavePrivRes} disabled={!privResForm.name || !privResForm.destination}>
                  Zapisz Zasób
                </button>
              </footer>
            </div>
          </div>
        {/if}

        <!-- 3C. CREATE/EDIT PUBLIC RESOURCE MODAL -->
        {#if showCreatePubResModal}
          <div class="modal-backdrop" onclick={() => showCreatePubResModal = false}>
            <div class="modal-content glass dialog-md" onclick={(e) => e.stopPropagation()}>
              <header class="modal-header">
                <h3>{isEditingPubRes ? 'Edytuj' : 'Utwórz'} Zasób Publiczny</h3>
                <button class="btn-icon-compact secondary" onclick={() => showCreatePubResModal = false}><X size={16} /></button>
              </header>
              <div class="modal-body scrollable max-h-400">
                <div class="form-grid-2">
                  <div class="form-group">
                    <label for="pub-res-name">Nazwa zasobu *</label>
                    <input id="pub-res-name" type="text" placeholder="Np. Panel Admina HTTP" bind:value={pubResForm.name} />
                  </div>
                  <div class="form-group">
                    <label for="pub-res-mode">Tryb zasobu *</label>
                    <select id="pub-res-mode" bind:value={pubResForm.mode}>
                      <option value="http">HTTP Web App</option>
                      <option value="ssh">Zdalny terminal SSH</option>
                      <option value="rdp">Pulpit zdalny RDP</option>
                      <option value="vnc">Pulpit zdalny VNC</option>
                      <option value="tcp">Czysty TCP Tunnel</option>
                      <option value="udp">Czysty UDP Tunnel</option>
                    </select>
                  </div>
                </div>

                {#if pubResForm.mode === 'tcp' || pubResForm.mode === 'udp'}
                  <div class="form-group mt-4">
                    <label for="pub-res-proxyport">Port Proxy w Pangolin *</label>
                    <input id="pub-res-proxyport" type="number" placeholder="8080" bind:value={pubResForm.proxyPort} />
                  </div>
                {:else}
                  <div class="form-grid-2 mt-4">
                    <div class="form-group">
                      <label for="pub-res-domain">Domena główna *</label>
                      <select id="pub-res-domain" bind:value={pubResForm.domainId}>
                        <option value="">Wybierz domenę...</option>
                        {#each domainsList as dom}
                          <option value={dom.domainId}>{dom.baseDomain}</option>
                        {/each}
                      </select>
                    </div>
                    <div class="form-group">
                      <label for="pub-res-sub">Subdomena</label>
                      <input id="pub-res-sub" type="text" placeholder="admin" bind:value={pubResForm.subdomain} />
                    </div>
                  </div>

                  <div class="form-grid-2 mt-4">
                    <div class="checkbox-group">
                      <input id="pub-res-sticky" type="checkbox" bind:checked={pubResForm.stickySession} />
                      <label for="pub-res-sticky">Lepkie sesje (Sticky Session)</label>
                    </div>
                    <div class="form-group">
                      <label for="pub-res-postauth">Post-Auth Path Redirect</label>
                      <input id="pub-res-postauth" type="text" placeholder="/dashboard" bind:value={pubResForm.postAuthPath} />
                    </div>
                  </div>
                {/if}

                <!-- Forward Target Configuration -->
                <h4 class="mt-4 border-bottom pb-2 font-semibold text-orange" style="font-size: 0.95rem;">Cel przekierowania (Forward Target)</h4>
                <div class="form-grid-3 mt-2">
                  <div class="form-group">
                    <label for="pub-res-target-site">Tunel Wyjściowy (Site) *</label>
                    <select id="pub-res-target-site" bind:value={pubResForm.targetSiteId}>
                      <option value="">Wybierz tunel...</option>
                      {#each sitesList as site}
                        <option value={site.siteId}>{site.name} ({site.type})</option>
                      {/each}
                    </select>
                  </div>
                  <div class="form-group">
                    <label for="pub-res-target-ip">Adres IP / Host docelowy *</label>
                    <input id="pub-res-target-ip" type="text" placeholder="192.168.1.100 lub nazwa-hosta" bind:value={pubResForm.targetIp} />
                  </div>
                  <div class="form-group">
                    <label for="pub-res-target-port">Port docelowy *</label>
                    <input id="pub-res-target-port" type="number" placeholder="80" bind:value={pubResForm.targetPort} min="1" max="65535" />
                  </div>
                </div>
              </div>
              <footer class="modal-footer">
                <button class="secondary" onclick={() => showCreatePubResModal = false}>Anuluj</button>
                <button class="primary" onclick={handleSavePubRes} disabled={isPubResFormInvalid}>
                  Zapisz Zasób
                </button>
              </footer>
            </div>
          </div>
        {/if}
      {/if}

      <!-- 4. ACCESS CONTROL VIEW -->
      {#if activeSubTab === 'access'}
        <div class="access-view">
          <div class="access-tab-nav">
            <button class="tab-btn" class:active={activeAccessSubTab === 'users'} onclick={() => activeAccessSubTab = 'users'}>
              <Users size={14} /> Użytkownicy
            </button>
            <button class="tab-btn" class:active={activeAccessSubTab === 'roles'} onclick={() => activeAccessSubTab = 'roles'}>
              <Shield size={14} /> Role
            </button>
            <button class="tab-btn" class:active={activeAccessSubTab === 'idps'} onclick={() => activeAccessSubTab = 'idps'}>
              <Key size={14} /> Identity Providers
            </button>
            <button class="tab-btn" class:active={activeAccessSubTab === 'invitations'} onclick={() => activeAccessSubTab = 'invitations'}>
              <UserPlus size={14} /> Zaproszenia
            </button>
          </div>

          <div class="access-content glass">
            <!-- 4A. Users Sub-tab -->
            {#if activeAccessSubTab === 'users'}
              <header class="section-header">
                <h3>Aktywni Użytkownicy Organizacji</h3>
              </header>
              <table class="telemetry-table">
                <thead>
                  <tr>
                    <th>E-mail (Username)</th>
                    <th>ID Użytkownika</th>
                    <th>Nazwisko / Nazwa</th>
                    <th>Uwierzytelnienie 2FA</th>
                    <th>Role</th>
                  </tr>
                </thead>
                <tbody>
                  {#if isAccessLoading}
                    <tr><td colspan="5" class="text-center py-4"><RefreshCw class="spin" size={16} /></td></tr>
                  {:else if usersList.length === 0}
                    <tr><td colspan="5" class="text-center py-4 text-muted">Brak użytkowników.</td></tr>
                  {:else}
                    {#each usersList as u}
                      <tr>
                        <td class="font-semibold">{u.username}</td>
                        <td class="mono-stats font-xs">{u.userId}</td>
                        <td>{u.name || '-'}</td>
                        <td>
                          <span class="status-badge {u.twoFaEnabled ? 'allowed' : 'blocked'}">
                            {u.twoFaEnabled ? 'Włączone' : 'Wyłączone'}
                          </span>
                        </td>
                        <td>
                          <div class="flex-tags">
                            {#each u.roles || [] as r}
                              <span class="tag-badge">{r.name}</span>
                            {/each}
                          </div>
                        </td>
                      </tr>
                    {/each}
                  {/if}
                </tbody>
              </table>
            {/if}

            <!-- 4B. Roles Sub-tab -->
            {#if activeAccessSubTab === 'roles'}
              <header class="section-header">
                <h3>Role Bezpieczeństwa</h3>
                <button class="primary btn-sm" onclick={() => { resetRoleForm(); showCreateRoleModal = true; }}>
                  <Plus size={16} /> Nowa Rola
                </button>
              </header>
              <table class="telemetry-table">
                <thead>
                  <tr>
                    <th>Nazwa roli</th>
                    <th>Opis</th>
                    <th>Weryfikacja urządzeń</th>
                    <th>Zgoda na SSH</th>
                    <th>Uprawnienia Sudo SSH</th>
                    <th>Operacje</th>
                  </tr>
                </thead>
                <tbody>
                  {#if isAccessLoading}
                    <tr><td colspan="6" class="text-center py-4"><RefreshCw class="spin" size={16} /></td></tr>
                  {:else if rolesList.length === 0}
                    <tr><td colspan="6" class="text-center py-4 text-muted">Brak zdefiniowanych ról.</td></tr>
                  {:else}
                    {#each rolesList as r}
                      <tr>
                        <td class="font-semibold text-orange">{r.name}</td>
                        <td>{r.description || '-'}</td>
                        <td>{r.requireDeviceApproval ? 'Wymagana' : 'Wyłączona'}</td>
                        <td>{r.allowSsh ? 'Zezwolono' : 'Zablokowano'}</td>
                        <td><span class="mode-tag">{r.sshSudoMode}</span></td>
                        <td>
                          <div class="flex-actions">
                            <button class="secondary btn-icon-compact" onclick={() => editRole(r)}>
                              <Edit2 size={14} />
                            </button>
                            <button class="secondary btn-icon-compact text-red" onclick={() => handleDeleteRole(r.roleId)}>
                              <Trash2 size={14} />
                            </button>
                          </div>
                        </td>
                      </tr>
                    {/each}
                  {/if}
                </tbody>
              </table>
            {/if}

            <!-- 4C. IDPs Sub-tab -->
            {#if activeAccessSubTab === 'idps'}
              <header class="section-header">
                <h3>Identity Providers (OIDC / OAuth)</h3>
              </header>
              <table class="telemetry-table">
                <thead>
                  <tr>
                    <th>ID dostawcy</th>
                    <th>Nazwa</th>
                    <th>OIDC Issuer</th>
                    <th>Client ID</th>
                  </tr>
                </thead>
                <tbody>
                  {#if isAccessLoading}
                    <tr><td colspan="4" class="text-center py-4"><RefreshCw class="spin" size={16} /></td></tr>
                  {:else if idpsList.length === 0}
                    <tr><td colspan="4" class="text-center py-4 text-muted">Brak dostawców tożsamości OIDC. Logowanie tylko kontami lokalnymi.</td></tr>
                  {:else}
                    {#each idpsList as idp}
                      <tr>
                        <td class="mono-stats">{idp.idpId}</td>
                        <td class="font-semibold">{idp.name}</td>
                        <td class="font-xs">{idp.issuer}</td>
                        <td class="mono-stats font-xs">{idp.clientId}</td>
                      </tr>
                    {/each}
                  {/if}
                </tbody>
              </table>
            {/if}

            <!-- 4D. Invitations Sub-tab -->
            {#if activeAccessSubTab === 'invitations'}
              <header class="section-header">
                <h3>Wysłane Zaproszenia Użytkowników</h3>
                <button class="primary btn-sm" onclick={() => showInviteModal = true}>
                  <Plus size={16} /> Zaproś użytkownika
                </button>
              </header>
              <table class="telemetry-table">
                <thead>
                  <tr>
                    <th>E-mail zaproszonego</th>
                    <th>Kod / Link zaproszenia</th>
                    <th>Wygasa za</th>
                    <th>Operacje</th>
                  </tr>
                </thead>
                <tbody>
                  {#if isAccessLoading}
                    <tr><td colspan="4" class="text-center py-4"><RefreshCw class="spin" size={16} /></td></tr>
                  {:else if invitationsList.length === 0}
                    <tr><td colspan="4" class="text-center py-4 text-muted">Brak otwartych zaproszeń.</td></tr>
                  {:else}
                    {#each invitationsList as inv}
                      <tr>
                        <td class="font-semibold">{inv.email}</td>
                        <td class="mono-stats font-xs select-all">{inv.code || '-'}</td>
                        <td>{inv.expiresAt ? new Date(inv.expiresAt).toLocaleString() : '-'}</td>
                        <td>
                          <button class="secondary btn-icon-compact text-red" onclick={() => handleCancelInvite(inv.inviteId)}>
                            <Trash2 size={14} />
                          </button>
                        </td>
                      </tr>
                    {/each}
                  {/if}
                </tbody>
              </table>
            {/if}
          </div>
        </div>

        <!-- 4E. INVITE USER MODAL -->
        {#if showInviteModal}
          <div class="modal-backdrop" onclick={() => showInviteModal = false}>
            <div class="modal-content glass dialog-sm" onclick={(e) => e.stopPropagation()}>
              <header class="modal-header">
                <h3>Zaproś użytkownika do organizacji</h3>
                <button class="btn-icon-compact secondary" onclick={() => showInviteModal = false}><X size={16} /></button>
              </header>
              <div class="modal-body">
                <div class="form-group">
                  <label for="invite-email">Adres e-mail *</label>
                  <input id="invite-email" type="email" placeholder="uzytkownik@firma.pl" bind:value={inviteForm.email} />
                </div>
                <div class="form-group">
                  <span class="group-label">Przypisz role startowe:</span>
                  <div class="checkbox-list">
                    {#each rolesList as role}
                      <label class="check-item">
                        <input type="checkbox" value={role.roleId} 
                          checked={inviteForm.roleIds.includes(role.roleId)}
                          onchange={(e) => {
                            const checked = (e.target as HTMLInputElement).checked;
                            if (checked) inviteForm.roleIds = [...inviteForm.roleIds, role.roleId];
                            else inviteForm.roleIds = inviteForm.roleIds.filter(id => id !== role.roleId);
                          }}
                        />
                        <span>{role.name}</span>
                      </label>
                    {/each}
                  </div>
                </div>
              </div>
              <footer class="modal-footer">
                <button class="secondary" onclick={() => showInviteModal = false}>Anuluj</button>
                <button class="primary" onclick={handleSendInvite} disabled={!inviteForm.email}>Wyślij zaproszenie</button>
              </footer>
            </div>
          </div>
        {/if}

        <!-- 4F. CREATE/EDIT ROLE MODAL -->
        {#if showCreateRoleModal}
          <div class="modal-backdrop" onclick={() => showCreateRoleModal = false}>
            <div class="modal-content glass dialog-md" onclick={(e) => e.stopPropagation()}>
              <header class="modal-header">
                <h3>{isEditingRole ? 'Edytuj' : 'Utwórz'} Rolę</h3>
                <button class="btn-icon-compact secondary" onclick={() => showCreateRoleModal = false}><X size={16} /></button>
              </header>
              <div class="modal-body scrollable max-h-400">
                <div class="form-group">
                  <label for="role-name">Nazwa roli *</label>
                  <input id="role-name" type="text" placeholder="Np. Administratorzy IT" bind:value={roleForm.name} />
                </div>
                <div class="form-group">
                  <label for="role-desc">Opis roli</label>
                  <textarea id="role-desc" placeholder="Dostęp techniczny do serwerów..." bind:value={roleForm.description}></textarea>
                </div>
                
                <div class="checkbox-group">
                  <input id="role-dev-app" type="checkbox" bind:checked={roleForm.requireDeviceApproval} />
                  <label for="role-dev-app">Wymagaj akceptacji urządzeń (MFA/Device Trust)</label>
                </div>

                <div class="checkbox-group mt-2">
                  <input id="role-allow-ssh" type="checkbox" bind:checked={roleForm.allowSsh} />
                  <label for="role-allow-ssh">Zezwól na dostęp SSH</label>
                </div>

                {#if roleForm.allowSsh}
                  <div class="ssh-role-details border-left-amber pl-3 mt-3">
                    <div class="form-group">
                      <label for="role-sudo">SSH Sudo Mode</label>
                      <select id="role-sudo" bind:value={roleForm.sshSudoMode}>
                        <option value="none">Brak Sudo (Użytkownik standardowy)</option>
                        <option value="full">Pełne Sudo (Pełne uprawnienia root)</option>
                        <option value="commands">Sudo tylko dla określonych komend</option>
                      </select>
                    </div>

                    {#if roleForm.sshSudoMode === 'commands'}
                      <div class="form-group">
                        <label for="sudo-commands-list">Dozwolone komendy Sudo:</label>
                        <div class="list-adder">
                          <input id="sudo-commands-list" type="text" placeholder="systemctl restart nginx" bind:value={newSudoCommand} />
                          <button class="primary btn-sm" onclick={addSudoCommand}>Dodaj</button>
                        </div>
                        <div class="tags-list mt-2">
                          {#each roleForm.sshSudoCommands as cmd}
                            <span class="tag-badge-removable">
                              {cmd} <button onclick={() => removeSudoCommand(cmd)}><X size={10} /></button>
                            </span>
                          {/each}
                        </div>
                      </div>
                    {/if}

                    <div class="checkbox-group mt-2">
                      <input id="role-home-dir" type="checkbox" bind:checked={roleForm.sshCreateHomeDir} />
                      <label for="role-home-dir">Automatycznie utwórz katalog domowy (/home)</label>
                    </div>

                    <div class="form-group mt-3">
                      <label for="unix-groups-list">Grupy UNIX systemowe na serwerze:</label>
                      <div class="list-adder">
                        <input id="unix-groups-list" type="text" placeholder="docker" bind:value={newUnixGroup} />
                        <button class="primary btn-sm" onclick={addUnixGroup}>Dodaj</button>
                      </div>
                      <div class="tags-list mt-2">
                        {#each roleForm.sshUnixGroups as grp}
                          <span class="tag-badge-removable">
                            {grp} <button onclick={() => removeUnixGroup(grp)}><X size={10} /></button>
                          </span>
                        {/each}
                      </div>
                    </div>
                  </div>
                {/if}
              </div>
              <footer class="modal-footer">
                <button class="secondary" onclick={() => showCreateRoleModal = false}>Anuluj</button>
                <button class="primary" onclick={handleSaveRole} disabled={!roleForm.name}>Zapisz Rolę</button>
              </footer>
            </div>
          </div>
        {/if}
      {/if}

      <!-- 5. CLIENTS / DEVICES VIEW -->
      {#if activeSubTab === 'clients'}
        <div class="clients-view">
          <div class="section-tabs">
            <button class="tab-btn" class:active={activeClientsTab === 'devices'} onclick={() => activeClientsTab = 'devices'}>
              Zaufane Urządzenia Użytkowników
            </button>
            <button class="tab-btn" class:active={activeClientsTab === 'tokens'} onclick={() => activeClientsTab = 'tokens'}>
              Tokeny API Maszynowe (Access Tokens)
            </button>
          </div>

          <div class="mgmt-section glass">
            {#if activeClientsTab === 'devices'}
              <header class="section-header">
                <h3>Zarejestrowane urządzenia końcowe</h3>
              </header>
              <table class="telemetry-table">
                <thead>
                  <tr>
                    <th>Nazwa urządzenia</th>
                    <th>Właściciel (Aktor)</th>
                    <th>Subnet IP</th>
                    <th>Zablokowane</th>
                    <th>Zarchiwizowane</th>
                    <th>Ostatnio widziane</th>
                    <th>Operacje</th>
                  </tr>
                </thead>
                <tbody>
                  {#if isClientsLoading}
                    <tr><td colspan="7" class="text-center py-4"><RefreshCw class="spin" size={16} /></td></tr>
                  {:else if clientsList.length === 0}
                    <tr><td colspan="7" class="text-center py-4 text-muted">Brak zarejestrowanych urządzeń.</td></tr>
                  {:else}
                    {#each clientsList as client}
                      <tr class:archived={client.archived} class:blocked={client.blocked}>
                        <td class="font-semibold">{client.name}</td>
                        <td>{client.username || 'Maszyna / Inny'}</td>
                        <td class="mono-stats">{client.subnet || '-'}</td>
                        <td>
                          <span class="status-badge {client.blocked ? 'blocked' : 'allowed'}">
                            {client.blocked ? 'Zablokowany' : 'Dozwolony'}
                          </span>
                        </td>
                        <td>
                          <span class="status-badge {client.archived ? 'blocked' : 'allowed'}">
                            {client.archived ? 'Tak' : 'Nie'}
                          </span>
                        </td>
                        <td class="mono-stats font-xs">
                          {client.lastHandshakeTime ? new Date(client.lastHandshakeTime * 1000).toLocaleString() : 'Nigdy'}
                        </td>
                        <td>
                          <div class="flex-actions">
                            <button class="secondary btn-sm" onclick={() => toggleBlockClient(client)}>
                              {client.blocked ? 'Odblokuj' : 'Zablokuj'}
                            </button>
                            <button class="secondary btn-sm" onclick={() => toggleArchiveClient(client)}>
                              {client.archived ? 'Przywróć' : 'Archiwizuj'}
                            </button>
                            <button class="secondary btn-icon-compact text-red" onclick={() => handleDeleteClient(client.clientId)}>
                              <Trash2 size={14} />
                            </button>
                          </div>
                        </td>
                      </tr>
                    {/each}
                  {/if}
                </tbody>
              </table>
            {:else}
              <header class="section-header">
                <h3>Maszynowe tokeny dostępu (API keys / Tokens)</h3>
              </header>
              <table class="telemetry-table">
                <thead>
                  <tr>
                    <th>Etykieta tokenu</th>
                    <th>ID Tokenu</th>
                    <th>Klucz (Prefix)</th>
                    <th>Wygasa za</th>
                    <th>Operacje</th>
                  </tr>
                </thead>
                <tbody>
                  {#if isClientsLoading}
                    <tr><td colspan="5" class="text-center py-4"><RefreshCw class="spin" size={16} /></td></tr>
                  {:else if accessTokensList.length === 0}
                    <tr><td colspan="5" class="text-center py-4 text-muted">Brak wygenerowanych kluczy dostępu.</td></tr>
                  {:else}
                    {#each accessTokensList as tok}
                      <tr>
                        <td class="font-semibold">{tok.name || 'Brak etykiety'}</td>
                        <td class="mono-stats font-xs">{tok.accessTokenId}</td>
                        <td class="mono-stats font-xs">{tok.keyPrefix}...</td>
                        <td>{tok.expiresAt ? new Date(tok.expiresAt).toLocaleString() : 'Nigdy'}</td>
                        <td>
                          <button class="secondary btn-icon-compact text-red" onclick={() => handleDeleteToken(tok.accessTokenId)}>
                            <Trash2 size={14} />
                          </button>
                        </td>
                      </tr>
                    {/each}
                  {/if}
                </tbody>
              </table>
            {/if}
          </div>
        </div>
      {/if}

      <!-- 6. SETTINGS VIEW -->
      {#if activeSubTab === 'settings'}
        <div class="settings-view glass glow-amber">
          <h3>Konfiguracja integracji Pangolin Integration API</h3>
          <p class="text-muted">Skonfiguruj połączenie z Twoim panelem zarządzania Pangolin, aby Jarvis mógł pobierać logi, statystyki oraz zarządzać tunelami i regułami dostępu.</p>
          
          {#if configMsg.text}
            <div class="info-alert {configMsg.type}">
              <Info size={16} />
              <span>{configMsg.text}</span>
            </div>
          {/if}

          <div class="settings-form">
            <div class="form-group">
              <label for="api-url-input">Adres URL API Pangolin</label>
              <input id="api-url-input" type="text" placeholder="https://api.pangolin.net" bind:value={config.api_url} />
              <span class="input-tip">Adres głównego panelu administracyjnego lub self-hosted Pangolin API.</span>
            </div>
            
            <div class="form-group">
              <label for="api-key-input">Pangolin API Token (Key)</label>
              <input id="api-key-input" type="password" placeholder={config.has_api_key ? '••••••••••••••••••••••••••••••••' : 'Wklej klucz API (Root lub Org Key)'} bind:value={apiKeyInput} />
              <span class="input-tip">Bearer token z uprawnieniami Integration API. {config.has_api_key ? 'Klucz jest już zapisany w bazie poświadczeń systemowych.' : 'Klucz zostanie zapisany w bezpiecznym menedżerze poświadczeń (Keyring).'}</span>
            </div>

            {#if orgs.length > 0}
              <div class="form-group">
                <label for="api-org-select">Aktywna Organizacja</label>
                <select id="api-org-select" bind:value={config.org_id}>
                  {#each orgs as org}
                    <option value={org.orgId}>{org.label} ({org.orgId})</option>
                  {/each}
                </select>
                <span class="input-tip">Wybierz organizację, którą chcesz zarządzać za pomocą tego panelu.</span>
              </div>
            {:else}
              <div class="form-group">
                <label for="api-org-input">Identyfikator Organizacji (Org ID)</label>
                <input id="api-org-input" type="text" placeholder="Wpisz ID organizacji ręcznie, jeśli nie pobrano listy" bind:value={config.org_id} />
                <span class="input-tip">Identyfikator organizacji dla kluczy o ograniczonym zasięgu (Org-scoped keys).</span>
              </div>
            {/if}

            <div class="flex-actions mt-4">
              <button class="primary" onclick={() => handleSaveConfig()} disabled={isSavingConfig}>
                <Check size={16} /> Zapisz i zweryfikuj
              </button>
              {#if config.has_api_key}
                <button class="secondary" onclick={loadConfig}>
                  <RefreshCw size={16} /> Odśwież połączenie
                </button>
              {/if}
            </div>
          </div>
        </div>
      {/if}
    {/if}
  </div>
</div>

<style>
  .pangolin-container {
    display: flex;
    flex-direction: column;
    height: 100%;
    color: var(--text-primary);
  }

  .tab-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 16px 24px;
    border-bottom: 1px solid var(--border-color);
    background: rgba(0,0,0,0.2);
  }

  .header-left {
    display: flex;
    align-items: center;
    gap: 16px;
  }

  .header-icon {
    color: var(--accent-amber);
    filter: drop-shadow(0 0 8px var(--accent-amber-glow));
  }

  .title-block h2 {
    font-size: 1.25rem;
    font-weight: 700;
  }

  .subtitle {
    font-size: 0.75rem;
    color: var(--text-secondary);
  }

  .tab-navbar {
    display: flex;
    gap: 8px;
  }

  .nav-btn {
    background: transparent;
    border: 1px solid transparent;
    color: var(--text-secondary);
    padding: 8px 14px;
    border-radius: var(--radius-sm);
    font-size: 0.85rem;
  }

  .nav-btn:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .nav-btn.active {
    background: var(--bg-active);
    color: var(--accent-amber);
    border-color: rgba(245, 158, 11, 0.2);
  }

  .nav-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .nav-btn:disabled:hover {
    background: transparent;
    color: var(--text-secondary);
  }

  .tab-content {
    flex: 1;
    padding: 24px;
    overflow-y: auto;
  }

  .scrollable {
    overflow-y: auto;
  }

  .loading-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 48px;
    gap: 16px;
    color: var(--text-secondary);
  }

  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 32px;
    color: var(--text-muted);
    gap: 8px;
  }

  .spin {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    100% { transform: rotate(360deg); }
  }

  .tab-content {
    flex: 1;
    padding: 16px;
    overflow-y: auto;
  }

  /* Dashboard CSS */
  .dashboard-view {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .stats-row {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 8px;
    flex-shrink: 0;
  }

  @media (max-width: 720px) {
    .stats-row {
      grid-template-columns: repeat(2, 1fr);
    }
  }

  .stat-card {
    padding: 8px 12px;
    border-radius: var(--radius-sm);
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 8px;
  }

  .glow-amber { border-left: 2px solid var(--accent-amber); }
  .glow-red { border-left: 2px solid var(--accent-red); }
  .glow-green { border-left: 2px solid var(--accent-green); }
  .glow-orange { border-left: 2px solid var(--accent-orange); }

  .card-label {
    font-size: 0.68rem;
    color: var(--text-secondary);
    letter-spacing: 0.04em;
    white-space: nowrap;
  }

  .card-val {
    font-family: var(--font-mono);
    font-size: 1.15rem;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
    line-height: 1;
  }

  .tabular-nums {
    font-variant-numeric: tabular-nums;
  }

  .filter-controls {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
  }

  .filter-controls select {
    height: 28px;
    padding: 2px 8px;
    font-size: 0.75rem;
  }

  .chart-section {
    padding: 14px 12px;
    border-radius: var(--radius-md);
    display: flex;
    flex-direction: column;
    overflow: visible;
    flex-shrink: 0;
  }

  .chart-section-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 10px;
  }

  .chart-section h3 {
    font-size: 0.88rem;
    font-weight: 600;
    margin: 0;
    text-wrap: balance;
  }

  .svg-chart-wrapper {
    position: relative;
    width: 100%;
    margin-inline: auto;
    display: grid;
    grid-template-columns: 36px minmax(0, 1fr);
    gap: 8px;
    align-items: start;
    flex-shrink: 0;
  }

  .chart-y-axis-container {
    position: relative;
    height: var(--chart-plot-height, 168px);
    min-height: 0;
  }

  .chart-y-axis {
    position: relative;
    height: 100%;
  }

  .chart-y-axis .chart-axis-label {
    position: absolute;
    right: 0;
    transform: translateY(-50%);
    font-family: var(--font-mono);
    font-size: 0.65rem;
    font-variant-numeric: tabular-nums;
    color: var(--text-muted);
    line-height: 1;
  }

  .chart-main-container {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }

  .chart-plot {
    width: 100%;
    height: var(--chart-plot-height, 168px);
    display: block;
    flex-shrink: 0;
    min-height: 0;
    overflow: hidden;
  }

  .svg-chart {
    width: 100%;
    display: block;
    min-width: 0;
  }

  .chart-x-axis {
    position: relative;
    left: 2%;
    width: 96%;
    height: 22px;
    margin-top: 4px;
    flex-shrink: 0;
    pointer-events: none;
  }

  .chart-x-axis .chart-x-label {
    position: absolute;
    transform: translateX(-50%);
    font-family: var(--font-mono);
    font-size: 0.62rem;
    font-variant-numeric: tabular-nums;
    color: var(--text-muted);
    white-space: nowrap;
  }

  .chart-legend {
    display: flex;
    gap: 14px;
    justify-content: center;
    font-size: 0.72rem;
    margin-top: 8px;
    max-width: 680px;
    margin-inline: auto;
    width: 100%;
  }

  .chart-section .loading-state,
  .chart-section .empty-state {
    height: 194px;
    padding: 24px;
  }

  .legend-item {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .legend-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
  }

  .legend-dot.green { background-color: var(--accent-green); }
  .legend-dot.red { background-color: var(--accent-red); }
  .legend-dot.pink { background-color: #f472b6; }

  .bar-chart {
    overflow: visible;
  }

  .stats-grids {
    display: grid;
    grid-template-columns: 1.1fr 0.9fr;
    gap: 20px;
  }

  @media (max-width: 960px) {
    .stats-grids {
      grid-template-columns: 1fr;
    }
  }

  .stats-panel {
    padding: 20px;
    border-radius: var(--radius-md);
  }

  .stats-panel h3 {
    font-size: 0.95rem;
    margin-bottom: 16px;
    font-weight: 600;
    text-wrap: balance;
  }

  .geo-map-panel {
    display: flex;
    flex-direction: column;
  }

  .top-countries-header {
    display: grid;
    grid-template-columns: 1fr 64px 40px;
    gap: 8px;
    font-size: 0.7rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-muted);
    padding: 0 12px 8px;
    border-bottom: 1px solid var(--border-color);
    margin-bottom: 4px;
  }

  .top-countries-list {
    display: flex;
    flex-direction: column;
    max-height: 340px;
    overflow-y: auto;
    padding-right: 4px;
  }

  .country-row {
    position: relative;
    display: grid;
    grid-template-columns: 1fr 64px 40px;
    gap: 8px;
    align-items: center;
    padding: 10px 12px;
    border-radius: var(--radius-sm);
    font-size: 0.85rem;
    transition: background 0.15s ease, transform 0.15s ease;
    cursor: default;
  }

  .country-row.hovered {
    transform: translateX(2px);
    box-shadow: inset 0 0 0 1px rgba(251, 146, 60, 0.25);
  }

  .country-label {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }

  .country-code {
    font-family: var(--font-mono);
    font-size: 0.75rem;
    font-weight: 700;
    color: var(--accent-amber);
    width: 24px;
    flex-shrink: 0;
  }

  .country-name {
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .country-total,
  .country-pct {
    text-align: right;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }

  .country-pct {
    color: var(--text-secondary);
  }

  .country-tooltip-fixed {
    position: fixed;
    background: rgba(15, 15, 18, 0.96);
    border: 1px solid rgba(251, 146, 60, 0.3);
    border-radius: var(--radius-sm);
    padding: 8px 10px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: 0.75rem;
    z-index: 9999;
    pointer-events: none;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
    width: 240px;
  }

  .country-tooltip-fixed strong {
    color: var(--text-primary);
    font-size: 0.8rem;
  }

  .tooltip-detail {
    color: var(--text-muted);
    font-size: 0.72rem;
  }

  /* Custom Chart Tooltip */
  .chart-tooltip-fixed {
    position: fixed;
    background: rgba(15, 15, 18, 0.96);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: var(--radius-md);
    padding: 10px 12px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 0.75rem;
    z-index: 9999;
    pointer-events: none;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.5);
    width: 220px;
    backdrop-filter: blur(8px);
  }

  .tooltip-header {
    font-weight: 700;
    color: var(--text-primary);
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
    padding-bottom: 4px;
    margin-bottom: 2px;
  }

  .tooltip-body {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .tooltip-row {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .tooltip-row .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .tooltip-row .dot.total { background-color: var(--text-muted); }
  .tooltip-row .dot.green { background-color: var(--accent-green); }
  .tooltip-row .dot.pink { background-color: #f472b6; }

  .tooltip-row .label {
    color: var(--text-secondary);
    flex-grow: 1;
  }

  .tooltip-row .val {
    font-family: var(--font-mono);
    font-weight: 600;
    color: var(--text-primary);
  }

  .tooltip-row .val.text-pink {
    color: #f472b6;
  }

  /* Logs view CSS */
  .filters-bar {
    display: flex;
    gap: 16px;
    padding: 16px 20px;
    border-radius: var(--radius-md);
    margin-bottom: 20px;
    align-items: flex-end;
  }

  .filter-col {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .filter-col label {
    font-size: 0.75rem;
    color: var(--text-secondary);
  }

  .filter-col select, .filter-col input {
    height: 36px;
    padding: 6px 12px;
  }

  .flex-grow {
    flex-grow: 1;
  }

  .logs-table-wrapper {
    border-radius: var(--radius-md);
    overflow: hidden;
    margin-bottom: 16px;
  }

  .telemetry-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.85rem;
  }

  .telemetry-table th {
    background: rgba(0,0,0,0.3);
    color: var(--text-secondary);
    font-weight: 600;
    text-align: left;
    padding: 12px 16px;
    font-size: 0.75rem;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    border-bottom: 1px solid var(--border-color);
  }

  .telemetry-table td {
    padding: 12px 16px;
    border-bottom: 1px solid var(--border-color);
    vertical-align: middle;
  }

  .log-row {
    cursor: pointer;
    transition: var(--transition-fast);
  }

  .log-row:hover {
    background: var(--bg-hover);
  }

  .font-xs {
    font-size: 0.75rem;
  }

  .truncate-cell {
    max-width: 180px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .method-tag {
    font-family: var(--font-mono);
    font-size: 0.7rem;
    font-weight: 700;
    padding: 2px 6px;
    border-radius: 3px;
    background: rgba(255,255,255,0.1);
  }

  .method-tag.get { color: #60a5fa; background: rgba(96, 165, 250, 0.1); }
  .method-tag.post { color: var(--accent-green); background: rgba(16, 185, 129, 0.1); }
  .method-tag.put { color: var(--accent-amber); background: rgba(245, 158, 11, 0.1); }
  .method-tag.delete { color: var(--accent-red); background: rgba(239, 68, 68, 0.1); }
  
  .status-badge {
    font-size: 0.7rem;
    font-weight: 600;
    padding: 2px 8px;
    border-radius: 10px;
  }

  .status-badge.allowed { color: var(--accent-green); background: var(--accent-green-glow); }
  .status-badge.blocked { color: var(--accent-red); background: var(--accent-red-glow); }

  .ip-block {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .country-badge {
    font-size: 0.65rem;
    padding: 1px 4px;
    border-radius: 2px;
    background: rgba(255, 255, 255, 0.1);
    color: var(--text-secondary);
  }

  .pagination-bar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-size: 0.85rem;
    color: var(--text-secondary);
    padding: 8px 10px;
  }

  .pagination-controls {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  /* Modals */
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    backdrop-filter: blur(4px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
    padding: 20px;
  }

  .modal-content {
    border-radius: var(--radius-lg);
    display: flex;
    flex-direction: column;
    box-shadow: 0 20px 50px rgba(0,0,0,0.6);
    border: 1px solid var(--border-color);
  }

  .dialog-sm { width: 400px; }
  .dialog-md { width: 600px; }

  .modal-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 16px 20px;
    border-bottom: 1px solid var(--border-color);
  }

  .modal-body {
    padding: 20px;
  }

  .max-h-400 {
    max-height: 400px;
  }

  .modal-footer {
    display: flex;
    justify-content: flex-end;
    gap: 12px;
    padding: 16px 20px;
    border-top: 1px solid var(--border-color);
    background: rgba(0,0,0,0.1);
  }

  /* Form Elements */
  .form-group {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-bottom: 14px;
  }

  .form-group label {
    font-size: 0.8rem;
    font-weight: 500;
    color: var(--text-secondary);
  }

  .form-grid-2 {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 16px;
  }

  .form-grid-3 {
    display: grid;
    grid-template-columns: 1fr 1fr 1fr;
    gap: 16px;
  }

  .link-btn {
    background: transparent;
    border: none;
    padding: 0;
    margin: 0;
    color: var(--accent-amber);
    text-align: left;
    cursor: pointer;
    font-family: inherit;
    font-size: inherit;
    display: inline-block;
  }
  .link-btn:hover {
    text-decoration: underline;
  }

  .checkbox-group {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 0.85rem;
    margin-top: 8px;
  }

  .checkbox-group label {
    cursor: pointer;
  }

  .checkbox-list {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
    gap: 10px;
    background: rgba(255,255,255,0.02);
    border: 1px solid var(--border-color);
    padding: 12px;
    border-radius: var(--radius-sm);
  }

  .check-item {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 0.8rem;
    cursor: pointer;
  }

  .group-label {
    font-size: 0.8rem;
    color: var(--text-secondary);
    margin-bottom: 6px;
    display: block;
  }

  .btn-sm {
    padding: 6px 12px;
    font-size: 0.8rem;
  }

  /* Management Sections */
  .mgmt-section {
    padding: 20px;
    border-radius: var(--radius-md);
    margin-bottom: 24px;
  }

  .section-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 16px;
  }

  .sec-title {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .sec-title h3 {
    font-size: 0.95rem;
    font-weight: 600;
  }

  .flex-actions {
    display: flex;
    gap: 8px;
  }

  .text-red { color: var(--accent-red); }
  .text-green { color: var(--accent-green); }
  .text-orange { color: var(--accent-amber); }

  .mode-tag {
    font-size: 0.75rem;
    font-weight: 600;
    padding: 1px 6px;
    border-radius: 4px;
    text-transform: uppercase;
    background: rgba(255,255,255,0.1);
  }
  
  .mode-tag.http { color: #a855f7; background: rgba(168, 85, 247, 0.1); }
  .mode-tag.ssh { color: #f43f5e; background: rgba(244, 63, 94, 0.1); }
  .mode-tag.rdp { color: #0ea5e9; background: rgba(14, 165, 233, 0.1); }

  .status-dot {
    display: inline-block;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    margin-right: 6px;
  }

  .status-dot.nominal { background: var(--accent-green); box-shadow: 0 0 6px var(--accent-green); }
  .status-dot.inactive { background: var(--text-muted); }

  /* Settings CSS */
  .settings-view {
    max-width: 600px;
    margin: 0 auto;
    padding: 30px;
    border-radius: var(--radius-lg);
  }

  .settings-view h3 {
    font-size: 1.1rem;
    margin-bottom: 8px;
  }

  .settings-form {
    margin-top: 24px;
  }

  .input-tip {
    font-size: 0.75rem;
    color: var(--text-muted);
    margin-top: 4px;
  }

  .info-alert {
    display: flex;
    gap: 12px;
    padding: 12px 16px;
    border-radius: var(--radius-sm);
    margin-top: 16px;
    font-size: 0.85rem;
    align-items: center;
  }

  .info-alert.warning {
    background: rgba(245, 158, 11, 0.1);
    border: 1px solid rgba(245, 158, 11, 0.2);
    color: var(--accent-amber);
  }

  .info-alert.error {
    background: rgba(239, 68, 68, 0.1);
    border: 1px solid rgba(239, 68, 68, 0.2);
    color: var(--accent-red);
  }

  .info-alert.success {
    background: rgba(16, 185, 129, 0.1);
    border: 1px solid rgba(16, 185, 129, 0.2);
    color: var(--accent-green);
  }

  /* Access Sub-tabs */
  .access-tab-nav {
    display: flex;
    gap: 4px;
    background: rgba(0,0,0,0.2);
    padding: 4px;
    border-radius: var(--radius-sm);
    margin-bottom: 16px;
    width: fit-content;
  }

  .tab-btn {
    background: transparent;
    border: none;
    color: var(--text-secondary);
    padding: 6px 12px;
    font-size: 0.8rem;
    border-radius: var(--radius-sm);
  }

  .tab-btn.active {
    background: var(--bg-hover);
    color: var(--accent-amber);
    font-weight: 600;
  }

  .tag-badge {
    background: rgba(245, 158, 11, 0.1);
    border: 1px solid rgba(245, 158, 11, 0.2);
    color: var(--accent-amber);
    font-size: 0.75rem;
    padding: 1px 6px;
    border-radius: 4px;
  }

  .tag-badge-removable {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    background: rgba(245, 158, 11, 0.1);
    border: 1px solid rgba(245, 158, 11, 0.2);
    color: var(--accent-amber);
    font-size: 0.75rem;
    padding: 2px 6px;
    border-radius: 4px;
  }

  .tag-badge-removable button {
    background: transparent;
    border: none;
    color: var(--accent-amber);
    padding: 0;
    cursor: pointer;
    display: flex;
    align-items: center;
  }

  .list-adder {
    display: flex;
    gap: 8px;
  }

  .list-adder input {
    flex: 1;
  }

  .tags-list {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .border-left-amber {
    border-left: 2px solid var(--accent-amber);
  }

  .select-all {
    user-select: all;
  }

  /* Clients Tab Table overrides */
  tr.archived td {
    opacity: 0.5;
  }
  
  tr.blocked td {
    color: var(--accent-red);
  }

  /* Log details modal overrides */
  .log-details {
    width: 650px;
    max-height: 90vh;
  }

  .grid-details {
    display: grid;
    gap: 12px;
    margin-bottom: 20px;
  }

  .detail-row {
    display: flex;
    justify-content: space-between;
    border-bottom: 1px solid var(--border-color);
    padding-bottom: 8px;
    font-size: 0.85rem;
  }

  .detail-row .lbl {
    color: var(--text-secondary);
    font-weight: 500;
  }

  .detail-row .val {
    color: var(--text-primary);
    text-align: right;
  }

  .truncate-text {
    max-width: 400px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .json-section {
    margin-top: 20px;
  }

  .json-section h4 {
    font-size: 0.85rem;
    color: var(--text-secondary);
    margin-bottom: 8px;
  }

  .json-block {
    background: rgba(0,0,0,0.4);
    border: 1px solid var(--border-color);
    padding: 12px;
    border-radius: var(--radius-sm);
    font-family: var(--font-mono);
    white-space: pre-wrap;
    word-break: break-all;
    max-height: 180px;
    overflow-y: auto;
  }
</style>
