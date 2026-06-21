<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { 
    ShieldAlert, Shield, ShieldOff, Play, Square, RotateCw, RefreshCw, 
    Plus, Trash2, KeyRound, Check, HelpCircle, Settings, Clipboard,
    ArrowUpRight, AlertCircle, Cpu, FileText, Activity, Users, Box, HardDrive, List, Info,
    Search, Pause
  } from 'lucide-svelte';
  import SortableTh from './ui/SortableTh.svelte';
  import { applySort, nextSort, type SortState } from '$lib/sort/sortUtils';
  import yaml from 'js-yaml';
  import { get } from 'svelte/store';
    import {
    formatInvokeError,
    isSudoPasswordIncorrect,
    isSudoPasswordRequired,
  } from '$lib/backendErrors';
  import { formatDate } from '$lib/formatLocale';
  import { notifications } from '$lib/notifications.svelte';

  // Svelte 5 Props
  let { profileId, visible = true } = $props<{ profileId: string; visible?: boolean }>();

  export function refresh() {
    if (isInstalled) loadAllData();
    else initCrowdsec();
  }

  // Connection and installation management state
  let isInstalled = $state<boolean | null>(null);
  let connectionMode = $state<'auto' | 'baremetal' | 'docker'>('auto');
  let detectedMode = $state<'baremetal' | 'docker' | null>(null);
  let containerName = $state('crowdsec');
  let customPrefix = $state('');
  let isServiceActive = $state<boolean | null>(null);
  let lapiVersion = $state('');

  // Loading and form states
  let isLoading = $state(false);
  let isInstalling = $state(false);
  let errorMsg = $state('');

  $effect(() => {
    if (errorMsg) {
      notifications.error(errorMsg);
      errorMsg = '';
    }
  });
  let activeSubTab = $state<'dashboard' | 'decisions' | 'whitelist' | 'alerts' | 'bouncers' | 'metrics' | 'hub'>('dashboard');

  // Modals
  let showSettingsModal = $state(false);
  let showAddDecisionModal = $state(false);
  let showAddBouncerModal = $state(false);
  let showSudoModal = $state(false);
  let showBouncerKeyModal = $state(false);

  // CrowdSec data
  let decisions = $state<any[]>([]);
  let whitelistData = $state<{ ip: string[]; cidr: string[] }>({ ip: [], cidr: [] });
  interface WhitelistItem {
    value: string;
    type: 'ip' | 'cidr';
    file: string;
    isSystem: boolean;
  }
  let whitelistItems = $state<WhitelistItem[]>([]);
  let whitelistTarget = $state<string>('yaml');
  let lapiAllowlists = $state<string[]>([]);
  let alerts = $state<any[]>([]);
  let bouncers = $state<any[]>([]);
  let metrics = $state<any>(null);
  let hubItems = $state<any[]>([]);

  // Form data and generated keys
  let newDecisionIp = $state('');
  let newDecisionDuration = $state('4h');
  let newDecisionReason = $state("Manual block");
  let newDecisionScope = $state('ip');
  let newBouncerName = $state('');
  let generatedBouncerKey = $state('');
  let newWhitelistIp = $state('');
  let newWhitelistType = $state<'ip' | 'cidr'>('ip');

  // Sudo auth state
  let sudoPassword = $state('');
  let pendingAction: (() => Promise<void>) | null = null;
  let sudoError = $state('');
  let isSudoAuthorized = $state(false);

  // Search and sorting
  let searchDecisionQuery = $state('');
  let searchAlertQuery = $state('');
  let searchHubQuery = $state('');
  
  type DecisionSortCol = 'value' | 'type' | 'origin' | 'reason' | 'cn' | 'duration' | 'until';
  let decisionSort = $state<SortState<DecisionSortCol>>({ column: 'value', direction: 'asc' });

  type AlertSortCol = 'id' | 'source' | 'scenario' | 'events_count' | 'created_at';
  let alertSort = $state<SortState<AlertSortCol>>({ column: 'id', direction: 'desc' });

  type HubSortCol = 'type' | 'name' | 'status' | 'version';
  let hubSort = $state<SortState<HubSortCol>>({ column: 'name', direction: 'asc' });

  // Selected detailed alert for preview
  let selectedAlert = $state<any | null>(null);

  // Metric Log Viewer state
  let selectedMetricLogPath = $state<string | null>(null);
  let metricLogContent = $state<string>('');
  let isMetricLogLoading = $state<boolean>(false);
  let metricLogSearchQuery = $state<string>('');
  let isMetricLogStreaming = $state<boolean>(false);
  let metricLogIntervalId = $state<any>(null);

  // Local storage configuration
  const configKey = $derived(`crowdsec_config_${profileId}`);

  function loadLocalConfig() {
    try {
      const stored = localStorage.getItem(configKey);
      if (stored) {
        const parsed = JSON.parse(stored);
        connectionMode = parsed.connectionMode || 'auto';
        containerName = parsed.containerName || 'crowdsec';
        customPrefix = parsed.customPrefix || '';
        return true;
      }
    } catch (e) {
      console.error(e);
    }
    return false;
  }

  function saveLocalConfig() {
    try {
      localStorage.setItem(configKey, JSON.stringify({
        connectionMode,
        containerName,
        customPrefix
      }));
    } catch (e) {
      console.error(e);
    }
  }

  // Building cscli command depending on mode
  function getCscliPrefix(): string {
    if (connectionMode === 'docker' || (connectionMode === 'auto' && detectedMode === 'docker')) {
      return customPrefix ? customPrefix : `docker exec -i ${containerName} cscli`;
    }
    return 'cscli';
  }

  async function runCscliCommand(cmdArgs: string, useSudo: boolean = true): Promise<string> {
    const prefix = getCscliPrefix();
    const cmd = `${prefix} ${cmdArgs}`;
    return await invoke('exec_custom_command', { cmd, useSudo });
  }

  async function handleActionWithSudo(action: () => Promise<void>) {
    const run = async () => {
      try {
        await action();
      } catch (err: unknown) {
        if (isSudoPasswordRequired(err)) {
          pendingAction = run;
          showSudoModal = true;
        } else if (isSudoPasswordIncorrect(err)) {
          sudoError = "Incorrect sudo password. Try again.";
          showSudoModal = true;
        } else {
          errorMsg = `Action failed: ${formatInvokeError(err)}`;
        }
      }
    };
    await run();
  }

  async function submitSudoPassword() {
    sudoError = '';
    try {
      await invoke('set_sudo_password', { password: sudoPassword });
      showSudoModal = false;
      sudoPassword = '';
      isSudoAuthorized = true;
      if (pendingAction) {
        const action = pendingAction;
        pendingAction = null;
        await action();
      }
    } catch (err: unknown) {
      sudoError = isSudoPasswordIncorrect(err)
        ? "Incorrect sudo password. Try again."
        : formatInvokeError(err);
    }
  }

  function requestSudoAuth() {
    pendingAction = async () => {
      isSudoAuthorized = true;
      await detectEnvironment();
    };
    showSudoModal = true;
  }

  function cancelSudoModal() {
    showSudoModal = false;
    sudoPassword = '';
    pendingAction = null;
    isSudoAuthorized = false;
    isInstalled = null;
    isServiceActive = null;
    isLoading = false;
    errorMsg = '';
  }

  async function initCrowdsec() {
    try {
      isSudoAuthorized = await invoke<boolean>('has_sudo_password');
    } catch {
      isSudoAuthorized = false;
    }

    if (!isSudoAuthorized) {
      isInstalled = null;
      isLoading = false;
      return;
    }

    await detectEnvironment();
  }

  // Find installation and auto-detection
  async function detectEnvironment() {
    isLoading = true;
    errorMsg = '';
    
    // Load saved configuration
    loadLocalConfig();

    try {
      if (connectionMode === 'baremetal') {
        detectedMode = 'baremetal';
        isInstalled = true;
      } else if (connectionMode === 'docker') {
        detectedMode = 'docker';
        isInstalled = true;
      } else {
        // Automatic mode
        // 1. Check whether cscli is installed natively
        try {
          await invoke('exec_custom_command', { cmd: 'which cscli', useSudo: false });
          detectedMode = 'baremetal';
          isInstalled = true;
        } catch (e) {
          // 2. If not natively, check docker containers
          try {
            const dockerOut = await invoke<string>('exec_custom_command', {
              cmd: 'docker ps --filter "name=crowdsec" --format "{{.Names}}"',
              useSudo: true
            });
            const containers = dockerOut.trim().split('\n').filter(Boolean);
            if (containers.length > 0) {
              detectedMode = 'docker';
              // By default we take the first matched container if the user did not enter another one
              if (!containerName || containerName === 'crowdsec') {
                containerName = containers[0];
              }
              isInstalled = true;
            } else {
              isInstalled = false;
            }
          } catch (dockErr) {
            isInstalled = false;
          }
        }
      }

      if (isInstalled) {
        await loadAllData();
      }
    } catch (err: any) {
      errorMsg = `Environment detection error: ${formatInvokeError(err)}`;
      isInstalled = false;
    } finally {
      isLoading = false;
    }
  }

  // Load all data
  async function loadAllData() {
    if (!isInstalled) return;
    isLoading = true;
    errorMsg = '';

    try {
      // 1. Service status
      await fetchServiceStatus();

      // 2. Depending on the active tab, we load specific data,
      //    but we also load key statistics for the Dashboard.
      await Promise.all([
        fetchDecisions(),
        fetchBouncers(),
        fetchAlerts(),
        fetchMetrics(),
        fetchHub(),
        fetchWhitelist()
      ]);
    } catch (err: any) {
      if (isSudoPasswordRequired(err) || isSudoPasswordIncorrect(err)) {
        pendingAction = loadAllData;
        showSudoModal = true;
        if (isSudoPasswordIncorrect(err)) {
          sudoError = "Incorrect sudo password. Try again.";
        }
      } else {
        errorMsg = `Failed to load CrowdSec data: ${formatInvokeError(err)}`;
      }
    } finally {
      isLoading = false;
    }
  }

  // Fetch system service or container status
  async function fetchServiceStatus() {
    const isDocker = connectionMode === 'docker' || (connectionMode === 'auto' && detectedMode === 'docker');
    try {
      if (isDocker) {
        const out = await invoke<string>('exec_custom_command', {
          cmd: `docker inspect -f '{{.State.Running}}' ${containerName}`,
          useSudo: true
        });
        isServiceActive = out.trim() === 'true';
        
        // Fetch LAPI version in Docker
        const verOut = await runCscliCommand('version', true);
        const match = verOut.match(/version:\s*(v[0-9.]+)/i);
        lapiVersion = match ? match[1] : 'Docker Container';
      } else {
        const out = await invoke<string>('exec_custom_command', {
          cmd: 'systemctl is-active crowdsec',
          useSudo: false
        });
        isServiceActive = out.trim() === 'active';

        const verOut = await runCscliCommand('version', true);
        const match = verOut.match(/version:\s*(v[0-9.]+)/i);
        lapiVersion = match ? match[1] : 'Systemd Native';
      }
    } catch (err: any) {
      if (isSudoPasswordRequired(err) || isSudoPasswordIncorrect(err)) {
        throw err;
      }
      console.error(err);
      isServiceActive = false;
      lapiVersion = "Unknown";
    }
  }

  // Service control (Start/Stop/Restart)
  async function controlService(action: 'start' | 'stop' | 'restart') {
    const isDocker = connectionMode === 'docker' || (connectionMode === 'auto' && detectedMode === 'docker');
    const run = async () => {
      isLoading = true;
      errorMsg = '';
      try {
        if (isDocker) {
          await invoke('exec_custom_command', {
            cmd: `docker ${action} ${containerName}`,
            useSudo: true
          });
        } else {
          await invoke('exec_custom_command', {
            cmd: `systemctl ${action} crowdsec`,
            useSudo: true
          });
        }
        await fetchServiceStatus();
      } catch (err: any) {
        if (isSudoPasswordRequired(err) || isSudoPasswordIncorrect(err)) {
          throw err;
        }
        errorMsg = `Service ${action} failed: ${formatInvokeError(err)}`;
      } finally {
        isLoading = false;
      }
    };

    await handleActionWithSudo(run);
  }

  function parseDurationToMs(durationStr: string): number {
    if (!durationStr) return 0;
    const regex = /(\d+(?:\.\d+)?)(d|h|m|s)/g;
    let matches;
    let totalMs = 0;
    while ((matches = regex.exec(durationStr)) !== null) {
      const value = parseFloat(matches[1]);
      const unit = matches[2];
      switch (unit) {
        case 'd': totalMs += value * 24 * 60 * 60 * 1000; break;
        case 'h': totalMs += value * 60 * 60 * 1000; break;
        case 'm': totalMs += value * 60 * 1000; break;
        case 's': totalMs += value * 1000; break;
      }
    }
    return totalMs;
  }

  // Fetch decisions (Bans)
  async function fetchDecisions() {
    try {
      const out = await runCscliCommand('decisions list -o json', true);
      const parsed = JSON.parse(out);
      let flatDecisions: any[] = [];
      if (Array.isArray(parsed)) {
        for (const item of parsed) {
          if (item && Array.isArray(item.decisions)) {
            for (const dec of item.decisions) {
              const untilVal = dec.until || (dec.duration ? new Date(Date.now() + parseDurationToMs(dec.duration)).toISOString() : '');
              flatDecisions.push({
                ...dec,
                until: untilVal,
                cn: item.source?.cn || '',
                as_name: item.source?.as_name || '',
                as_number: item.source?.as_number || '',
              });
            }
          } else if (item && item.value && item.type) {
            const untilVal = item.until || (item.duration ? new Date(Date.now() + parseDurationToMs(item.duration)).toISOString() : '');
            flatDecisions.push({
              ...item,
              until: untilVal
            });
          }
        }
      }
      decisions = flatDecisions;
    } catch (err: any) {
      if (isSudoPasswordRequired(err) || isSudoPasswordIncorrect(err)) {
        throw err;
      }
      console.error('Error fetching decisions:', err);
      decisions = [];
    }
  }

  // Add manual ban
  async function addDecision() {
    if (!newDecisionIp) return;
    const run = async () => {
      isLoading = true;
      errorMsg = '';
      try {
        const cmd = `decisions add --${newDecisionScope} ${newDecisionIp} --duration ${newDecisionDuration} --reason "${newDecisionReason}"`;
        await runCscliCommand(cmd, true);
        showAddDecisionModal = false;
        newDecisionIp = '';
        await fetchDecisions();
      } catch (err: any) {
        if (isSudoPasswordRequired(err) || isSudoPasswordIncorrect(err)) {
          throw err;
        }
        errorMsg = `Failed to add ban: ${formatInvokeError(err)}`;
      } finally {
        isLoading = false;
      }
    };
    await handleActionWithSudo(run);
  }

  // Manual ban removal (Unban)
  async function deleteDecision(ip: string) {
    if (!confirm(`Are you sure you want to remove the ban (unban) for IP ${ip}?`)) return;
    const run = async () => {
      isLoading = true;
      errorMsg = '';
      try {
        await runCscliCommand(`decisions delete --ip ${ip}`, true);
        await fetchDecisions();
      } catch (err: any) {
        if (isSudoPasswordRequired(err) || isSudoPasswordIncorrect(err)) {
          throw err;
        }
        errorMsg = `Failed to unban ${ip}: ${formatInvokeError(err)}`;
      } finally {
        isLoading = false;
      }
    };
    await handleActionWithSudo(run);
  }

  // Remove all decisions (bans)
  async function deleteAllDecisions() {
    if (!confirm("Are you sure you want to remove ALL active CrowdSec blocks?")) return;
    const run = async () => {
      isLoading = true;
      errorMsg = '';
      try {
        await runCscliCommand('decisions delete --all', true);
        await fetchDecisions();
      } catch (err: any) {
        if (isSudoPasswordRequired(err) || isSudoPasswordIncorrect(err)) {
          throw err;
        }
        errorMsg = `Failed to remove all bans: ${formatInvokeError(err)}`;
      } finally {
        isLoading = false;
      }
    };
    await handleActionWithSudo(run);
  }

  // Fetch whitelist from all files in s02-enrich and LAPI
  async function fetchWhitelist() {
    const isDocker = connectionMode === 'docker' || (connectionMode === 'auto' && detectedMode === 'docker');
    const dirPath = '/etc/crowdsec/parsers/s02-enrich';
    
    let items: WhitelistItem[] = [];
    let managedIp: string[] = [];
    let managedCidr: string[] = [];
    let foundLapiLists: string[] = [];

    // 1. Fetching from YAML parser files
    try {
      let filesListStr = '';
      if (isDocker) {
        filesListStr = await invoke<string>('exec_custom_command', {
          cmd: `docker exec -i ${containerName} ls -1 ${dirPath}`,
          useSudo: true
        });
      } else {
        filesListStr = await invoke<string>('exec_custom_command', {
          cmd: `ls -1 ${dirPath}`,
          useSudo: true
        });
      }

      const files = filesListStr.trim().split('\n').filter(f => f.endsWith('.yaml') || f.endsWith('.yml'));

      for (const file of files) {
        const filePath = `${dirPath}/${file}`;
        let content = '';
        try {
          if (isDocker) {
            content = await invoke<string>('exec_custom_command', {
              cmd: `docker exec -i ${containerName} cat ${filePath}`,
              useSudo: true
            });
          } else {
            content = await invoke<string>('exec_custom_command', {
              cmd: `cat ${filePath}`,
              useSudo: true
            });
          }

          const parsed: any = yaml.load(content);
          if (parsed && parsed.whitelist) {
            const ips = Array.isArray(parsed.whitelist.ip) ? parsed.whitelist.ip : [];
            const cidrs = Array.isArray(parsed.whitelist.cidr) ? parsed.whitelist.cidr : [];
            const isSystem = file === 'whitelists.yaml';

            ips.forEach((ip: string) => {
              items.push({ value: ip, type: 'ip', file, isSystem });
            });
            cidrs.forEach((cidr: string) => {
              items.push({ value: cidr, type: 'cidr', file, isSystem });
            });

            // If it is a file managed by Jarvis, load into the form
            if (file === 'jarvis-whitelist.yaml') {
              managedIp = ips;
              managedCidr = cidrs;
            }
          }
        } catch (fileErr: any) {
          if (isSudoPasswordRequired(fileErr) || isSudoPasswordIncorrect(fileErr)) {
            throw fileErr;
          }
          console.error(`Error reading whitelist file ${file}:`, fileErr);
        }
      }
    } catch (err: any) {
      if (isSudoPasswordRequired(err) || isSudoPasswordIncorrect(err)) {
        throw err;
      }
      console.error('Error listing whitelist files:', err);
    }

    // 2. Fetching allowlists from LAPI
    try {
      let allowlistsOut = '';
      let usingSubcommand = 'allowlists';
      try {
        allowlistsOut = await runCscliCommand('allowlists list -o json', true);
      } catch (e) {
        // Try singular alias
        allowlistsOut = await runCscliCommand('allowlist list -o json', true);
        usingSubcommand = 'allowlist';
      }

      if (allowlistsOut && allowlistsOut.trim()) {
        const lists = JSON.parse(allowlistsOut);
        if (Array.isArray(lists)) {
          for (const listObj of lists) {
            const name = listObj.name;
            if (name) {
              foundLapiLists.push(name);
              try {
                const inspectOut = await runCscliCommand(`${usingSubcommand} inspect ${name} -o json`, true);
                if (inspectOut && inspectOut.trim()) {
                  const inspectData = JSON.parse(inspectOut);
                  let listItems: any[] = [];
                  if (Array.isArray(inspectData)) {
                    listItems = inspectData;
                  } else if (inspectData && typeof inspectData === 'object') {
                    if (Array.isArray(inspectData.items)) {
                      listItems = inspectData.items;
                    } else if (Array.isArray(inspectData.content)) {
                      listItems = inspectData.content;
                    } else if (Array.isArray(inspectData.values)) {
                      listItems = inspectData.values;
                    } else if (Array.isArray(inspectData.entries)) {
                      listItems = inspectData.entries;
                    }
                  }

                  listItems.forEach((entry: any) => {
                    const value = entry.value || entry.ip || entry.cidr;
                    if (value) {
                      const type = value.includes('/') ? 'cidr' : 'ip';
                      items.push({
                        value: value,
                        type: type,
                        file: `LAPI: ${name}`,
                        isSystem: false
                      });
                    }
                  });
                }
              } catch (inspectErr: any) {
                if (isSudoPasswordRequired(inspectErr) || isSudoPasswordIncorrect(inspectErr)) {
                  throw inspectErr;
                }
                console.error(`Error inspecting allowlist ${name}:`, inspectErr);
              }
            }
          }
        }
      }
    } catch (lapiErr: any) {
      if (isSudoPasswordRequired(lapiErr) || isSudoPasswordIncorrect(lapiErr)) {
        throw lapiErr;
      }
      console.warn('LAPI allowlists are not supported or could not be fetched:', lapiErr);
    }

    whitelistItems = items;
    whitelistData = { ip: managedIp, cidr: managedCidr };
    lapiAllowlists = foundLapiLists;
  }

  // Save whitelist to YAML file and reload/restart
  async function saveWhitelist(updatedData: { ip: string[]; cidr: string[] }) {
    const isDocker = connectionMode === 'docker' || (connectionMode === 'auto' && detectedMode === 'docker');
    const path = '/etc/crowdsec/parsers/s02-enrich/jarvis-whitelist.yaml';
    
    const run = async () => {
      isLoading = true;
      errorMsg = '';
      try {
        const doc = {
          name: 'crowdsecurity/jarvis-whitelist',
          description: 'Whitelist managed by Jarvis',
          filter: '1==1',
          whitelist: {
            reason: 'Jarvis whitelisted IP',
            ip: updatedData.ip,
            cidr: updatedData.cidr
          }
        };
        const yamlStr = yaml.dump(doc);
        
        // Since the file is in a protected location, we must write using echo and sudo tee
        // Safe escaping of quotes for echo
        const escapedYaml = yamlStr.replace(/'/g, "'\\''");
        
        if (isDocker) {
          // Docker requires creating the directory if it does not exist in the container (s02-enrich usually exists)
          await invoke('exec_custom_command', {
            cmd: `docker exec -i ${containerName} mkdir -p /etc/crowdsec/parsers/s02-enrich`,
            useSudo: true
          });
          
          await invoke('exec_custom_command', {
            cmd: `echo '${escapedYaml}' | docker exec -i ${containerName} tee ${path}`,
            useSudo: true
          });

          // Reload CrowdSec in Docker - send SIGHUP to process 1
          try {
            await invoke('exec_custom_command', {
              cmd: `docker exec -i ${containerName} kill -HUP 1`,
              useSudo: true
            });
          } catch (reloadErr) {
            // If kill -HUP fails, restart the container
            await invoke('exec_custom_command', {
              cmd: `docker restart ${containerName}`,
              useSudo: true
            });
          }
        } else {
          await invoke('exec_custom_command', {
            cmd: `mkdir -p /etc/crowdsec/parsers/s02-enrich`,
            useSudo: true
          });

          await invoke('exec_custom_command', {
            cmd: `echo '${escapedYaml}' | sudo tee ${path}`,
            useSudo: true
          });

          // Systemctl reload crowdsec
          await invoke('exec_custom_command', {
            cmd: `systemctl reload crowdsec`,
            useSudo: true
          });
        }
        
        whitelistData = updatedData;
        await fetchWhitelist();
      } catch (err: any) {
        if (isSudoPasswordRequired(err) || isSudoPasswordIncorrect(err)) {
          throw err;
        }
        errorMsg = `Whitelist save error: ${formatInvokeError(err)}`;
      } finally {
        isLoading = false;
      }
    };

    await handleActionWithSudo(run);
  }

  // Add element to whitelist (YAML or LAPI)
  async function addWhitelistItem() {
    if (!newWhitelistIp) return;
    const targetIp = newWhitelistIp.trim();
    
    if (whitelistTarget.startsWith('lapi:')) {
      const allowlistName = whitelistTarget.substring(5);
      const run = async () => {
        isLoading = true;
        errorMsg = '';
        try {
          let usingSubcommand = 'allowlists';
          try {
            await runCscliCommand(`allowlists add ${allowlistName} ${targetIp}`, true);
          } catch (e) {
            await runCscliCommand(`allowlist add ${allowlistName} ${targetIp}`, true);
          }
          
          if (newWhitelistType === 'ip') {
            try {
              await runCscliCommand(`decisions delete --ip ${targetIp}`, true);
              await fetchDecisions();
            } catch (unbanErr) {
              // Ignore unban errors (if there wasn't one)
            }
          }
          
          newWhitelistIp = '';
          await fetchWhitelist();
        } catch (err: any) {
          if (isSudoPasswordRequired(err) || isSudoPasswordIncorrect(err)) {
            throw err;
          }
          errorMsg = `LAPI allowlist add error: ${formatInvokeError(err)}`;
        } finally {
          isLoading = false;
        }
      };
      await handleActionWithSudo(run);
      return;
    }
    
    const updated = { ...whitelistData };
    if (newWhitelistType === 'ip') {
      if (!updated.ip.includes(targetIp)) {
        updated.ip.push(targetIp);
      }
    } else {
      if (!updated.cidr.includes(targetIp)) {
        updated.cidr.push(targetIp);
      }
    }

    const run = async () => {
      // 1. Save whitelist
      await saveWhitelist(updated);
      
      // 2. Automatically remove active ban from this IP (immediate expiration)
      if (newWhitelistType === 'ip') {
        try {
          await runCscliCommand(`decisions delete --ip ${targetIp}`, true);
          await fetchDecisions();
        } catch (unbanErr) {
          // Ignore unban errors (if it was not in the database)
        }
      }
      
      newWhitelistIp = '';
    };

    await handleActionWithSudo(run);
  }

  // Remove element from whitelist (YAML or LAPI)
  async function removeWhitelistItem(item: WhitelistItem) {
    if (item.isSystem) return;
    
    if (item.file.startsWith('LAPI: ')) {
      const allowlistName = item.file.substring(6);
      if (!confirm("Confirm delete" + ' ' + item.value)) return;

      const run = async () => {
        isLoading = true;
        errorMsg = '';
        try {
          let usingSubcommand = 'allowlists';
          try {
            await runCscliCommand(`allowlists remove ${allowlistName} ${item.value}`, true);
          } catch (e) {
            await runCscliCommand(`allowlist remove ${allowlistName} ${item.value}`, true);
          }
          await fetchWhitelist();
        } catch (err: any) {
          if (isSudoPasswordRequired(err) || isSudoPasswordIncorrect(err)) {
            throw err;
          }
          errorMsg = `LAPI allowlist remove error: ${formatInvokeError(err)}`;
        } finally {
          isLoading = false;
        }
      };
      await handleActionWithSudo(run);
      return;
    }

    if (!confirm("Confirm delete" + ' ' + item.value)) return;
    
    const isDocker = connectionMode === 'docker' || (connectionMode === 'auto' && detectedMode === 'docker');
    const path = `/etc/crowdsec/parsers/s02-enrich/${item.file}`;

    const run = async () => {
      isLoading = true;
      errorMsg = '';
      try {
        let content = '';
        if (isDocker) {
          content = await invoke<string>('exec_custom_command', {
            cmd: `docker exec -i ${containerName} cat ${path}`,
            useSudo: true
          });
        } else {
          content = await invoke<string>('exec_custom_command', {
            cmd: `cat ${path}`,
            useSudo: true
          });
        }

        const parsed: any = yaml.load(content);
        if (parsed && parsed.whitelist) {
          if (item.type === 'ip' && Array.isArray(parsed.whitelist.ip)) {
            parsed.whitelist.ip = parsed.whitelist.ip.filter((x: string) => x !== item.value);
          } else if (item.type === 'cidr' && Array.isArray(parsed.whitelist.cidr)) {
            parsed.whitelist.cidr = parsed.whitelist.cidr.filter((x: string) => x !== item.value);
          }
          
          const yamlStr = yaml.dump(parsed);
          const escapedYaml = yamlStr.replace(/'/g, "'\\''");

          if (isDocker) {
            await invoke('exec_custom_command', {
              cmd: `echo '${escapedYaml}' | docker exec -i ${containerName} tee ${path}`,
              useSudo: true
            });
            try {
              await invoke('exec_custom_command', {
                cmd: `docker exec -i ${containerName} kill -HUP 1`,
                useSudo: true
              });
            } catch (e) {
              await invoke('exec_custom_command', { cmd: `docker restart ${containerName}`, useSudo: true });
            }
          } else {
            await invoke('exec_custom_command', {
              cmd: `echo '${escapedYaml}' | sudo tee ${path}`,
              useSudo: true
            });
            await invoke('exec_custom_command', {
              cmd: `systemctl reload crowdsec`,
              useSudo: true
            });
          }
        }
        await fetchWhitelist();
      } catch (err: any) {
        if (isSudoPasswordRequired(err) || isSudoPasswordIncorrect(err)) {
          throw err;
        }
        errorMsg = `Whitelist removal error: ${formatInvokeError(err)}`;
      } finally {
        isLoading = false;
      }
    };

    await handleActionWithSudo(run);
  }

  // Fetch registered Bouncers
  async function fetchBouncers() {
    try {
      const out = await runCscliCommand('bouncers list -o json', true);
      const parsed = JSON.parse(out);
      bouncers = Array.isArray(parsed) ? parsed : [];
    } catch (err: any) {
      if (isSudoPasswordRequired(err) || isSudoPasswordIncorrect(err)) {
        throw err;
      }
      console.error('Error fetching bouncers:', err);
      bouncers = [];
    }
  }

  // Add a new bouncer and obtain API Key
  async function addBouncer() {
    if (!newBouncerName) return;
    const run = async () => {
      isLoading = true;
      errorMsg = '';
      generatedBouncerKey = '';
      try {
        const out = await runCscliCommand(`bouncers add ${newBouncerName} -o json`, true);
        const parsed = JSON.parse(out);
        
        // cscli returns a JSON object or array containing "api_key"
        if (parsed && parsed.api_key) {
          generatedBouncerKey = parsed.api_key;
        } else if (Array.isArray(parsed) && parsed[0] && parsed[0].api_key) {
          generatedBouncerKey = parsed[0].api_key;
        } else {
          // If for some reason the JSON does not contain the key and it is text output
          generatedBouncerKey = out;
        }
        
        showAddBouncerModal = false;
        showBouncerKeyModal = true;
        newBouncerName = '';
        await fetchBouncers();
      } catch (err: any) {
        if (isSudoPasswordRequired(err) || isSudoPasswordIncorrect(err)) {
          throw err;
        }
        errorMsg = `Failed to add bouncer: ${formatInvokeError(err)}`;
      } finally {
        isLoading = false;
      }
    };
    await handleActionWithSudo(run);
  }

  // Copy bouncer key
  function copyBouncerKey() {
    navigator.clipboard.writeText(generatedBouncerKey);
    alert("Copied");
  }

  // Remove bouncer
  async function deleteBouncer(name: string) {
    if (!confirm("Remove this bouncer" + ' "' + name + '"?')) return;
    const run = async () => {
      isLoading = true;
      errorMsg = '';
      try {
        await runCscliCommand(`bouncers delete ${name}`, true);
        await fetchBouncers();
      } catch (err: any) {
        if (isSudoPasswordRequired(err) || isSudoPasswordIncorrect(err)) {
          throw err;
        }
        errorMsg = `Action failed: ${formatInvokeError(err)}`;
      } finally {
        isLoading = false;
      }
    };
    await handleActionWithSudo(run);
  }

  // Pruning bouncers
  async function pruneBouncers() {
    if (!confirm("Clean up bouncers that have not communicated for over 45 minutes?")) return;
    const run = async () => {
      isLoading = true;
      errorMsg = '';
      try {
        await runCscliCommand('bouncers prune -d 45m', true);
        await fetchBouncers();
      } catch (err: any) {
        if (isSudoPasswordRequired(err) || isSudoPasswordIncorrect(err)) {
          throw err;
        }
        errorMsg = `Failed to clean up inactive bouncers: ${formatInvokeError(err)}`;
      } finally {
        isLoading = false;
      }
    };
    await handleActionWithSudo(run);
  }

  // Fetch alerts
  async function fetchAlerts() {
    try {
      const out = await runCscliCommand('alerts list -o json', true);
      const parsed = JSON.parse(out);
      alerts = Array.isArray(parsed) ? parsed : [];
    } catch (err: any) {
      if (isSudoPasswordRequired(err) || isSudoPasswordIncorrect(err)) {
        throw err;
      }
      console.error('Error fetching alerts:', err);
      alerts = [];
    }
  }

  // Fetch detailed metrics
  async function fetchMetrics() {
    try {
      const out = await runCscliCommand('metrics show -o json', true);
      metrics = JSON.parse(out);
    } catch (err: any) {
      if (isSudoPasswordRequired(err) || isSudoPasswordIncorrect(err)) {
        throw err;
      }
      console.error('Error fetching metrics:', err);
      metrics = null;
    }
  }

  // Fetch hub list
  async function fetchHub() {
    try {
      const out = await runCscliCommand('hub list -o json', true);
      const parsed = JSON.parse(out);
      
      // Parsing hub list from JSON:
      // Returns keys like 'parsers', 'scenarios', 'postoverflows', 'collections' as arrays
      let allItems: any[] = [];
      if (parsed) {
        const types = ['parsers', 'scenarios', 'postoverflows', 'collections'];
        for (const type of types) {
          if (Array.isArray(parsed[type])) {
            parsed[type].forEach((item: any) => {
              allItems.push({
                ...item,
                item_type: type.slice(0, -1) // parser, scenario, etc.
              });
            });
          }
        }
      }
      hubItems = allItems;
    } catch (err: any) {
      if (isSudoPasswordRequired(err) || isSudoPasswordIncorrect(err)) {
        throw err;
      }
      console.error('Error fetching hub list:', err);
      hubItems = [];
    }
  }

  // Run CrowdSec installation (Debian/Ubuntu)
  async function installCrowdSec() {
    const run = async () => {
      isInstalling = true;
      errorMsg = '';
      try {
        // Skrypt instalacyjny CrowdSec i bouncera firewall
        const cmd = 'curl -s https://install.crowdsec.net | sudo sh && sudo apt-get update && sudo apt-get install -y crowdsec crowdsec-firewall-bouncer-iptables';
        await invoke('exec_custom_command', { cmd, useSudo: true });
        
        // Attempt environment re-detection after installation
        await detectEnvironment();
      } catch (err: any) {
        if (isSudoPasswordRequired(err) || isSudoPasswordIncorrect(err)) {
          throw err;
        }
        errorMsg = `CrowdSec installation error: ${formatInvokeError(err)}`;
      } finally {
        isInstalling = false;
      }
    };
    
    await handleActionWithSudo(run);
  }

  // Connection test in settings
  let isTestingConnection = $state(false);
  let testConnectionResult = $state<{ success: boolean; msg: string } | null>(null);

  async function testConnection() {
    isTestingConnection = true;
    testConnectionResult = null;
    try {
      const verOut = await runCscliCommand('version', true);
      if (verOut.toLowerCase().includes('version') || verOut.includes('db')) {
        testConnectionResult = { success: true, msg: `Connection successful. CrowdSec version: ${verOut.split('\n')[0]}` };
      } else {
        testConnectionResult = { success: false, msg: `Unexpected response from CrowdSec: ${verOut.substring(0, 200)}` };
      }
    } catch (err: any) {
      testConnectionResult = { success: false, msg: `Connection test failed: ${formatInvokeError(err)}` };
    } finally {
      isTestingConnection = false;
    }
  }

  // Save settings
  function handleSaveSettings() {
    saveLocalConfig();
    showSettingsModal = false;
    testConnectionResult = null;
    detectEnvironment(); // Update environment state
  }

  // --- VIEW FILTERING AND SORTING ---

  // Sorting and filtering decisions
  const filteredDecisions = $derived(
    decisions.filter(d => {
      if (!searchDecisionQuery) return true;
      const q = searchDecisionQuery.toLowerCase();
      const reason = (d.reason || d.scenario || '').toLowerCase();
      return (
        (d.value && d.value.toLowerCase().includes(q)) ||
        (d.type && d.type.toLowerCase().includes(q)) ||
        (d.origin && d.origin.toLowerCase().includes(q)) ||
        (d.scope && d.scope.toLowerCase().includes(q)) ||
        (d.cn && d.cn.toLowerCase().includes(q)) ||
        (d.as_name && d.as_name.toLowerCase().includes(q)) ||
        reason.includes(q)
      );
    })
  );

  const sortedDecisions = $derived(
    applySort(filteredDecisions, decisionSort, {
      value: (d) => d.value || '',
      type: (d) => d.type || '',
      origin: (d) => d.origin || '',
      reason: (d) => d.reason || d.scenario || '',
      cn: (d) => d.cn || '',
      duration: (d) => d.duration || '',
      until: (d) => d.until || '',
    })
  );

  function setDecisionSort(column: string) {
    decisionSort = nextSort(decisionSort, column as DecisionSortCol);
  }

  // Sorting and filtering alerts
  const filteredAlerts = $derived(
    alerts.filter(a => {
      if (!searchAlertQuery) return true;
      const q = searchAlertQuery.toLowerCase();
      return (
        (a.source?.value && a.source.value.toLowerCase().includes(q)) ||
        (a.scenario && a.scenario.toLowerCase().includes(q)) ||
        (a.message && a.message.toLowerCase().includes(q))
      );
    })
  );

  const sortedAlerts = $derived(
    applySort(filteredAlerts, alertSort, {
      id: (a) => a.id || 0,
      source: (a) => a.source?.value || '',
      scenario: (a) => a.scenario || '',
      events_count: (a) => a.events_count || 0,
      created_at: (a) => a.created_at || '',
    })
  );

  function setAlertSort(column: string) {
    alertSort = nextSort(alertSort, column as AlertSortCol);
  }

  // Sorting and filtering CrowdSec Hub
  const filteredHubItems = $derived(
    hubItems.filter(h => {
      if (!searchHubQuery) return true;
      const q = searchHubQuery.toLowerCase();
      return (
        (h.name && h.name.toLowerCase().includes(q)) ||
        (h.item_type && h.item_type.toLowerCase().includes(q))
      );
    })
  );

  const sortedHubItems = $derived(
    applySort(filteredHubItems, hubSort, {
      type: (h) => h.item_type || '',
      name: (h) => h.name || '',
      status: (h) => (h.status === 'enabled' ? 1 : 0),
      version: (h) => h.local_version || '',
    })
  );

  function setHubSort(column: string) {
    hubSort = nextSort(hubSort, column as HubSortCol);
  }

  // --- CALCULATION STATISTICS (METRICS) ---

  // Safely extract logs list from metrics (Acquisition)
  const acquisitionList = $derived.by(() => {
    if (!metrics || !metrics.acquisition) return [];
    
    // CrowdSec versions may return acquisition as an array or map
    if (Array.isArray(metrics.acquisition)) {
      return metrics.acquisition.map((item: any) => ({
        source: item.source || "Unknown" + ' log',
        read: parseInt(item.read) || 0,
        parsed: parseInt(item.parsed) || 0,
        unparsed: parseInt(item.unparsed) || 0
      }));
    } else {
      const keys = Object.keys(metrics.acquisition);
      return keys.map(key => {
        const item = metrics.acquisition[key];
        return {
          source: key,
          read: parseInt(item.read) || 0,
          parsed: parseInt(item.parsed) || 0,
          unparsed: parseInt(item.unparsed) || 0
        };
      });
    }
  });

  // Aggregated log stats for the Dashboard widget
  const logStats = $derived.by(() => {
    let totalRead = 0;
    let totalParsed = 0;
    let totalUnparsed = 0;
    
    acquisitionList.forEach((item: any) => {
      totalRead += item.read;
      totalParsed += item.parsed;
      totalUnparsed += item.unparsed;
    });

    const successRate = totalRead > 0 ? Math.round((totalParsed / totalRead) * 100) : 100;

    return {
      read: totalRead,
      parsed: totalParsed,
      unparsed: totalUnparsed,
      successRate
    };
  });

  // Detecting current connection description in UI
  const connectionModeLabel = $derived.by(() => {
    if (connectionMode === 'auto') {
      const modeName = detectedMode === 'docker' ? 'Docker' : 'Systemd';
      return `Automatic (${modeName})`;
    }
    return connectionMode === 'docker' 
      ? "Docker container" 
      : "Bare-metal (Systemd)";
  });

  const configuredContainerName = $derived(
    (connectionMode === 'docker' || (connectionMode === 'auto' && detectedMode === 'docker')) ? containerName : ''
  );

  // Statistics computed from decisions and alerts
  const stats = $derived.by(() => {
    const scenarioCounts: Record<string, number> = {};
    const ipCounts: Record<string, number> = {};
    const countryCounts: Record<string, number> = {};
    const originCounts: Record<string, number> = {};

    // Process decisions
    decisions.forEach(d => {
      if (d.origin) {
        originCounts[d.origin] = (originCounts[d.origin] || 0) + 1;
      }
      const reason = d.reason || d.scenario || 'unknown';
      scenarioCounts[reason] = (scenarioCounts[reason] || 0) + 1;
      if (d.value) {
        ipCounts[d.value] = (ipCounts[d.value] || 0) + 1;
      }
      if (d.cn) {
        countryCounts[d.cn] = (countryCounts[d.cn] || 0) + 1;
      }
    });

    // Process alerts
    const alertScenarios: Record<string, number> = {};
    const alertIps: Record<string, number> = {};
    const alertCountries: Record<string, number> = {};

    alerts.forEach(a => {
      if (a.scenario) {
        alertScenarios[a.scenario] = (alertScenarios[a.scenario] || 0) + 1;
      }
      if (a.source?.value) {
        alertIps[a.source.value] = (alertIps[a.source.value] || 0) + 1;
      }
      if (a.source?.cn) {
        alertCountries[a.source.cn] = (alertCountries[a.source.cn] || 0) + 1;
      }
    });

    const getTop = (record: Record<string, number>, limit = 5) => {
      return Object.entries(record)
        .map(([name, count]) => ({ name, count }))
        .sort((a, b) => b.count - a.count)
        .slice(0, limit);
    };

    return {
      topDecisionsScenarios: getTop(scenarioCounts),
      topDecisionsIps: getTop(ipCounts),
      topDecisionsCountries: getTop(countryCounts),
      decisionsOrigins: Object.entries(originCounts).map(([name, count]) => ({ name, count })),
      
      topAlertScenarios: getTop(alertScenarios),
      topAlertIps: getTop(alertIps),
      topAlertCountries: getTop(alertCountries),
    };
  });

  // Country Flag Emoji Helper
  function getFlagEmoji(countryCode: string): string {
    if (!countryCode || countryCode.length !== 2) return '🏳️';
    const codePoints = countryCode
      .toUpperCase()
      .split('')
      .map(char => 127397 + char.charCodeAt(0));
    try {
      return String.fromCodePoint(...codePoints);
    } catch {
      return '🏳️';
    }
  }

  // Metric Log Viewer functions
  async function fetchMetricLogContent() {
    if (!selectedMetricLogPath) return;
    isMetricLogLoading = true;
    try {
      const cmd = `tail -n 150 "${selectedMetricLogPath}"`;
      const output = await runCscliCommand(cmd, true);
      metricLogContent = output;
    } catch (err: any) {
      if (isSudoPasswordRequired(err) || isSudoPasswordIncorrect(err)) {
        pendingAction = fetchMetricLogContent;
        showSudoModal = true;
        isMetricLogStreaming = false;
      } else {
        metricLogContent = `Error loading log file: ${formatInvokeError(err)}`;
      }
    } finally {
      isMetricLogLoading = false;
    }
  }

  function startMetricLogStreaming() {
    isMetricLogStreaming = true;
    fetchMetricLogContent();
    if (metricLogIntervalId) clearInterval(metricLogIntervalId);
    // Skip the tick while this pane is hidden (kept alive) to avoid wasted SSH calls.
    metricLogIntervalId = setInterval(() => { if (visible) fetchMetricLogContent(); }, 3000);
  }

  function stopMetricLogStreaming() {
    isMetricLogStreaming = false;
    if (metricLogIntervalId) {
      clearInterval(metricLogIntervalId);
      metricLogIntervalId = null;
    }
  }

  function closeMetricLogViewer() {
    stopMetricLogStreaming();
    selectedMetricLogPath = null;
    metricLogContent = '';
    metricLogSearchQuery = '';
  }

  const filteredMetricLogs = $derived.by(() => {
    if (!metricLogSearchQuery) return metricLogContent;
    const q = metricLogSearchQuery.toLowerCase();
    return metricLogContent
      .split('\n')
      .filter(line => line.toLowerCase().includes(q))
      .join('\n');
  });

  // Handle auto streaming lifecycle
  $effect(() => {
    if (selectedMetricLogPath) {
      startMetricLogStreaming();
    } else {
      stopMetricLogStreaming();
    }
    return () => stopMetricLogStreaming();
  });

  onMount(() => {
    initCrowdsec();
  });

  $effect(() => {
    if (profileId) {
      untrack(() => {
        initCrowdsec();
      });
    }
  });
</script>

<div class="crowdsec-manager manager-shell fade-in">
  <header class="manager-header">
    <div class="header-title-section">
      <h1 class="page-title">Security (CrowdSec)</h1>
      {#if isSudoAuthorized && isInstalled}
        <span class="status-pill {isServiceActive === null ? 'unknown' : (isServiceActive ? 'active' : 'inactive')}">
          <span class="status-dot"></span>
          {#if isServiceActive === null}
            CHECKING...
          {:else}
            {isServiceActive ? "ACTIVE" : "INACTIVE"}
          {/if}
          ({connectionModeLabel})
        </span>
      {/if}
    </div>

    <div class="header-actions">
      <button class="secondary btn-sm" onclick={() => { showSettingsModal = true; testConnectionResult = null; }}>
        <Settings size={14} /> Settings
      </button>
    </div>
  </header>

  {#if !isSudoAuthorized}
    <div class="auth-gate fade-in">
      <div class="auth-gate-card glass">
        <div class="auth-gate-icon">
          <KeyRound size={40} class="accent-amber-text" />
        </div>
        <h2>Sudo authentication required</h2>
        <p>
          The CrowdSec tab requires a sudo password to communicate with the server. No background operations will run until you provide it.
        </p>
        <button class="primary" onclick={requestSudoAuth}>
          <KeyRound size={16} /> Enter sudo password
        </button>
      </div>
    </div>
  {:else if isInstalled === null}
    <!-- Loading / detection state -->
    <div class="loading-state">
      <RefreshCw size={36} class="spin muted-icon" />
      <p>Detecting CrowdSec environment…</p>
    </div>
  {:else if isInstalled === false}
    <!-- Onboarding screen / Installer -->
    <div class="onboarding-screen fade-in">
      <div class="onboarding-card glass">
        <div class="onboarding-icon-box">
          <ShieldAlert size={42} class="accent-amber-text" />
        </div>
        <h2>Secure your server with CrowdSec</h2>
        <p class="onboarding-desc">
          CrowdSec is a modern collaborative IPS. It analyzes server logs for intrusion attempts, port scans and malicious activity, blocks attackers and shares their addresses with a global threat database.
        </p>

        <div class="features-grid">
          <div class="feat-card">
            <Activity size={20} class="accent-amber-text" />
            <h4>Log analysis</h4>
            <p>Monitors system and service logs (SSH, Web, Mail) in real time.</p>
          </div>
          <div class="feat-card">
            <Users size={20} class="accent-green-text" />
            <h4>IP reputation network</h4>
            <p>Downloads and updates a database of malicious IPs verified by the CrowdSec community.</p>
          </div>
          <div class="feat-card">
            <Shield size={20} class="accent-red-text" />
            <h4>Bouncers (blocks)</h4>
            <p>Automatically integrates with the server firewall (e.g. UFW/iptables), rejecting attacks.</p>
          </div>
        </div>

        <div class="divider"></div>

        <h3>How do you want to configure CrowdSec?</h3>

        <div class="install-options">
          <div class="install-box">
            <h4>Method 1: Direct installation (Bare-metal)</h4>
            <p>Installs CrowdSec and an associated firewall bouncer (iptables) directly on the server (requires APT-based distro, e.g. Debian/Ubuntu).</p>
            <button class="primary" onclick={installCrowdSec} disabled={isInstalling}>
              {#if isInstalling}
                <RefreshCw size={16} class="spin" /> Installing…
              {:else}
                <Plus size={16} /> Install directly
              {/if}
            </button>
          </div>

          <div class="install-box">
            <h4>Method 2: Run in Docker</h4>
            <p>If you prefer Docker, run CrowdSec in a container. Ensure the container is named crowdsec or open settings to specify another name.</p>
            <div class="code-preview">
              <pre><code>version: "3"
services:
  crowdsec:
    image: crowdsecurity/crowdsec
    container_name: crowdsec
    environment:
      - COLLECTIONS=crowdsecurity/linux
    volumes:
      - /var/log:/var/log:ro
      - crowdsec-db:/var/lib/crowdsec/data
      - crowdsec-config:/etc/crowdsec</code></pre>
            </div>
          </div>
        </div>

        <div class="onboarding-actions">
          <button class="secondary" onclick={() => { showSettingsModal = true; testConnectionResult = null; }}>
            Configure connection manually
          </button>
        </div>
      </div>
    </div>
  {:else}
    <!-- Main panel with sub-tabs -->
    <div class="tabs-bar glass">
      <button class="tab-btn {activeSubTab === 'dashboard' ? 'active' : ''}" onclick={() => activeSubTab = 'dashboard'}>
        <Activity size={16} /> Overview
      </button>
      <button class="tab-btn {activeSubTab === 'decisions' ? 'active' : ''}" onclick={() => activeSubTab = 'decisions'}>
        <Shield size={16} /> Decisions (Bans)
      </button>
      <button class="tab-btn {activeSubTab === 'whitelist' ? 'active' : ''}" onclick={() => activeSubTab = 'whitelist'}>
        <Check size={16} /> Whitelist
      </button>
      <button class="tab-btn {activeSubTab === 'alerts' ? 'active' : ''}" onclick={() => activeSubTab = 'alerts'}>
        <ShieldAlert size={16} /> Alerts
      </button>
      <button class="tab-btn {activeSubTab === 'bouncers' ? 'active' : ''}" onclick={() => activeSubTab = 'bouncers'}>
        <Box size={16} /> Bouncers
      </button>
      <button class="tab-btn {activeSubTab === 'metrics' ? 'active' : ''}" onclick={() => activeSubTab = 'metrics'}>
        <FileText size={16} /> Log metrics
      </button>
      <button class="tab-btn {activeSubTab === 'hub' ? 'active' : ''}" onclick={() => activeSubTab = 'hub'}>
        <Cpu size={16} /> CrowdSec Hub
      </button>
    </div>

    <div class="tab-content">
      {#if activeSubTab === 'dashboard'}
        <!-- DASHBOARD WIDGETS -->
        <div class="dashboard-grid fade-in">
          <!-- Service status widget -->
          <div class="dash-card glass service-status-card">
            <h3>Service status</h3>
            <div class="status-indicator-box">
              <div class="status-dot-large {isServiceActive === null ? 'unknown' : (isServiceActive ? 'active' : 'inactive')}"></div>
              <div class="status-details">
                <span class="status-text">
                  {#if isServiceActive === null}
                    Checking status…
                  {:else}
                    {isServiceActive ? "Running (Online)" : "Stopped (Offline)"}
                  {/if}
                </span>
                <span class="ver-text">
                  {#if lapiVersion}
                    {`LAPI version: ${lapiVersion}`}
                  {:else}
                    Checking status…
                  {/if}
                </span>
              </div>
            </div>
            <div class="service-actions">
              {#if isServiceActive}
                <button class="danger btn-sm" onclick={() => controlService('stop')} disabled={isLoading}>
                  <Square size={14} /> Stop
                </button>
              {:else}
                <button class="primary btn-sm" onclick={() => controlService('start')} disabled={isLoading || isServiceActive === null}>
                  <Play size={14} /> Start
                </button>
              {/if}
              <button class="secondary btn-sm" onclick={() => controlService('restart')} disabled={isLoading || isServiceActive === null}>
                <RotateCw size={14} /> Restart
              </button>
            </div>
          </div>

          <!-- Widget Statystyk telemetrycznych -->
          <div class="dash-card glass stats-overview-card">
            <h3>Database summary</h3>
            <div class="stats-grid">
              <button class="stat-item clickable" onclick={() => activeSubTab = 'decisions'} style="background: transparent; border: 1px solid var(--border-color); text-align: center; width: 100%;">
                <span class="stat-num mono-stats">{decisions.length}</span>
                <span class="stat-label">Active bans</span>
              </button>
              <button class="stat-item clickable" onclick={() => activeSubTab = 'bouncers'} style="background: transparent; border: 1px solid var(--border-color); text-align: center; width: 100%;">
                <span class="stat-num mono-stats">{bouncers.length}</span>
                <span class="stat-label">Bouncers</span>
              </button>
              <button class="stat-item clickable" onclick={() => activeSubTab = 'alerts'} style="background: transparent; border: 1px solid var(--border-color); text-align: center; width: 100%;">
                <span class="stat-num mono-stats">{alerts.length}</span>
                <span class="stat-label">Alert history</span>
              </button>
              <button class="stat-item clickable" onclick={() => activeSubTab = 'whitelist'} style="background: transparent; border: 1px solid var(--border-color); text-align: center; width: 100%;">
                <span class="stat-num mono-stats">{whitelistData.ip.length + whitelistData.cidr.length}</span>
                <span class="stat-label">Whitelist</span>
              </button>
            </div>
          </div>

          <!-- Log processing statistics widget -->
          <div class="dash-card glass log-metrics-card">
            <h3>Log processing</h3>
            <div class="log-progress-section">
              <div class="progress-details">
                <span class="progress-label">Parsing success rate</span>
                <span class="progress-percentage mono-stats">{logStats.successRate}%</span>
              </div>
              <div class="progress-bar-container">
                <div class="progress-bar-fill" style="width: {logStats.successRate}%"></div>
              </div>
              <div class="progress-legend">
                <span class="leg-item"><span class="dot success"></span> Parsed: <strong class="mono-stats">{logStats.parsed}</strong></span>
                <span class="leg-item"><span class="dot danger"></span> Skipped: <strong class="mono-stats">{logStats.unparsed}</strong></span>
                <span class="leg-item">Total: <strong class="mono-stats">{logStats.read}</strong></span>
              </div>
            </div>
          </div>

          <!-- Threat Intelligence & Statistics Section -->
          <div class="threat-intel-section" style="grid-column: span 2; margin-top: 12px; display: flex; flex-direction: column; gap: 16px;">
            <div style="display: flex; align-items: center; gap: 8px;">
              <Activity size={18} class="accent-amber-text" />
              <h3 style="font-size: 1.05rem; font-weight: 600; color: white; margin: 0;">Threat Intelligence & Telemetry</h3>
            </div>
            
            <div class="stats-details-grid" style="display: grid; grid-template-columns: repeat(2, 1fr); gap: 20px;">
              <!-- Top Alert Scenarios -->
              <div class="dash-card glass">
                <h4 style="font-size: 0.9rem; color: white; border-bottom: 1px solid var(--border-color); padding-bottom: 8px; margin: 0 0 10px 0; font-weight: 500;">Top Scenario Alerts</h4>
                <div class="stats-list" style="display: flex; flex-direction: column; gap: 12px;">
                  {#each stats.topAlertScenarios as item}
                    {@const maxCount = Math.max(...stats.topAlertScenarios.map(s => s.count), 1)}
                    {@const percentage = (item.count / maxCount) * 100}
                    <div class="stats-row" style="display: flex; flex-direction: column; gap: 4px;">
                      <div class="stats-row-header" style="display: flex; justify-content: space-between; align-items: center; font-size: 0.82rem;">
                        <span class="stats-row-name font-bold text-secondary" style="word-break: break-all;" title={item.name}>{item.name.replace('crowdsecurity/', '')}</span>
                        <span class="stats-row-value mono-stats font-bold">{item.count}</span>
                      </div>
                      <div class="progress-bar-container compact">
                        <div class="progress-bar-fill" style="width: {percentage}%"></div>
                      </div>
                    </div>
                  {/each}
                  {#if stats.topAlertScenarios.length === 0}
                    <span class="text-muted text-xs italic">No scenario alerts registered.</span>
                  {/if}
                </div>
              </div>

              <!-- Top Alert IPs -->
              <div class="dash-card glass">
                <h4 style="font-size: 0.9rem; color: white; border-bottom: 1px solid var(--border-color); padding-bottom: 8px; margin: 0 0 10px 0; font-weight: 500;">Top Active Attacking IPs</h4>
                <div class="stats-list" style="display: flex; flex-direction: column; gap: 12px;">
                  {#each stats.topAlertIps as item}
                    {@const maxCount = Math.max(...stats.topAlertIps.map(s => s.count), 1)}
                    {@const percentage = (item.count / maxCount) * 100}
                    <div class="stats-row" style="display: flex; flex-direction: column; gap: 4px;">
                      <div class="stats-row-header" style="display: flex; justify-content: space-between; align-items: center; font-size: 0.82rem;">
                        <span class="stats-row-name font-bold text-secondary"><code>{item.name}</code></span>
                        <span class="stats-row-value mono-stats font-bold">{item.count} alerts</span>
                      </div>
                      <div class="progress-bar-container compact">
                        <div class="progress-bar-fill" style="width: {percentage}%"></div>
                      </div>
                    </div>
                  {/each}
                  {#if stats.topAlertIps.length === 0}
                    <span class="text-muted text-xs italic">No attacker IP data available.</span>
                  {/if}
                </div>
              </div>

              <!-- Top Alert Countries -->
              <div class="dash-card glass">
                <h4 style="font-size: 0.9rem; color: white; border-bottom: 1px solid var(--border-color); padding-bottom: 8px; margin: 0 0 10px 0; font-weight: 500;">Top Attack Country Sources</h4>
                <div class="stats-list" style="display: flex; flex-direction: column; gap: 12px;">
                  {#each stats.topAlertCountries.length > 0 ? stats.topAlertCountries : stats.topDecisionsCountries as item}
                    {@const maxCount = Math.max(...(stats.topAlertCountries.length > 0 ? stats.topAlertCountries : stats.topDecisionsCountries).map(s => s.count), 1)}
                    {@const percentage = (item.count / maxCount) * 100}
                    <div class="stats-row" style="display: flex; flex-direction: column; gap: 4px;">
                      <div class="stats-row-header" style="display: flex; justify-content: space-between; align-items: center; font-size: 0.82rem;">
                        <span class="stats-row-name font-bold text-secondary" style="display: inline-flex; align-items: center; gap: 6px;">
                          <span>{getFlagEmoji(item.name)}</span> {item.name}
                        </span>
                        <span class="stats-row-value mono-stats font-bold">{item.count} events</span>
                      </div>
                      <div class="progress-bar-container compact">
                        <div class="progress-bar-fill" style="width: {percentage}%"></div>
                      </div>
                    </div>
                  {/each}
                  {#if stats.topAlertCountries.length === 0 && stats.topDecisionsCountries.length === 0}
                    <span class="text-muted text-xs italic">No country data available.</span>
                  {/if}
                </div>
              </div>

              <!-- Decisions by Origin -->
              <div class="dash-card glass">
                <h4 style="font-size: 0.9rem; color: white; border-bottom: 1px solid var(--border-color); padding-bottom: 8px; margin: 0 0 10px 0; font-weight: 500;">Ban Origin Sources</h4>
                <div class="stats-list" style="display: flex; flex-direction: column; gap: 12px;">
                  {#each stats.decisionsOrigins as item}
                    {@const maxCount = Math.max(...stats.decisionsOrigins.map(s => s.count), 1)}
                    {@const percentage = (item.count / maxCount) * 100}
                    <div class="stats-row" style="display: flex; flex-direction: column; gap: 4px;">
                      <div class="stats-row-header" style="display: flex; justify-content: space-between; align-items: center; font-size: 0.82rem;">
                        <span class="stats-row-name font-bold text-secondary">{item.name.toUpperCase()}</span>
                        <span class="stats-row-value mono-stats font-bold">{item.count}</span>
                      </div>
                      <div class="progress-bar-container compact">
                        <div class="progress-bar-fill" style="width: {percentage}%"></div>
                      </div>
                    </div>
                  {/each}
                  {#if stats.decisionsOrigins.length === 0}
                    <span class="text-muted text-xs italic">No active bans in the database.</span>
                  {/if}
                </div>
              </div>
            </div>
          </div>
        </div>

      {:else if activeSubTab === 'decisions'}
        <!-- DECISION MANAGER (BANY) -->
        <div class="sub-tab-panel fade-in">
          <div class="panel-header">
            <div class="search-box">
              <input type="text" placeholder="Search by IP or reason…" bind:value={searchDecisionQuery} />
            </div>
            <div class="panel-actions">
              <button class="primary" onclick={() => showAddDecisionModal = true}>
                <Plus size={16} /> Add ban
              </button>
              <button class="danger" onclick={deleteAllDecisions} disabled={decisions.length === 0}>
                <Trash2 size={16} /> Remove all bans
              </button>
            </div>
          </div>

          <div class="table-container glass">
            <table>
              <thead>
                <tr>
                  <SortableTh label="IP / range" column="value" activeColumn={decisionSort.column} direction={decisionSort.direction} onsort={setDecisionSort} width="15%" />
                  <SortableTh label="Type" column="type" activeColumn={decisionSort.column} direction={decisionSort.direction} onsort={setDecisionSort} width="10%" />
                  <SortableTh label="Origin" column="origin" activeColumn={decisionSort.column} direction={decisionSort.direction} onsort={setDecisionSort} width="10%" />
                  <SortableTh label="Reason" column="reason" activeColumn={decisionSort.column} direction={decisionSort.direction} onsort={setDecisionSort} width="25%" />
                  <SortableTh label="Country / AS" column="cn" activeColumn={decisionSort.column} direction={decisionSort.direction} onsort={setDecisionSort} width="20%" />
                  <SortableTh label="Duration" column="duration" activeColumn={decisionSort.column} direction={decisionSort.direction} onsort={setDecisionSort} width="10%" />
                  <SortableTh label="Until" column="until" activeColumn={decisionSort.column} direction={decisionSort.direction} onsort={setDecisionSort} width="10%" />
                  <th style="width: 5%; text-align: right;">Unban</th>
                </tr>
              </thead>
              <tbody>
                {#each sortedDecisions as dec}
                  <tr>
                    <td class="mono-stats font-bold">
                      <span class="scope-tag" style="font-size: 0.7rem; color: var(--text-muted); margin-right: 4px;">[{dec.scope || 'IP'}]</span>
                      <code>{dec.value}</code>
                    </td>
                    <td><span class="badge {(dec.type || '').toLowerCase() === 'ban' ? 'danger' : 'warning'}">{(dec.type || '').toUpperCase()}</span></td>
                    <td class="text-secondary mono-stats" style="font-size: 0.8rem;">{dec.origin || 'unknown'}</td>
                    <td class="text-secondary" style="font-size: 0.8rem;" title={dec.reason || dec.scenario}>{dec.reason || dec.scenario || 'manual'}</td>
                    <td class="text-secondary" style="font-size: 0.8rem;">
                      {#if dec.cn}
                        <span class="country-flag" style="margin-right: 4px;" title={dec.cn}>{getFlagEmoji(dec.cn)} {dec.cn}</span>
                      {/if}
                      {#if dec.as_name}
                        <span class="as-info" style="font-size: 0.72rem; color: var(--text-muted);" title={`${dec.as_number ? 'AS' + dec.as_number : ''} - ${dec.as_name}`}>
                          ({dec.as_name.substring(0, 15)}{dec.as_name.length > 15 ? '...' : ''})
                        </span>
                      {/if}
                    </td>
                    <td class="mono-stats">{dec.duration}</td>
                    <td class="mono-stats text-muted" title={dec.until}>{dec.until ? formatDate(dec.until) : "(no data)"}</td>
                    <td style="text-align: right;">
                      <button class="btn-table danger-text" onclick={() => deleteDecision(dec.value)} title="Unban">
                        <Trash2 size={14} />
                      </button>
                    </td>
                  </tr>
                {/each}

                {#if sortedDecisions.length === 0}
                  <tr>
                    <td colspan="8" class="empty-state">No active blocks (bans). Your server is safe.</td>
                  </tr>
                {/if}
              </tbody>
            </table>
          </div>
        </div>

      {:else if activeSubTab === 'whitelist'}
        <!-- WHITELIST MANAGER -->
        <div class="sub-tab-panel fade-in">
          <div class="whitelist-container">
            <div class="whitelist-form-box glass">
              <h3>Add to whitelist</h3>
              <p class="form-desc">Adding an IP or CIDR subnet will make CrowdSec ignore all their activity. Saves to selected YAML file or LAPI allowlist. Active bans for that IP are removed automatically.</p>
              
              <div class="form-group">
                <label for="wl-target">Save location</label>
                <select id="wl-target" bind:value={whitelistTarget}>
                  <option value="yaml">Local file (jarvis-whitelist.yaml)</option>
                  {#each lapiAllowlists as name}
                    <option value="lapi:{name}">{`LAPI Allowlist: ${name}`}</option>
                  {/each}
                </select>
              </div>

              <div class="form-group">
                <label for="wl-type">Entry type</label>
                <select id="wl-type" bind:value={newWhitelistType}>
                  <option value="ip">Single IP (e.g. 192.168.1.50)</option>
                  <option value="cidr">CIDR subnet (e.g. 192.168.1.0/24)</option>
                </select>
              </div>

              <div class="form-group">
                <label for="wl-ip">IP address or subnet</label>
                <input id="wl-ip" type="text" placeholder={newWhitelistType === 'ip' ? '8.8.8.8' : '10.0.0.0/24'} bind:value={newWhitelistIp} />
              </div>

              <button class="primary" onclick={addWhitelistItem} disabled={!newWhitelistIp || isLoading}>
                <Plus size={16} /> Add to whitelist
              </button>
            </div>

            <div class="whitelist-list-box glass" style="display: flex; flex-direction: column;">
              <h3>Active exclusions (whitelist)</h3>
              
              <div class="table-container" style="border: none; box-shadow: none; padding: 0; overflow-y: auto; max-height: 400px; margin-top: 10px;">
                <table>
                  <thead>
                    <tr>
                      <th>IP address / subnet</th>
                      <th>Type</th>
                      <th>Source</th>
                      <th style="width: 5%; text-align: right;">Remove</th>
                    </tr>
                  </thead>
                  <tbody>
                    {#each whitelistItems as item}
                      <tr>
                        <td class="mono-stats font-bold"><code>{item.value}</code></td>
                        <td>
                          <span class="badge {item.type === 'ip' ? 'success' : 'warning'}">
                            {item.type.toUpperCase()}
                          </span>
                        </td>
                        <td>
                          <span class="file-source" title={item.file.startsWith('LAPI: ') ? 'LAPI Allowlist' : `/etc/crowdsec/parsers/s02-enrich/${item.file}`}>
                            {item.file}
                            {#if item.isSystem}
                              <span class="tag-system">System</span>
                            {:else if item.file === 'jarvis-whitelist.yaml'}
                              <span class="tag-managed">Jarvis</span>
                            {:else if item.file.startsWith('LAPI: ')}
                              <span class="tag-lapi">LAPI</span>
                            {:else}
                              <span class="tag-custom">Custom</span>
                            {/if}
                          </span>
                        </td>
                        <td style="text-align: right;">
                          {#if item.isSystem}
                            <button class="btn-table" disabled title="System file - read-only" style="opacity: 0.3; cursor: not-allowed;">
                              <Trash2 size={14} />
                            </button>
                          {:else}
                            <button class="btn-table danger-text" onclick={() => removeWhitelistItem(item)} title="Remove">
                              <Trash2 size={14} />
                            </button>
                          {/if}
                        </td>
                      </tr>
                    {/each}

                    {#if whitelistItems.length === 0}
                      <tr>
                        <td colspan="4" class="empty-state">No active whitelist exclusions.</td>
                      </tr>
                    {/if}
                  </tbody>
                </table>
              </div>
            </div>
          </div>
        </div>

      {:else if activeSubTab === 'alerts'}
        <!-- ALERTS LOG (INCIDENT HISTORY) -->
        <div class="sub-tab-panel fade-in">
          <div class="panel-header">
            <div class="search-box">
              <input type="text" placeholder="Search alerts by IP or scenario…" bind:value={searchAlertQuery} />
            </div>
          </div>

          <div class="table-container glass">
            <table>
              <thead>
                <tr>
                  <SortableTh label="ID" column="id" activeColumn={alertSort.column} direction={alertSort.direction} onsort={setAlertSort} width="10%" />
                  <SortableTh label="Source" column="source" activeColumn={alertSort.column} direction={alertSort.direction} onsort={setAlertSort} width="20%" />
                  <SortableTh label="Scenario" column="scenario" activeColumn={alertSort.column} direction={alertSort.direction} onsort={setAlertSort} width="35%" />
                  <SortableTh label="Events" column="events_count" activeColumn={alertSort.column} direction={alertSort.direction} onsort={setAlertSort} width="15%" />
                  <SortableTh label="Created" column="created_at" activeColumn={alertSort.column} direction={alertSort.direction} onsort={setAlertSort} width="15%" />
                  <th style="width: 5%; text-align: right;">Details</th>
                </tr>
              </thead>
              <tbody>
                {#each sortedAlerts as alert}
                  <tr>
                    <td class="mono-stats">{alert.id}</td>
                    <td class="mono-stats font-bold"><code>{alert.source?.value}</code></td>
                    <td class="text-secondary">{alert.scenario}</td>
                    <td class="mono-stats">{alert.events_count}</td>
                    <td class="mono-stats text-muted">{formatDate(alert.created_at)}</td>
                    <td style="text-align: right;">
                      <button class="btn-table" onclick={() => selectedAlert = alert} title="Show alert details">
                        <Info size={14} />
                      </button>
                    </td>
                  </tr>
                {/each}

                {#if sortedAlerts.length === 0}
                  <tr>
                    <td colspan="6" class="empty-state">No alert history. Server logs are clean.</td>
                  </tr>
                {/if}
              </tbody>
            </table>
          </div>
        </div>

      {:else if activeSubTab === 'bouncers'}
        <!-- BOUNCERS REGISTRY -->
        <div class="sub-tab-panel fade-in">
          <div class="panel-header">
            <div class="search-box">
              <span class="text-secondary text-sm">List of bouncers (agents receiving decisions and applying firewall blocks).</span>
            </div>
            <div class="panel-actions">
              <button class="primary" onclick={() => showAddBouncerModal = true}>
                <Plus size={16} /> Register bouncer
              </button>
              <button class="secondary" onclick={pruneBouncers} disabled={bouncers.length === 0}>
                Clean up inactive
              </button>
            </div>
          </div>

          <div class="table-container glass">
            <table>
              <thead>
                <tr>
                  <th>Name</th>
                  <th>IP address</th>
                  <th>Type (engine)</th>
                  <th>Version</th>
                  <th>Last activity</th>
                  <th style="text-align: right;">Actions</th>
                </tr>
              </thead>
              <tbody>
                {#each bouncers as bouncer}
                  <tr>
                    <td class="font-bold">{bouncer.name}</td>
                    <td class="mono-stats"><code>{bouncer.ip_address || 'Lokalny / Unix socket'}</code></td>
                    <td><span class="badge warning">{bouncer.type || "(none)"}</span></td>
                    <td class="mono-stats">{bouncer.version || "(none)"}</td>
                    <td class="mono-stats text-muted">
                      {bouncer.last_pull ? formatDate(bouncer.last_pull) : "Never"}
                    </td>
                    <td style="text-align: right;">
                      <button class="btn-table danger-text" onclick={() => deleteBouncer(bouncer.name)} title="Remove this bouncer">
                        <Trash2 size={14} />
                      </button>
                    </td>
                  </tr>
                {/each}

                {#if bouncers.length === 0}
                  <tr>
                    <td colspan="6" class="empty-state">No registered bouncers. Without a bouncer, CrowdSec blocks are not physically applied to the firewall.</td>
                  </tr>
                {/if}
              </tbody>
            </table>
          </div>
        </div>

      {:else if activeSubTab === 'metrics'}
        <!-- LOG METRICS DETAILS -->
        <div class="sub-tab-panel fade-in">
          <div class="metrics-panel">
            <div class="table-container glass">
              <h3>Monitored log files (acquisition)</h3>
              <table style="margin-top: 10px;">
                <thead>
                  <tr>
                    <th>File location</th>
                    <th style="text-align: right;">Lines read</th>
                    <th style="text-align: right;">Lines parsed</th>
                    <th style="text-align: right;">Unparsed</th>
                    <th style="text-align: right; width: 200px;">Success rate</th>
                    <th style="text-align: right; width: 80px;">Logs</th>
                  </tr>
                </thead>
                <tbody>
                  {#each acquisitionList as item}
                    {@const successRate = item.read > 0 ? Math.round((item.parsed / item.read) * 100) : 100}
                    <tr>
                      <td class="mono-stats font-bold"><code>{item.source}</code></td>
                      <td class="mono-stats" style="text-align: right;">{item.read}</td>
                      <td class="mono-stats text-green" style="text-align: right;">{item.parsed}</td>
                      <td class="mono-stats text-red" style="text-align: right;">{item.unparsed}</td>
                      <td>
                        <div class="progress-details" style="margin-bottom: 2px;">
                          <span class="mono-stats text-sm">{successRate}%</span>
                        </div>
                        <div class="progress-bar-container compact">
                          <div class="progress-bar-fill" style="width: {successRate}%"></div>
                        </div>
                      </td>
                      <td style="text-align: right;">
                        <button class="btn-table" onclick={() => selectedMetricLogPath = item.source} title="Preview log file">
                          <FileText size={14} />
                        </button>
                      </td>
                    </tr>
                  {/each}

                  {#if acquisitionList.length === 0}
                    <tr>
                      <td colspan="6" class="empty-state">No metrics. No active log streams.</td>
                    </tr>
                  {/if}
                </tbody>
              </table>
            </div>
          </div>
        </div>

      {:else if activeSubTab === 'hub'}
        <!-- HUB list -->
        <div class="sub-tab-panel fade-in">
          <div class="panel-header">
            <div class="search-box">
              <input type="text" placeholder="Search parsers or scenarios…" bind:value={searchHubQuery} />
            </div>
          </div>

          <div class="table-container glass">
            <table>
              <thead>
                <tr>
                  <SortableTh label="Type" column="type" activeColumn={hubSort.column} direction={hubSort.direction} onsort={setHubSort} width="20%" />
                  <SortableTh label="Name" column="name" activeColumn={hubSort.column} direction={hubSort.direction} onsort={setHubSort} width="40%" />
                  <SortableTh label="Status" column="status" activeColumn={hubSort.column} direction={hubSort.direction} onsort={setHubSort} width="20%" />
                  <SortableTh label="Version" column="version" activeColumn={hubSort.column} direction={hubSort.direction} onsort={setHubSort} width="20%" />
                </tr>
              </thead>
              <tbody>
                {#each sortedHubItems as item}
                  <tr>
                    <td><span class="badge {item.item_type === 'scenario' ? 'warning' : 'success'}">{item.item_type.toUpperCase()}</span></td>
                    <td class="font-bold">{item.name}</td>
                    <td>
                      <span class="badge {item.status === 'enabled' ? 'success-glow' : 'muted'}">
                        {item.status === 'enabled' ? "ENABLED" : "DISABLED"}
                      </span>
                    </td>
                    <td class="mono-stats">{item.local_version || "(none)"}</td>
                  </tr>
                {/each}

                {#if sortedHubItems.length === 0}
                  <tr>
                    <td colspan="4" class="empty-state">No items fetched from CrowdSec Hub.</td>
                  </tr>
                {/if}
              </tbody>
            </table>
          </div>
        </div>
      {/if}
    </div>
  {/if}
</div>

<!-- ================= MODALS ================= -->

<!-- Connection settings modal -->
{#if showSettingsModal}
  <div class="modal-overlay">
    <div class="modal-content glass settings-modal fade-in">
      <h3>CrowdSec connection settings</h3>
      <p class="modal-desc">Configure how CrowdSec commands are invoked on this server. Settings are saved in this connection profile.</p>
      
      <div class="form-group">
        <label>Connection mode</label>
        <div class="radio-group">
          <label class="radio-label">
            <input type="radio" name="conn-mode" value="auto" bind:group={connectionMode} />
            <span>Auto-detect</span>
          </label>
          <label class="radio-label">
            <input type="radio" name="conn-mode" value="baremetal" bind:group={connectionMode} />
            <span>Native (Systemd / Bare-metal)</span>
          </label>
          <label class="radio-label">
            <input type="radio" name="conn-mode" value="docker" bind:group={connectionMode} />
            <span>Docker (inside container)</span>
          </label>
        </div>
      </div>

      {#if connectionMode === 'docker'}
        <div class="form-group">
          <label for="docker-name">Docker container name</label>
          <input id="docker-name" type="text" placeholder="crowdsec" bind:value={containerName} />
        </div>
      {/if}

      <div class="form-group">
        <label for="custom-prefix">Custom command prefix (optional — e.g. for K8s, Podman)</label>
        <input id="custom-prefix" type="text" placeholder="e.g. podman exec -i crowdsec" bind:value={customPrefix} />
      </div>

      <!-- Connection test result -->
      {#if testConnectionResult}
        <div class="test-result-box {testConnectionResult.success ? 'success' : 'error'}">
          <AlertCircle size={16} />
          <span>{testConnectionResult.msg}</span>
        </div>
      {/if}

      <div class="modal-actions">
        <button class="secondary" onclick={testConnection} disabled={isTestingConnection}>
          {#if isTestingConnection}
            <RefreshCw size={14} class="spin" /> Testing…
          {:else}
            Test connection
          {/if}
        </button>
        <button class="primary" onclick={handleSaveSettings}>Save settings</button>
        <button class="secondary" onclick={() => { showSettingsModal = false; testConnectionResult = null; }}>Cancel</button>
      </div>
    </div>
  </div>
{/if}

<!-- Modal Dodawania Decyzji (Banowania IP) -->
{#if showAddDecisionModal}
  <div class="modal-overlay">
    <div class="modal-content glass fade-in">
      <h3>Apply manual ban</h3>
      <p class="modal-desc">Enter details to manually block an IP or subnet in CrowdSec.</p>
      
      <div class="form-group">
        <label for="dec-scope">Block scope</label>
        <select id="dec-scope" bind:value={newDecisionScope}>
          <option value="ip">Single IP (e.g. 185.220.101.5)</option>
          <option value="range">CIDR subnet (e.g. 185.220.101.0/24)</option>
        </select>
      </div>

      <div class="form-group">
        <label for="dec-ip">IP address / range</label>
        <input id="dec-ip" type="text" placeholder="1.2.3.4" bind:value={newDecisionIp} />
      </div>

      <div class="form-group">
        <label for="dec-dur">Block duration</label>
        <select id="dec-dur" bind:value={newDecisionDuration}>
          <option value="4h">4 hours (default)</option>
          <option value="24h">24 hours</option>
          <option value="48h">48 hours</option>
          <option value="7d">7 days</option>
          <option value="1h">1 hour</option>
        </select>
      </div>

      <div class="form-group">
        <label for="dec-reason">Block reason</label>
        <input id="dec-reason" type="text" placeholder="Manual block by administrator" bind:value={newDecisionReason} />
      </div>

      <div class="modal-actions">
        <button class="primary" onclick={addDecision} disabled={!newDecisionIp || isLoading}>Add block</button>
        <button class="secondary" onclick={() => { showAddDecisionModal = false; newDecisionIp = ''; }}>Cancel</button>
      </div>
    </div>
  </div>
{/if}

<!-- Modal Dodawania Bouncera -->
{#if showAddBouncerModal}
  <div class="modal-overlay">
    <div class="modal-content glass fade-in">
      <h3>Register new bouncer</h3>
      <p class="modal-desc">Name the new agent to generate a unique API key for bouncer authorization (e.g. crowdsec-nginx-bouncer).</p>
      
      <div class="form-group">
        <label for="bouncer-name">Bouncer name</label>
        <input id="bouncer-name" type="text" placeholder="e.g. nginx-bouncer-local" bind:value={newBouncerName} />
      </div>

      <div class="modal-actions">
        <button class="primary" onclick={addBouncer} disabled={!newBouncerName || isLoading}>Generate key</button>
        <button class="secondary" onclick={() => { showAddBouncerModal = false; newBouncerName = ''; }}>Cancel</button>
      </div>
    </div>
  </div>
{/if}

<!-- Modal displaying generated bouncer key -->
{#if showBouncerKeyModal}
  <div class="modal-overlay">
    <div class="modal-content glass fade-in key-modal">
      <div class="modal-header-icon">
        <KeyRound size={32} class="accent-amber-text" />
      </div>
      <h3>Bouncer API key generated!</h3>
      <p class="modal-desc">Copy and save the API key below. It will be shown only once. You will need it in the bouncer configuration file.</p>
      
      <div class="key-display-box">
        <code class="key-code">{generatedBouncerKey}</code>
        <button class="copy-btn" onclick={copyBouncerKey} title="Copied">
          <Clipboard size={16} />
        </button>
      </div>

      <div class="modal-actions">
        <button class="primary" onclick={() => { showBouncerKeyModal = false; generatedBouncerKey = ''; }}>Close and done</button>
      </div>
    </div>
  </div>
{/if}

<!-- Sudo Password Prompt Modal -->
{#if showSudoModal}
  <div class="modal-overlay">
    <div class="modal-content glass fade-in">
      <div class="modal-header-icon">
        <KeyRound size={32} class="accent-amber-text" />
      </div>
      <h3>Sudo authentication required</h3>
      <p class="modal-desc">The CrowdSec tab requires a sudo password to communicate with the server. No background operations will run until you provide it.</p>
      <input 
        type="password" 
        placeholder="Enter sudo password" 
        bind:value={sudoPassword} 
        onkeydown={(e) => e.key === 'Enter' && submitSudoPassword()}
      />
      {#if sudoError}
        <span class="error-text">{sudoError}</span>
      {/if}
      <div class="modal-actions">
        <button class="primary" onclick={submitSudoPassword}>Submit</button>
        <button class="secondary" onclick={cancelSudoModal}>Cancel</button>
      </div>
    </div>
  </div>
{/if}

<!-- Alert details panel (side drawer) -->
{#if selectedAlert}
  <div class="drawer-overlay" onclick={() => selectedAlert = null} onkeydown={(e) => e.key === 'Escape' && (selectedAlert = null)} role="button" tabindex="0">
    <div class="drawer-content glass fade-in-right" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()} role="button" tabindex="-1">
      <div class="drawer-header">
        <h3>{`Alert details #${selectedAlert.id}`}</h3>
        <button class="close-drawer-btn" onclick={() => selectedAlert = null}>&times;</button>
      </div>
      
      <div class="drawer-body">
        <div class="detail-row">
          <span class="detail-label">Scenario</span>
          <span class="detail-val font-bold">{selectedAlert.scenario}</span>
        </div>
        <div class="detail-row">
          <span class="detail-label">Attacker IP</span>
          <span class="detail-val font-bold text-red"><code>{selectedAlert.source?.value}</code></span>
        </div>
        <div class="detail-row">
          <span class="detail-label">Description / message</span>
          <span class="detail-val text-secondary">{selectedAlert.message || "Malicious behavior detected"}</span>
        </div>
        <div class="detail-row">
          <span class="detail-label">Event count</span>
          <span class="detail-val mono-stats">{selectedAlert.events_count}</span>
        </div>
        <div class="detail-row">
          <span class="detail-label">Created at</span>
          <span class="detail-val mono-stats">{formatDate(selectedAlert.created_at)}</span>
        </div>

        <div class="divider" style="margin: 20px 0;"></div>
        
        <h4>Decisions linked to alert</h4>
        <div class="alert-decisions">
          {#if selectedAlert.decisions && selectedAlert.decisions.length > 0}
            {#each selectedAlert.decisions as dec}
              <div class="alert-dec-card">
                <div>
                  <span class="badge danger">{dec.type.toUpperCase()}</span>
                  <span class="text-sm text-secondary" style="margin-left: 8px;">Duration: <strong class="mono-stats">{dec.duration}</strong></span>
                </div>
                <div class="text-xs text-muted" style="margin-top: 4px;">Origin: {dec.origin}</div>
              </div>
            {/each}
          {:else}
            <span class="text-muted text-sm">No immediate decisions applied.</span>
          {/if}
        </div>
        
        <div class="divider" style="margin: 20px 0;"></div>

        <h4>Surowy obiekt JSON</h4>
        <div class="raw-json-box">
          <pre><code>{JSON.stringify(selectedAlert, null, 2)}</code></pre>
        </div>
      </div>
    </div>
  </div>
{/if}

<!-- Metric Log Viewer Drawer -->
{#if selectedMetricLogPath}
  <div class="drawer-overlay" onclick={closeMetricLogViewer} onkeydown={(e) => e.key === 'Escape' && closeMetricLogViewer()} role="button" tabindex="0">
    <div class="drawer-content glass fade-in-right" style="width: 650px;" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()} role="button" tabindex="-1">
      <div class="drawer-header">
        <div style="display: flex; flex-direction: column; gap: 4px;">
          <h3 style="font-size: 1.15rem; color: white; margin: 0;">CrowdSec Log Preview</h3>
          <span class="text-xs text-muted mono-stats" style="word-break: break-all;">{selectedMetricLogPath}</span>
        </div>
        <button class="close-drawer-btn" onclick={closeMetricLogViewer}>&times;</button>
      </div>
      
      <div class="drawer-body" style="display: flex; flex-direction: column; height: calc(100% - 70px); padding: 20px; overflow: hidden; gap: 12px;">
        <div class="log-actions-bar" style="display: flex; gap: 10px; align-items: center; flex-wrap: wrap; width: 100%;">
          <div class="search-box" style="flex: 1; min-width: 150px; display: flex; align-items: center; background: rgba(0,0,0,0.2); border: 1px solid var(--border-color); border-radius: var(--radius-sm); padding: 0 10px;">
            <Search size={14} class="text-muted" style="margin-right: 6px;" />
            <input type="text" style="width: 100%; height: 32px; font-size: 0.8rem; background: transparent; border: none; outline: none; padding: 0; margin: 0; color: white;" placeholder="Search logs..." bind:value={metricLogSearchQuery} />
          </div>
          <button class="secondary btn-sm" onclick={fetchMetricLogContent} disabled={isMetricLogLoading} style="display: inline-flex; align-items: center; justify-content: center; height: 34px;">
            <RefreshCw size={14} class={isMetricLogLoading ? 'spin' : ''} />
          </button>
          
          {#if isMetricLogStreaming}
            <button class="secondary btn-sm" onclick={stopMetricLogStreaming} style="display: inline-flex; align-items: center; justify-content: center; gap: 6px; height: 34px;">
              <Pause size={14} /> Pause
            </button>
          {:else}
            <button class="primary btn-sm" onclick={startMetricLogStreaming} style="display: inline-flex; align-items: center; justify-content: center; gap: 6px; height: 34px;">
              <Play size={14} /> Stream
            </button>
          {/if}
        </div>
        
        <div class="log-viewer-box" style="flex: 1; background: #030406; border: 1px solid var(--border-color); border-radius: var(--radius-sm); overflow: auto; padding: 12px; display: flex; flex-direction: column; width: 100%;">
          {#if isMetricLogLoading && !metricLogContent}
            <div style="display: flex; align-items: center; justify-content: center; flex: 1; color: var(--text-secondary);">
              <RefreshCw size={24} class="spin" />
            </div>
          {:else}
            <pre style="margin: 0; font-family: var(--font-mono); font-size: 0.78rem; line-height: 1.4; color: var(--text-secondary); white-space: pre-wrap; word-break: break-all; width: 100%;"><code>{filteredMetricLogs || 'No data to display.'}</code></pre>
          {/if}
        </div>
      </div>
    </div>
  </div>
{/if}

<style>
  .crowdsec-manager {
    display: flex;
    flex-direction: column;
    padding: 24px;
    height: 100%;
    overflow-y: auto;
  }

  .header-title-section {
    display: flex;
    align-items: center;
    gap: 16px;
    flex-wrap: wrap;
  }

  .status-pill {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 6px 12px;
    border-radius: 50px;
    font-size: 0.72rem;
    font-weight: 700;
    border: 1px solid var(--border-color);
  }

  .status-pill.active {
    background: rgba(16, 185, 129, 0.08);
    border-color: rgba(16, 185, 129, 0.3);
    color: var(--accent-green);
  }

  .status-pill.inactive {
    background: rgba(239, 68, 68, 0.08);
    border-color: rgba(239, 68, 68, 0.3);
    color: var(--accent-red);
  }

  .status-pill.unknown {
    background: rgba(245, 158, 11, 0.08);
    border-color: rgba(245, 158, 11, 0.3);
    color: var(--accent-amber);
  }

  .status-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
  }

  .status-pill.active .status-dot {
    background-color: var(--accent-green);
    box-shadow: 0 0 6px var(--accent-green);
  }

  .status-pill.inactive .status-dot {
    background-color: var(--accent-red);
    box-shadow: 0 0 6px var(--accent-red);
  }

  .status-pill.unknown .status-dot {
    background-color: var(--accent-amber);
    box-shadow: 0 0 6px var(--accent-amber);
  }

  /* Auth gate */
  .auth-gate {
    display: flex;
    align-items: center;
    justify-content: center;
    min-height: 420px;
    padding: 24px;
  }

  .auth-gate-card {
    max-width: 440px;
    padding: 36px 32px;
    border-radius: var(--radius-md);
    text-align: center;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 16px;
  }

  .auth-gate-icon {
    background: rgba(245, 158, 11, 0.08);
    border: 1px solid rgba(245, 158, 11, 0.2);
    border-radius: 50%;
    padding: 16px;
    margin-bottom: 4px;
  }

  .auth-gate-card h2 {
    font-size: 1.15rem;
    font-weight: 600;
    color: var(--text-primary);
  }

  .auth-gate-card p {
    font-size: 0.88rem;
    color: var(--text-secondary);
    line-height: 1.55;
    margin: 0;
  }

  .auth-gate-card button {
    margin-top: 8px;
    display: inline-flex;
    align-items: center;
    gap: 8px;
  }

  /* Loading state */
  .loading-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 300px;
    gap: 16px;
    color: var(--text-secondary);
  }

  /* Onboarding */
  .onboarding-screen {
    display: flex;
    justify-content: center;
    align-items: center;
    padding: 40px 0;
  }

  .onboarding-card {
    max-width: 800px;
    width: 100%;
    padding: 40px;
    border-radius: var(--radius-lg);
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
  }

  .onboarding-icon-box {
    background: var(--accent-amber-glow);
    border: 1px solid rgba(245, 158, 11, 0.3);
    padding: 20px;
    border-radius: 50%;
    margin-bottom: 24px;
    box-shadow: 0 0 30px rgba(245, 158, 11, 0.1);
  }

  .onboarding-card h2 {
    font-size: 1.8rem;
    color: white;
    margin-bottom: 12px;
  }

  .onboarding-desc {
    color: var(--text-secondary);
    font-size: 0.95rem;
    line-height: 1.6;
    margin-bottom: 30px;
    max-width: 650px;
  }

  .features-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 20px;
    width: 100%;
    margin-bottom: 30px;
  }

  .feat-card {
    background: rgba(255, 255, 255, 0.01);
    border: 1px solid var(--border-color);
    border-radius: var(--radius-md);
    padding: 20px;
    text-align: left;
    transition: var(--transition-fast);
  }

  .feat-card:hover {
    border-color: rgba(245, 158, 11, 0.15);
    background: rgba(255, 255, 255, 0.02);
  }

  .feat-card h4 {
    color: white;
    margin: 10px 0 6px 0;
    font-size: 0.95rem;
  }

  .feat-card p {
    color: var(--text-secondary);
    font-size: 0.8rem;
    line-height: 1.4;
  }

  .divider {
    height: 1px;
    background: var(--border-color);
    width: 100%;
    margin: 20px 0;
  }

  .onboarding-card h3 {
    font-size: 1.2rem;
    color: white;
    margin-bottom: 20px;
  }

  .install-options {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 24px;
    width: 100%;
    margin-bottom: 30px;
    text-align: left;
  }

  .install-box {
    background: rgba(0, 0, 0, 0.2);
    border: 1px solid var(--border-color);
    border-radius: var(--radius-md);
    padding: 24px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .install-box h4 {
    color: white;
    font-size: 1rem;
  }

  .install-box p {
    color: var(--text-secondary);
    font-size: 0.82rem;
    line-height: 1.5;
    flex-grow: 1;
  }

  .install-box button {
    align-self: flex-start;
  }

  .code-preview {
    background: #030406;
    border: 1px solid var(--border-color);
    border-radius: var(--radius-sm);
    padding: 12px;
    overflow-x: auto;
    font-family: var(--font-mono);
    font-size: 0.72rem;
    max-height: 120px;
  }

  /* Dashboard Grid */
  .dashboard-grid {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 24px;
    margin-top: 20px;
  }

  .dash-card {
    padding: 24px;
    border-radius: var(--radius-md);
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .dash-card h3 {
    font-size: 1.05rem;
    color: white;
    border-bottom: 1px solid var(--border-color);
    padding-bottom: 10px;
    margin-bottom: 4px;
  }

  /* Service status widget */
  .status-indicator-box {
    display: flex;
    align-items: center;
    gap: 16px;
    background: rgba(255, 255, 255, 0.01);
    padding: 16px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-color);
  }

  .status-dot-large {
    width: 12px;
    height: 12px;
    border-radius: 50%;
  }

  .status-dot-large.active {
    background-color: var(--accent-green);
    box-shadow: 0 0 10px var(--accent-green);
  }

  .status-dot-large.inactive {
    background-color: var(--accent-red);
    box-shadow: 0 0 10px var(--accent-red);
  }

  .status-dot-large.unknown {
    background-color: var(--accent-amber);
    box-shadow: 0 0 10px var(--accent-amber);
  }

  .status-details {
    display: flex;
    flex-direction: column;
  }

  .status-text {
    font-size: 1.1rem;
    font-weight: 600;
    color: white;
  }

  .ver-text {
    font-size: 0.78rem;
    color: var(--text-muted);
    margin-top: 2px;
  }

  .service-actions {
    display: flex;
    gap: 10px;
  }

  /* Stats grid widget */
  .stats-grid {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 16px;
    flex-grow: 1;
  }

  .stat-item {
    background: rgba(255, 255, 255, 0.01);
    border: 1px solid var(--border-color);
    border-radius: var(--radius-sm);
    padding: 16px;
    display: flex;
    flex-direction: column;
    justify-content: center;
    align-items: center;
    transition: var(--transition-fast);
    font-family: inherit;
    font-size: inherit;
    color: inherit;
  }

  .stat-item.clickable:hover {
    background: var(--bg-hover);
    border-color: rgba(245, 158, 11, 0.2);
    cursor: pointer;
    transform: translateY(-1px);
  }

  .stat-item.clickable:active {
    transform: translateY(0);
  }

  .stat-num {
    font-size: 1.8rem;
    font-weight: 700;
    color: white;
    font-variant-numeric: tabular-nums;
  }

  .stat-label {
    font-size: 0.8rem;
    color: var(--text-secondary);
    margin-top: 4px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  /* Log Metrics Widget */
  .log-metrics-card {
    grid-column: span 2;
  }

  .log-progress-section {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .progress-details {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .progress-label {
    font-size: 0.9rem;
    color: var(--text-secondary);
  }

  .progress-percentage {
    font-size: 1.2rem;
    font-weight: 700;
    color: var(--accent-amber);
    font-variant-numeric: tabular-nums;
  }

  .progress-bar-container {
    height: 8px;
    background: rgba(255, 255, 255, 0.04);
    border-radius: 10px;
    overflow: hidden;
    width: 100%;
  }

  .progress-bar-container.compact {
    height: 6px;
  }

  .progress-bar-fill {
    height: 100%;
    background: linear-gradient(90deg, var(--accent-amber), var(--accent-orange));
    border-radius: 10px;
    transition: width 0.4s ease-out;
  }

  .progress-legend {
    display: flex;
    gap: 24px;
    margin-top: 4px;
    flex-wrap: wrap;
  }

  .leg-item {
    font-size: 0.8rem;
    color: var(--text-secondary);
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    display: inline-block;
  }

  .dot.success { background-color: var(--accent-green); }
  .dot.danger { background-color: var(--accent-red); }

  /* Sub-tab panels */
  .sub-tab-panel {
    display: flex;
    flex-direction: column;
    gap: 20px;
    margin-top: 20px;
  }

  .panel-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 16px;
    flex-wrap: wrap;
  }

  .search-box {
    flex: 1;
    min-width: 250px;
  }

  .search-box input {
    width: 100%;
    max-width: 400px;
  }

  .panel-actions {
    display: flex;
    gap: 10px;
  }

  /* Whitelist Container */
  .whitelist-container {
    display: grid;
    grid-template-columns: 2fr 3fr;
    gap: 24px;
  }

  .whitelist-form-box, .whitelist-list-box {
    padding: 24px;
    border-radius: var(--radius-md);
  }

  .whitelist-form-box h3, .whitelist-list-box h3 {
    font-size: 1.1rem;
    color: white;
    border-bottom: 1px solid var(--border-color);
    padding-bottom: 10px;
    margin-bottom: 16px;
  }

  .form-desc {
    font-size: 0.8rem;
    color: var(--text-secondary);
    line-height: 1.5;
    margin-bottom: 20px;
  }

  .whitelist-section h4 {
    font-size: 0.85rem;
    color: var(--text-secondary);
    margin-bottom: 10px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .wl-tags-list {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .wl-tag {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    background: rgba(16, 185, 129, 0.08);
    border: 1px solid rgba(16, 185, 129, 0.3);
    color: var(--accent-green);
    padding: 6px 12px;
    border-radius: var(--radius-sm);
    font-size: 0.85rem;
  }

  .wl-tag.warning {
    background: rgba(245, 158, 11, 0.08);
    border-color: rgba(245, 158, 11, 0.3);
    color: var(--accent-amber);
  }

  .wl-tag button {
    background: transparent;
    border: none;
    color: inherit;
    font-size: 1.1rem;
    padding: 0;
    cursor: pointer;
    line-height: 1;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 16px;
    height: 16px;
  }

  .wl-tag button:hover {
    color: var(--accent-red);
  }

  .empty-tag-text {
    font-size: 0.8rem;
    color: var(--text-muted);
  }

  /* Table styling overrides */
  table {
    width: 100%;
    border-collapse: collapse;
    text-align: left;
  }

  th, td {
    padding: 8px 12px;
    border-bottom: 1px solid var(--border-color);
  }

  td {
    font-size: 0.9rem;
  }

  th {
    font-size: 0.72rem;
    text-transform: uppercase;
    color: var(--text-muted);
    font-weight: 600;
    letter-spacing: 0.05em;
    user-select: none;
  }

  tr:last-child td {
    border-bottom: none;
  }

  tr:hover td {
    background-color: var(--bg-hover);
  }

  .empty-state {
    text-align: center;
    color: var(--text-muted);
    padding: 40px !important;
    font-style: italic;
  }

  .btn-table {
    background: transparent;
    border: none;
    padding: 6px;
    border-radius: 4px;
    color: var(--text-muted);
    cursor: pointer;
    transition: var(--transition-fast);
  }

  .btn-table:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .btn-table.danger-text:hover {
    color: var(--accent-red);
    background: var(--accent-red-glow);
  }

  .text-green { color: var(--accent-green); }
  .text-red { color: var(--accent-red); }
  .font-bold { font-weight: 600; }

  /* Modale */
  .modal-overlay {
    position: fixed;
    top: 0;
    left: 0;
    width: 100vw;
    height: 100vh;
    background: rgba(0, 0, 0, 0.7);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
    backdrop-filter: blur(4px);
  }

  .modal-content {
    width: 480px;
    padding: 30px;
    border-radius: var(--radius-md);
    display: flex;
    flex-direction: column;
    gap: 16px;
    background: var(--bg-secondary);
    border: 1px solid var(--border-color);
  }

  .settings-modal {
    width: 540px;
  }

  .modal-header-icon {
    display: flex;
    justify-content: center;
    margin-bottom: 8px;
  }

  .modal-content h3 {
    font-size: 1.2rem;
    color: white;
    text-align: center;
  }

  .modal-desc {
    font-size: 0.85rem;
    color: var(--text-secondary);
    line-height: 1.5;
    text-align: center;
    margin-bottom: 8px;
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

  .form-group select, .form-group input {
    width: 100%;
  }

  .radio-group {
    display: flex;
    flex-direction: column;
    gap: 8px;
    background: rgba(255, 255, 255, 0.01);
    padding: 10px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-color);
  }

  .radio-label {
    display: flex;
    align-items: center;
    gap: 10px;
    cursor: pointer;
    font-size: 0.85rem;
    color: var(--text-primary);
  }

  .radio-label input {
    width: auto;
    cursor: pointer;
  }

  .modal-actions {
    display: flex;
    gap: 10px;
    justify-content: flex-end;
    margin-top: 10px;
  }

  .test-result-box {
    padding: 10px 14px;
    border-radius: var(--radius-sm);
    font-size: 0.8rem;
    display: flex;
    align-items: center;
    gap: 10px;
    line-height: 1.4;
  }

  .test-result-box.success {
    background: var(--accent-green-glow);
    border: 1px solid rgba(16, 185, 129, 0.3);
    color: #a7f3d0;
  }

  .test-result-box.error {
    background: var(--accent-red-glow);
    border: 1px solid rgba(239, 68, 68, 0.3);
    color: #fca5a5;
  }

  .error-text {
    font-size: 0.8rem;
    color: #ff8585;
    margin-top: -8px;
  }

  /* Key Modal */
  .key-modal {
    width: 500px;
    text-align: center;
  }

  .key-display-box {
    display: flex;
    align-items: center;
    justify-content: space-between;
    background: #030406;
    border: 1px solid var(--border-color);
    border-radius: var(--radius-sm);
    padding: 12px 16px;
    margin: 10px 0;
    gap: 12px;
  }

  .key-code {
    font-family: var(--font-mono);
    color: var(--accent-amber);
    word-break: break-all;
    font-size: 0.85rem;
    flex-grow: 1;
    text-align: left;
    user-select: all;
  }

  .copy-btn {
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid var(--border-color);
    color: var(--text-secondary);
    padding: 8px;
    border-radius: var(--radius-sm);
    cursor: pointer;
    transition: var(--transition-fast);
  }

  .copy-btn:hover {
    background: var(--bg-hover);
    color: white;
  }

  /* Drawer / Alert details side panel */
  .drawer-overlay {
    position: fixed;
    top: 0;
    left: 0;
    width: 100vw;
    height: 100vh;
    background: rgba(0, 0, 0, 0.5);
    z-index: 1000;
    display: flex;
    justify-content: flex-end;
    backdrop-filter: blur(2px);
  }

  .drawer-content {
    width: 480px;
    height: 100vh;
    background: var(--bg-secondary);
    border-left: 1px solid var(--border-color);
    box-shadow: -8px 0 32px 0 rgba(0, 0, 0, 0.5);
    display: flex;
    flex-direction: column;
    animation: slideIn 0.25s cubic-bezier(0.16, 1, 0.3, 1) forwards;
  }

  @keyframes slideIn {
    from { transform: translateX(100%); }
    to { transform: translateX(0); }
  }

  .drawer-header {
    padding: 20px 24px;
    border-bottom: 1px solid var(--border-color);
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .drawer-header h3 {
    font-size: 1.15rem;
    color: white;
  }

  .close-drawer-btn {
    background: transparent;
    border: none;
    font-size: 1.8rem;
    color: var(--text-secondary);
    cursor: pointer;
    line-height: 1;
    padding: 0 4px;
  }

  .close-drawer-btn:hover {
    color: white;
  }

  .drawer-body {
    padding: 24px;
    overflow-y: auto;
    flex: 1;
  }

  .detail-row {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin-bottom: 16px;
  }

  .detail-label {
    font-size: 0.78rem;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .detail-val {
    font-size: 0.95rem;
    color: white;
  }

  .alert-decisions {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin-top: 10px;
  }

  .alert-dec-card {
    background: rgba(255, 255, 255, 0.01);
    border: 1px solid var(--border-color);
    border-radius: var(--radius-sm);
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .raw-json-box {
    background: #030406;
    border: 1px solid var(--border-color);
    border-radius: var(--radius-sm);
    padding: 16px;
    overflow: auto;
    max-height: 250px;
    margin-top: 10px;
  }

  .raw-json-box pre {
    margin: 0;
  }

  .raw-json-box code {
    font-family: var(--font-mono);
    font-size: 0.75rem;
    color: var(--text-secondary);
  }

  .btn-sm {
    padding: 8px 14px;
    font-size: 0.8rem;
  }

  .text-sm { font-size: 0.8rem; }
  .text-xs { font-size: 0.72rem; }
  .text-secondary { color: var(--text-secondary); }
  .text-muted { color: var(--text-muted); }
  .accent-amber-text { color: var(--accent-amber); }
  .accent-green-text { color: var(--accent-green); }
  .accent-red-text { color: var(--accent-red); }

  /* Whitelist File Sources and Badges */
  .file-source {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 0.8rem;
    color: var(--text-secondary);
  }

  .tag-system, .tag-managed, .tag-custom, .tag-lapi {
    font-size: 0.65rem;
    padding: 2px 6px;
    border-radius: 4px;
    font-weight: 700;
    text-transform: uppercase;
  }

  .tag-system {
    background: rgba(255, 255, 255, 0.05);
    color: var(--text-secondary);
    border: 1px solid var(--border-color);
  }

  .tag-managed {
    background: rgba(245, 158, 11, 0.1);
    color: var(--accent-amber);
    border: 1px solid rgba(245, 158, 11, 0.25);
  }

  .tag-custom {
    background: rgba(16, 185, 129, 0.1);
    color: var(--accent-green);
    border: 1px solid rgba(16, 185, 129, 0.25);
  }

  .tag-lapi {
    background: rgba(59, 130, 246, 0.1);
    color: #60a5fa;
    border: 1px solid rgba(59, 130, 246, 0.25);
  }

  /* Responsive fixes */
  @media (max-width: 1024px) {
    .dashboard-grid {
      grid-template-columns: 1fr;
    }
    .log-metrics-card {
      grid-column: span 1;
    }
    .whitelist-container {
      grid-template-columns: 1fr;
    }
  }
</style>
