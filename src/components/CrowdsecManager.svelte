<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { 
    ShieldAlert, Shield, ShieldOff, Play, Square, RotateCw, RefreshCw, 
    Plus, Trash2, KeyRound, Check, HelpCircle, Settings, Clipboard,
    ArrowUpRight, AlertCircle, Cpu, FileText, Activity, Users, Box, HardDrive, List, Info
  } from 'lucide-svelte';
  import SortableTh from './ui/SortableTh.svelte';
  import { applySort, nextSort, type SortState } from '$lib/sort/sortUtils';
  import yaml from 'js-yaml';

  // Svelte 5 Props
  let { profileId } = $props<{ profileId: string }>();

  // State zarządzania połączeniem i instalacją
  let isInstalled = $state<boolean | null>(null);
  let connectionMode = $state<'auto' | 'baremetal' | 'docker'>('auto');
  let detectedMode = $state<'baremetal' | 'docker' | null>(null);
  let containerName = $state('crowdsec');
  let customPrefix = $state('');
  let isServiceActive = $state<boolean | null>(null);
  let lapiVersion = $state('');

  // Stany ładowania i formularzy
  let isLoading = $state(false);
  let isInstalling = $state(false);
  let errorMsg = $state('');
  let activeSubTab = $state<'dashboard' | 'decisions' | 'whitelist' | 'alerts' | 'bouncers' | 'metrics' | 'hub'>('dashboard');

  // Modale
  let showSettingsModal = $state(false);
  let showAddDecisionModal = $state(false);
  let showAddBouncerModal = $state(false);
  let showSudoModal = $state(false);
  let showBouncerKeyModal = $state(false);

  // Dane CrowdSec
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

  // Dane formularzy i wygenerowane klucze
  let newDecisionIp = $state('');
  let newDecisionDuration = $state('4h');
  let newDecisionReason = $state('Ręczna blokada');
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

  // Wyszukiwanie i sortowanie
  let searchDecisionQuery = $state('');
  let searchAlertQuery = $state('');
  let searchHubQuery = $state('');
  
  type DecisionSortCol = 'value' | 'type' | 'reason' | 'duration' | 'until';
  let decisionSort = $state<SortState<DecisionSortCol>>({ column: 'value', direction: 'asc' });

  type AlertSortCol = 'id' | 'source' | 'scenario' | 'events_count' | 'created_at';
  let alertSort = $state<SortState<AlertSortCol>>({ column: 'id', direction: 'desc' });

  type HubSortCol = 'type' | 'name' | 'status' | 'version';
  let hubSort = $state<SortState<HubSortCol>>({ column: 'name', direction: 'asc' });

  // Wybrany szczegółowy alert do podglądu
  let selectedAlert = $state<any | null>(null);

  // Konfiguracja zapisu lokalnego
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

  // Budowanie polecenia cscli w zależności od trybu
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
      } catch (err: any) {
        if (err.toString() === 'SUDO_PASSWORD_REQUIRED') {
          pendingAction = run;
          showSudoModal = true;
        } else if (err.toString() === 'SUDO_PASSWORD_INCORRECT') {
          sudoError = 'Niepoprawne hasło sudo. Spróbuj ponownie.';
          showSudoModal = true;
        } else {
          errorMsg = 'Błąd wykonania akcji: ' + err.toString();
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
    } catch (err: any) {
      sudoError = err.toString();
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

  // Wyszukiwanie instalacji i autodetekcja
  async function detectEnvironment() {
    isLoading = true;
    errorMsg = '';
    
    // Wczytaj zapisaną konfigurację
    loadLocalConfig();

    try {
      if (connectionMode === 'baremetal') {
        detectedMode = 'baremetal';
        isInstalled = true;
      } else if (connectionMode === 'docker') {
        detectedMode = 'docker';
        isInstalled = true;
      } else {
        // Tryb automatyczny
        // 1. Sprawdź whether cscli jest zainstalowany natywnie
        try {
          await invoke('exec_custom_command', { cmd: 'which cscli', useSudo: false });
          detectedMode = 'baremetal';
          isInstalled = true;
        } catch (e) {
          // 2. Jeśli nie natywnie, sprawdź kontenery docker
          try {
            const dockerOut = await invoke<string>('exec_custom_command', {
              cmd: 'docker ps --filter "name=crowdsec" --format "{{.Names}}"',
              useSudo: true
            });
            const containers = dockerOut.trim().split('\n').filter(Boolean);
            if (containers.length > 0) {
              detectedMode = 'docker';
              // Domyślnie bierzemy pierwszy dopasowany kontener, jeśli użytkownik nie wpisał innego
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
      errorMsg = 'Błąd podczas detekcji środowiska: ' + err.toString();
      isInstalled = false;
    } finally {
      isLoading = false;
    }
  }

  // Ładowanie wszystkich danych
  async function loadAllData() {
    if (!isInstalled) return;
    isLoading = true;
    errorMsg = '';

    try {
      // 1. Status usługi
      await fetchServiceStatus();

      // 2. W zależności od aktywnej zakładki ładujemy dane specyficzne, 
      //    ale wczytujemy też kluczowe statystyki do Dashboardu.
      await Promise.all([
        fetchDecisions(),
        fetchBouncers(),
        fetchAlerts(),
        fetchMetrics(),
        fetchHub(),
        fetchWhitelist()
      ]);
    } catch (err: any) {
      const errStr = err.toString();
      if (errStr === 'SUDO_PASSWORD_REQUIRED' || errStr === 'SUDO_PASSWORD_INCORRECT') {
        pendingAction = loadAllData;
        showSudoModal = true;
        if (errStr === 'SUDO_PASSWORD_INCORRECT') {
          sudoError = 'Niepoprawne hasło sudo. Spróbuj ponownie.';
        }
      } else {
        errorMsg = 'Błąd wczytywania danych CrowdSec: ' + errStr;
      }
    } finally {
      isLoading = false;
    }
  }

  // Pobieranie statusu usługi systemowej lub kontenera
  async function fetchServiceStatus() {
    const isDocker = connectionMode === 'docker' || (connectionMode === 'auto' && detectedMode === 'docker');
    try {
      if (isDocker) {
        const out = await invoke<string>('exec_custom_command', {
          cmd: `docker inspect -f '{{.State.Running}}' ${containerName}`,
          useSudo: true
        });
        isServiceActive = out.trim() === 'true';
        
        // Pobranie wersji LAPI w dockerze
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
      if (err.toString() === 'SUDO_PASSWORD_REQUIRED' || err.toString() === 'SUDO_PASSWORD_INCORRECT') {
        throw err;
      }
      console.error(err);
      isServiceActive = false;
      lapiVersion = 'Nieznana';
    }
  }

  // Kontrola usługi (Start/Stop/Restart)
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
        const errStr = err.toString();
        if (errStr === 'SUDO_PASSWORD_REQUIRED' || errStr === 'SUDO_PASSWORD_INCORRECT') {
          throw err;
        }
        errorMsg = `Błąd wykonania ${action} usługi: ` + errStr;
      } finally {
        isLoading = false;
      }
    };

    await handleActionWithSudo(run);
  }

  // Pobranie decyzji (Banów)
  async function fetchDecisions() {
    try {
      const out = await runCscliCommand('decisions list -o json', true);
      const parsed = JSON.parse(out);
      decisions = Array.isArray(parsed) ? parsed : [];
    } catch (err: any) {
      if (err.toString() === 'SUDO_PASSWORD_REQUIRED' || err.toString() === 'SUDO_PASSWORD_INCORRECT') {
        throw err;
      }
      console.error('Błąd pobierania decyzji:', err);
      decisions = [];
    }
  }

  // Dodanie manualnego bana
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
        const errStr = err.toString();
        if (errStr === 'SUDO_PASSWORD_REQUIRED' || errStr === 'SUDO_PASSWORD_INCORRECT') {
          throw err;
        }
        errorMsg = 'Nie udało się dodać bana: ' + errStr;
      } finally {
        isLoading = false;
      }
    };
    await handleActionWithSudo(run);
  }

  // Ręczne usuwanie bana (Unban)
  async function deleteDecision(ip: string) {
    if (!confirm(`Czy na pewno chcesz usunąć blokadę (unban) dla IP ${ip}?`)) return;
    const run = async () => {
      isLoading = true;
      errorMsg = '';
      try {
        await runCscliCommand(`decisions delete --ip ${ip}`, true);
        await fetchDecisions();
      } catch (err: any) {
        const errStr = err.toString();
        if (errStr === 'SUDO_PASSWORD_REQUIRED' || errStr === 'SUDO_PASSWORD_INCORRECT') {
          throw err;
        }
        errorMsg = `Nie udało się zdjąć bana z ${ip}: ` + errStr;
      } finally {
        isLoading = false;
      }
    };
    await handleActionWithSudo(run);
  }

  // Usunięcie wszystkich banów
  async function deleteAllDecisions() {
    if (!confirm('Czy na pewno chcesz usunąć WSZYSTKIE aktywne blokady w CrowdSec?')) return;
    const run = async () => {
      isLoading = true;
      errorMsg = '';
      try {
        await runCscliCommand('decisions delete --all', true);
        await fetchDecisions();
      } catch (err: any) {
        const errStr = err.toString();
        if (errStr === 'SUDO_PASSWORD_REQUIRED' || errStr === 'SUDO_PASSWORD_INCORRECT') {
          throw err;
        }
        errorMsg = 'Błąd podczas usuwania wszystkich banów: ' + errStr;
      } finally {
        isLoading = false;
      }
    };
    await handleActionWithSudo(run);
  }

  // Pobranie białej listy ze wszystkich plików w s02-enrich oraz z LAPI
  async function fetchWhitelist() {
    const isDocker = connectionMode === 'docker' || (connectionMode === 'auto' && detectedMode === 'docker');
    const dirPath = '/etc/crowdsec/parsers/s02-enrich';
    
    let items: WhitelistItem[] = [];
    let managedIp: string[] = [];
    let managedCidr: string[] = [];
    let foundLapiLists: string[] = [];

    // 1. Pobieranie z plików parsera YAML
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

            // Jeśli to jest plik zarządzany przez Jarvis, pobierz do formularza
            if (file === 'jarvis-whitelist.yaml') {
              managedIp = ips;
              managedCidr = cidrs;
            }
          }
        } catch (fileErr: any) {
          if (fileErr.toString() === 'SUDO_PASSWORD_REQUIRED' || fileErr.toString() === 'SUDO_PASSWORD_INCORRECT') {
            throw fileErr;
          }
          console.error(`Błąd odczytu pliku białej listy ${file}:`, fileErr);
        }
      }
    } catch (err: any) {
      if (err.toString() === 'SUDO_PASSWORD_REQUIRED' || err.toString() === 'SUDO_PASSWORD_INCORRECT') {
        throw err;
      }
      console.error('Błąd listowania plików białych list:', err);
    }

    // 2. Pobieranie allowlist z LAPI
    try {
      let allowlistsOut = '';
      let usingSubcommand = 'allowlists';
      try {
        allowlistsOut = await runCscliCommand('allowlists list -o json', true);
      } catch (e) {
        // Spróbuj aliasu w liczbie pojedynczej
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
                if (inspectErr.toString() === 'SUDO_PASSWORD_REQUIRED' || inspectErr.toString() === 'SUDO_PASSWORD_INCORRECT') {
                  throw inspectErr;
                }
                console.error(`Błąd inspekcji allowlisty ${name}:`, inspectErr);
              }
            }
          }
        }
      }
    } catch (lapiErr: any) {
      if (lapiErr.toString() === 'SUDO_PASSWORD_REQUIRED' || lapiErr.toString() === 'SUDO_PASSWORD_INCORRECT') {
        throw lapiErr;
      }
      console.warn('LAPI allowlisty nie są obsługiwane lub nie udało się ich pobrać:', lapiErr);
    }

    whitelistItems = items;
    whitelistData = { ip: managedIp, cidr: managedCidr };
    lapiAllowlists = foundLapiLists;
  }

  // Zapisanie białej listy do pliku YAML i reload/restart
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
        
        // Ponieważ plik leży w zabezpieczonej lokalizacji, musimy zapisać przez echo i sudo tee
        // Bezpieczne escapowanie cudzysłowów dla echo
        const escapedYaml = yamlStr.replace(/'/g, "'\\''");
        
        if (isDocker) {
          // Docker wymaga utworzenia katalogu, jeśli nie istnieje w kontenerze (s02-enrich zazwyczaj istnieje)
          await invoke('exec_custom_command', {
            cmd: `docker exec -i ${containerName} mkdir -p /etc/crowdsec/parsers/s02-enrich`,
            useSudo: true
          });
          
          await invoke('exec_custom_command', {
            cmd: `echo '${escapedYaml}' | docker exec -i ${containerName} tee ${path}`,
            useSudo: true
          });

          // Reload CrowdSec w Dockerze - wysyłamy SIGHUP do procesu 1
          try {
            await invoke('exec_custom_command', {
              cmd: `docker exec -i ${containerName} kill -HUP 1`,
              useSudo: true
            });
          } catch (reloadErr) {
            // Jeśli kill -HUP się nie uda, restartujemy kontener
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
        const errStr = err.toString();
        if (errStr === 'SUDO_PASSWORD_REQUIRED' || errStr === 'SUDO_PASSWORD_INCORRECT') {
          throw err;
        }
        errorMsg = 'Błąd podczas zapisywania białej listy: ' + errStr;
      } finally {
        isLoading = false;
      }
    };

    await handleActionWithSudo(run);
  }

  // Dodanie elementu do białej listy (YAML lub LAPI)
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
              // Ignoruj błędy usuwania bana (jeśli go nie było)
            }
          }
          
          newWhitelistIp = '';
          await fetchWhitelist();
        } catch (err: any) {
          const errStr = err.toString();
          if (errStr === 'SUDO_PASSWORD_REQUIRED' || errStr === 'SUDO_PASSWORD_INCORRECT') {
            throw err;
          }
          errorMsg = 'Błąd podczas dodawania elementu do LAPI allowlisty: ' + errStr;
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
      // 1. Zapisz whitelist
      await saveWhitelist(updated);
      
      // 2. Automatycznie usuń aktywny ban z tego IP (wygaszenie natychmiastowe)
      if (newWhitelistType === 'ip') {
        try {
          await runCscliCommand(`decisions delete --ip ${targetIp}`, true);
          await fetchDecisions();
        } catch (unbanErr) {
          // Ignorujemy błędy usuwania bana (jeśli go nie było w bazie)
        }
      }
      
      newWhitelistIp = '';
    };

    await handleActionWithSudo(run);
  }

  // Usunięcie elementu z białej listy (YAML lub LAPI)
  async function removeWhitelistItem(item: WhitelistItem) {
    if (item.isSystem) return;
    
    if (item.file.startsWith('LAPI: ')) {
      const allowlistName = item.file.substring(6);
      if (!confirm(`Czy na pewno chcesz usunąć ${item.value} z LAPI allowlisty "${allowlistName}"?`)) return;

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
          const errStr = err.toString();
          if (errStr === 'SUDO_PASSWORD_REQUIRED' || errStr === 'SUDO_PASSWORD_INCORRECT') {
            throw err;
          }
          errorMsg = 'Błąd podczas usuwania elementu z LAPI allowlisty: ' + errStr;
        } finally {
          isLoading = false;
        }
      };
      await handleActionWithSudo(run);
      return;
    }

    if (!confirm(`Czy na pewno chcesz usunąć ${item.value} z białej listy (plik: ${item.file})?`)) return;
    
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
        const errStr = err.toString();
        if (errStr === 'SUDO_PASSWORD_REQUIRED' || errStr === 'SUDO_PASSWORD_INCORRECT') {
          throw err;
        }
        errorMsg = 'Błąd podczas usuwania elementu z białej listy: ' + errStr;
      } finally {
        isLoading = false;
      }
    };

    await handleActionWithSudo(run);
  }

  // Pobranie zarejestrowanych Bouncerów
  async function fetchBouncers() {
    try {
      const out = await runCscliCommand('bouncers list -o json', true);
      const parsed = JSON.parse(out);
      bouncers = Array.isArray(parsed) ? parsed : [];
    } catch (err: any) {
      if (err.toString() === 'SUDO_PASSWORD_REQUIRED' || err.toString() === 'SUDO_PASSWORD_INCORRECT') {
        throw err;
      }
      console.error('Błąd pobierania bouncerów:', err);
      bouncers = [];
    }
  }

  // Dodanie nowego bouncera i uzyskanie API Key
  async function addBouncer() {
    if (!newBouncerName) return;
    const run = async () => {
      isLoading = true;
      errorMsg = '';
      generatedBouncerKey = '';
      try {
        const out = await runCscliCommand(`bouncers add ${newBouncerName} -o json`, true);
        const parsed = JSON.parse(out);
        
        // cscli zwraca w formacie JSON obiekt lub tablicę z "api_key"
        if (parsed && parsed.api_key) {
          generatedBouncerKey = parsed.api_key;
        } else if (Array.isArray(parsed) && parsed[0] && parsed[0].api_key) {
          generatedBouncerKey = parsed[0].api_key;
        } else {
          // Jeśli z jakiegoś powodu JSON nie ma klucza, a jest to tekstowy output
          generatedBouncerKey = out;
        }
        
        showAddBouncerModal = false;
        showBouncerKeyModal = true;
        newBouncerName = '';
        await fetchBouncers();
      } catch (err: any) {
        const errStr = err.toString();
        if (errStr === 'SUDO_PASSWORD_REQUIRED' || errStr === 'SUDO_PASSWORD_INCORRECT') {
          throw err;
        }
        errorMsg = 'Nie udało się dodać bouncera: ' + errStr;
      } finally {
        isLoading = false;
      }
    };
    await handleActionWithSudo(run);
  }

  // Kopiowanie klucza bouncera
  function copyBouncerKey() {
    navigator.clipboard.writeText(generatedBouncerKey);
    alert('Klucz API Bouncera skopiowany do schowka!');
  }

  // Usunięcie bouncera
  async function deleteBouncer(name: string) {
    if (!confirm(`Czy na pewno chcesz usunąć bouncera "${name}"?`)) return;
    const run = async () => {
      isLoading = true;
      errorMsg = '';
      try {
        await runCscliCommand(`bouncers delete ${name}`, true);
        await fetchBouncers();
      } catch (err: any) {
        const errStr = err.toString();
        if (errStr === 'SUDO_PASSWORD_REQUIRED' || errStr === 'SUDO_PASSWORD_INCORRECT') {
          throw err;
        }
        errorMsg = `Nie udało się usunąć bouncera ${name}: ` + errStr;
      } finally {
        isLoading = false;
      }
    };
    await handleActionWithSudo(run);
  }

  // Pruning bouncerów
  async function pruneBouncers() {
    if (!confirm('Czy chcesz wyczyścić bouncery, które nie komunikowały się od ponad 45 minut?')) return;
    const run = async () => {
      isLoading = true;
      errorMsg = '';
      try {
        await runCscliCommand('bouncers prune -d 45m', true);
        await fetchBouncers();
      } catch (err: any) {
        const errStr = err.toString();
        if (errStr === 'SUDO_PASSWORD_REQUIRED' || errStr === 'SUDO_PASSWORD_INCORRECT') {
          throw err;
        }
        errorMsg = 'Nie udało się wyczyścić nieaktywnych bouncerów: ' + errStr;
      } finally {
        isLoading = false;
      }
    };
    await handleActionWithSudo(run);
  }

  // Pobranie alertów
  async function fetchAlerts() {
    try {
      const out = await runCscliCommand('alerts list -o json', true);
      const parsed = JSON.parse(out);
      alerts = Array.isArray(parsed) ? parsed : [];
    } catch (err: any) {
      if (err.toString() === 'SUDO_PASSWORD_REQUIRED' || err.toString() === 'SUDO_PASSWORD_INCORRECT') {
        throw err;
      }
      console.error('Błąd pobierania alertów:', err);
      alerts = [];
    }
  }

  // Pobranie szczegółowych metryk
  async function fetchMetrics() {
    try {
      const out = await runCscliCommand('metrics show -o json', true);
      metrics = JSON.parse(out);
    } catch (err: any) {
      if (err.toString() === 'SUDO_PASSWORD_REQUIRED' || err.toString() === 'SUDO_PASSWORD_INCORRECT') {
        throw err;
      }
      console.error('Błąd pobierania metryk:', err);
      metrics = null;
    }
  }

  // Pobranie listy hub
  async function fetchHub() {
    try {
      const out = await runCscliCommand('hub list -o json', true);
      const parsed = JSON.parse(out);
      
      // Parsowanie hub list z JSON:
      // Zwraca klucze takie jak 'parsers', 'scenarios', 'postoverflows', 'collections' jako tablice
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
      if (err.toString() === 'SUDO_PASSWORD_REQUIRED' || err.toString() === 'SUDO_PASSWORD_INCORRECT') {
        throw err;
      }
      console.error('Błąd pobierania hub list:', err);
      hubItems = [];
    }
  }

  // Uruchomienie instalacji CrowdSec (Debian/Ubuntu)
  async function installCrowdSec() {
    const run = async () => {
      isInstalling = true;
      errorMsg = '';
      try {
        // Skrypt instalacyjny CrowdSec i bouncera firewall
        const cmd = 'curl -s https://install.crowdsec.net | sudo sh && sudo apt-get update && sudo apt-get install -y crowdsec crowdsec-firewall-bouncer-iptables';
        await invoke('exec_custom_command', { cmd, useSudo: true });
        
        // Próba ponownej detekcji po instalacji
        await detectEnvironment();
      } catch (err: any) {
        const errStr = err.toString();
        if (errStr === 'SUDO_PASSWORD_REQUIRED' || errStr === 'SUDO_PASSWORD_INCORRECT') {
          throw err;
        }
        errorMsg = 'Błąd instalacji CrowdSec: ' + errStr;
      } finally {
        isInstalling = false;
      }
    };
    
    await handleActionWithSudo(run);
  }

  // Test połączenia w ustawieniach
  let isTestingConnection = $state(false);
  let testConnectionResult = $state<{ success: boolean; msg: string } | null>(null);

  async function testConnection() {
    isTestingConnection = true;
    testConnectionResult = null;
    try {
      const verOut = await runCscliCommand('version', true);
      if (verOut.toLowerCase().includes('version') || verOut.includes('db')) {
        testConnectionResult = { success: true, msg: 'Połączenie udane. Wersja: ' + verOut.split('\n')[0] };
      } else {
        testConnectionResult = { success: false, msg: 'Nieoczekiwana odpowiedź: ' + verOut.substring(0, 100) };
      }
    } catch (err: any) {
      testConnectionResult = { success: false, msg: 'Błąd połączenia: ' + err.toString() };
    } finally {
      isTestingConnection = false;
    }
  }

  // Zapisanie ustawień
  function handleSaveSettings() {
    saveLocalConfig();
    showSettingsModal = false;
    testConnectionResult = null;
    detectEnvironment(); // Zaktualizuj stan środowiska
  }

  // --- FILTROWANIE I SORTOWANIE WIDOKÓW ---

  // Sortowanie i filtrowanie decyzji
  const filteredDecisions = $derived(
    decisions.filter(d => {
      if (!searchDecisionQuery) return true;
      const q = searchDecisionQuery.toLowerCase();
      return (
        (d.value && d.value.toLowerCase().includes(q)) ||
        (d.type && d.type.toLowerCase().includes(q)) ||
        (d.reason && d.reason.toLowerCase().includes(q))
      );
    })
  );

  const sortedDecisions = $derived(
    applySort(filteredDecisions, decisionSort, {
      value: (d) => d.value || '',
      type: (d) => d.type || '',
      reason: (d) => d.reason || '',
      duration: (d) => d.duration || '',
      until: (d) => d.until || '',
    })
  );

  function setDecisionSort(column: string) {
    decisionSort = nextSort(decisionSort, column as DecisionSortCol);
  }

  // Sortowanie i filtrowanie alertów
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

  // Sortowanie i filtrowanie CrowdSec Hub
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

  // --- STATYSTYKI OBLICZANIOWE (METRYKI) ---

  // Bezpieczne wyciąganie listy logów z metryk (Acquisition)
  const acquisitionList = $derived.by(() => {
    if (!metrics || !metrics.acquisition) return [];
    
    // Wersje CrowdSec mogą zwracać acquisition jako tablicę lub mapę
    if (Array.isArray(metrics.acquisition)) {
      return metrics.acquisition.map((item: any) => ({
        source: item.source || 'Nieznany log',
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

  // Zagregowane statystyki logów do widgetu Dashboardu
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

  // Wykrywanie aktualnego opisu połączenia w UI
  const connectionModeLabel = $derived.by(() => {
    if (connectionMode === 'auto') {
      return `Automatyczny (${detectedMode === 'docker' ? 'Docker' : 'Bare-metal'})`;
    }
    return connectionMode === 'docker' ? 'Kontener Docker' : 'Bare-metal (Systemd)';
  });

  const configuredContainerName = $derived(
    (connectionMode === 'docker' || (connectionMode === 'auto' && detectedMode === 'docker')) ? containerName : ''
  );

  onMount(() => {
    initCrowdsec();
  });
</script>

<div class="crowdsec-manager manager-shell fade-in">
  <header class="manager-header">
    <div class="header-title-section">
      <h1 class="page-title">Bezpieczeństwo (CrowdSec)</h1>
      {#if isSudoAuthorized && isInstalled}
        <span class="status-pill {isServiceActive === null ? 'unknown' : (isServiceActive ? 'active' : 'inactive')}">
          <span class="status-dot"></span>
          {#if isServiceActive === null}
            SPRAWDZANIE...
          {:else}
            {isServiceActive ? 'AKTYWNY' : 'NIEAKTYWNY'}
          {/if}
          ({connectionModeLabel})
        </span>
      {/if}
      {#if errorMsg}
        <div class="error-badge">{errorMsg}</div>
      {/if}
    </div>

    <div class="header-actions">
      {#if isInstalled}
        <button class="secondary btn-sm" onclick={loadAllData} disabled={isLoading}>
          <RefreshCw size={14} class={isLoading ? 'spin' : ''} /> Odśwież
        </button>
      {/if}
      <button class="secondary btn-sm" onclick={() => { showSettingsModal = true; testConnectionResult = null; }}>
        <Settings size={14} /> Ustawienia
      </button>
    </div>
  </header>

  {#if !isSudoAuthorized}
    <div class="auth-gate fade-in">
      <div class="auth-gate-card glass">
        <div class="auth-gate-icon">
          <KeyRound size={40} class="accent-amber-text" />
        </div>
        <h2>Wymagane uwierzytelnienie Sudo</h2>
        <p>
          Zakładka CrowdSec wymaga hasła sudo do komunikacji z serwerem.
          Dopóki go nie podasz, nie będą wykonywane żadne operacje w tle.
        </p>
        <button class="primary" onclick={requestSudoAuth}>
          <KeyRound size={16} /> Podaj hasło sudo
        </button>
      </div>
    </div>
  {:else if isInstalled === null}
    <!-- Stan ładowania / detekcji -->
    <div class="loading-state">
      <RefreshCw size={36} class="spin muted-icon" />
      <p>Trwa wykrywanie środowiska CrowdSec...</p>
    </div>
  {:else if isInstalled === false}
    <!-- Ekran Onboardingowy / Instalator -->
    <div class="onboarding-screen fade-in">
      <div class="onboarding-card glass">
        <div class="onboarding-icon-box">
          <ShieldAlert size={42} class="accent-amber-text" />
        </div>
        <h2>Zabezpiecz swój serwer z CrowdSec</h2>
        <p class="onboarding-desc">
          CrowdSec to nowoczesny, oparty na współpracy system ochrony IPS. Analizuje logi Twojego serwera w poszukiwaniu prób włamań, skanowania portów i innych szkodliwych aktywności, a następnie blokuje napastników i dzieli się ich adresami z globalną bazą zagrożeń.
        </p>

        <div class="features-grid">
          <div class="feat-card">
            <Activity size={20} class="accent-amber-text" />
            <h4>Analiza Logów</h4>
            <p>Monitoruje logi systemowe i logi usług (SSH, Web, Mail) w czasie rzeczywistym.</p>
          </div>
          <div class="feat-card">
            <Users size={20} class="accent-green-text" />
            <h4>Sieć Reputacji IP</h4>
            <p>Pobiera i aktualizuje bazę złośliwych adresów IP zweryfikowanych przez społeczność CrowdSec.</p>
          </div>
          <div class="feat-card">
            <Shield size={20} class="accent-red-text" />
            <h4>Bouncery (Blokady)</h4>
            <p>Automatycznie integruje się z zaporą ogniową serwera (np. UFW/iptables), odrzucając ataki.</p>
          </div>
        </div>

        <div class="divider"></div>

        <h3>Jak chcesz skonfigurować CrowdSec?</h3>

        <div class="install-options">
          <div class="install-box">
            <h4>Metoda 1: Instalacja bezpośrednia (Bare-metal)</h4>
            <p>Zainstaluje CrowdSec i powiązanego Bouncera zapory (iptables) bezpośrednio na serwerze (wymaga dystrybucji opartej o APT, np. Debian/Ubuntu).</p>
            <button class="primary" onclick={installCrowdSec} disabled={isInstalling}>
              {#if isInstalling}
                <RefreshCw size={16} class="spin" /> Instalowanie...
              {:else}
                <Plus size={16} /> Zainstaluj bezpośrednio
              {/if}
            </button>
          </div>

          <div class="install-box">
            <h4>Metoda 2: Uruchomienie w Dockerze</h4>
            <p>Jeśli wolisz Docker, uruchom CrowdSec w kontenerze. Upewnij się, że kontener ma nazwę <code>crowdsec</code> lub otwórz ustawienia, aby podać inną nazwę.</p>
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
            Skonfiguruj połączenie ręcznie
          </button>
        </div>
      </div>
    </div>
  {:else}
    <!-- Panel Główny z podzakładkami -->
    <div class="tabs-bar glass">
      <button class="tab-btn {activeSubTab === 'dashboard' ? 'active' : ''}" onclick={() => activeSubTab = 'dashboard'}>
        <Activity size={16} /> Podgląd
      </button>
      <button class="tab-btn {activeSubTab === 'decisions' ? 'active' : ''}" onclick={() => activeSubTab = 'decisions'}>
        <Shield size={16} /> Decyzje (Bany)
      </button>
      <button class="tab-btn {activeSubTab === 'whitelist' ? 'active' : ''}" onclick={() => activeSubTab = 'whitelist'}>
        <Check size={16} /> Biała lista
      </button>
      <button class="tab-btn {activeSubTab === 'alerts' ? 'active' : ''}" onclick={() => activeSubTab = 'alerts'}>
        <ShieldAlert size={16} /> Alerty
      </button>
      <button class="tab-btn {activeSubTab === 'bouncers' ? 'active' : ''}" onclick={() => activeSubTab = 'bouncers'}>
        <Box size={16} /> Bouncery
      </button>
      <button class="tab-btn {activeSubTab === 'metrics' ? 'active' : ''}" onclick={() => activeSubTab = 'metrics'}>
        <FileText size={16} /> Metryki logów
      </button>
      <button class="tab-btn {activeSubTab === 'hub' ? 'active' : ''}" onclick={() => activeSubTab = 'hub'}>
        <Cpu size={16} /> CrowdSec Hub
      </button>
    </div>

    <div class="tab-content">
      {#if activeSubTab === 'dashboard'}
        <!-- DASHBOARD WIDGETS -->
        <div class="dashboard-grid fade-in">
          <!-- Widget Statusu Usługi -->
          <div class="dash-card glass service-status-card">
            <h3>Status Usługi</h3>
            <div class="status-indicator-box">
              <div class="status-dot-large {isServiceActive === null ? 'unknown' : (isServiceActive ? 'active' : 'inactive')}"></div>
              <div class="status-details">
                <span class="status-text">
                  {#if isServiceActive === null}
                    Sprawdzanie statusu...
                  {:else}
                    {isServiceActive ? 'Działa (Online)' : 'Zatrzymana (Offline)'}
                  {/if}
                </span>
                <span class="ver-text">LAPI Wersja: {lapiVersion || 'Sprawdzanie...'}</span>
              </div>
            </div>
            <div class="service-actions">
              {#if isServiceActive}
                <button class="danger btn-sm" onclick={() => controlService('stop')} disabled={isLoading}>
                  <Square size={14} /> Zatrzymaj
                </button>
              {:else}
                <button class="primary btn-sm" onclick={() => controlService('start')} disabled={isLoading || isServiceActive === null}>
                  <Play size={14} /> Uruchom
                </button>
              {/if}
              <button class="secondary btn-sm" onclick={() => controlService('restart')} disabled={isLoading || isServiceActive === null}>
                <RotateCw size={14} /> Restart
              </button>
            </div>
          </div>

          <!-- Widget Statystyk telemetrycznych -->
          <div class="dash-card glass stats-overview-card">
            <h3>Podsumowanie Bazy</h3>
            <div class="stats-grid">
              <div class="stat-item clickable" onclick={() => activeSubTab = 'decisions'}>
                <span class="stat-num mono-stats">{decisions.length}</span>
                <span class="stat-label">Aktywne Bany</span>
              </div>
              <div class="stat-item clickable" onclick={() => activeSubTab = 'bouncers'}>
                <span class="stat-num mono-stats">{bouncers.length}</span>
                <span class="stat-label">Bouncery</span>
              </div>
              <div class="stat-item clickable" onclick={() => activeSubTab = 'alerts'}>
                <span class="stat-num mono-stats">{alerts.length}</span>
                <span class="stat-label">Historia Alertów</span>
              </div>
              <div class="stat-item clickable" onclick={() => activeSubTab = 'whitelist'}>
                <span class="stat-num mono-stats">{whitelistData.ip.length + whitelistData.cidr.length}</span>
                <span class="stat-label">Biała lista</span>
              </div>
            </div>
          </div>

          <!-- Widget statystyk przetwarzania logów -->
          <div class="dash-card glass log-metrics-card">
            <h3>Przetwarzanie logów</h3>
            <div class="log-progress-section">
              <div class="progress-details">
                <span class="progress-label">Skuteczność Parsowania</span>
                <span class="progress-percentage mono-stats">{logStats.successRate}%</span>
              </div>
              <div class="progress-bar-container">
                <div class="progress-bar-fill" style="width: {logStats.successRate}%"></div>
              </div>
              <div class="progress-legend">
                <span class="leg-item"><span class="dot success"></span> Przetworzone: <strong class="mono-stats">{logStats.parsed}</strong></span>
                <span class="leg-item"><span class="dot danger"></span> Pominięte: <strong class="mono-stats">{logStats.unparsed}</strong></span>
                <span class="leg-item">Suma: <strong class="mono-stats">{logStats.read}</strong></span>
              </div>
            </div>
          </div>
        </div>

      {:else if activeSubTab === 'decisions'}
        <!-- DECISION MANAGER (BANY) -->
        <div class="sub-tab-panel fade-in">
          <div class="panel-header">
            <div class="search-box">
              <input type="text" placeholder="Szukaj po adresie IP lub powodzie..." bind:value={searchDecisionQuery} />
            </div>
            <div class="panel-actions">
              <button class="primary" onclick={() => showAddDecisionModal = true}>
                <Plus size={16} /> Dodaj Ban
              </button>
              <button class="danger" onclick={deleteAllDecisions} disabled={decisions.length === 0}>
                <Trash2 size={16} /> Usuń wszystkie bany
              </button>
            </div>
          </div>

          <div class="table-container glass">
            <table>
              <thead>
                <tr>
                  <SortableTh label="Wartość (IP/Zakres)" column="value" activeColumn={decisionSort.column} direction={decisionSort.direction} onsort={setDecisionSort} width="25%" />
                  <SortableTh label="Typ" column="type" activeColumn={decisionSort.column} direction={decisionSort.direction} onsort={setDecisionSort} width="15%" />
                  <SortableTh label="Powód / Scenariusz" column="reason" activeColumn={decisionSort.column} direction={decisionSort.direction} onsort={setDecisionSort} width="30%" />
                  <SortableTh label="Czas Trwania" column="duration" activeColumn={decisionSort.column} direction={decisionSort.direction} onsort={setDecisionSort} width="15%" />
                  <SortableTh label="Wygasa" column="until" activeColumn={decisionSort.column} direction={decisionSort.direction} onsort={setDecisionSort} width="10%" />
                  <th style="width: 5%; text-align: right;">Odbanuj</th>
                </tr>
              </thead>
              <tbody>
                {#each sortedDecisions as dec}
                  <tr>
                    <td class="mono-stats font-bold"><code>{dec.value}</code></td>
                    <td><span class="badge {dec.type === 'ban' ? 'danger' : 'warning'}">{dec.type.toUpperCase()}</span></td>
                    <td class="text-secondary">{dec.reason}</td>
                    <td class="mono-stats">{dec.duration}</td>
                    <td class="mono-stats text-muted" title={dec.until}>{dec.until ? new Date(dec.until).toLocaleString('pl-PL') : 'Brak danych'}</td>
                    <td style="text-align: right;">
                      <button class="btn-table danger-text" onclick={() => deleteDecision(dec.value)} title="Zdejmij ban z tego IP">
                        <Trash2 size={14} />
                      </button>
                    </td>
                  </tr>
                {/each}

                {#if sortedDecisions.length === 0}
                  <tr>
                    <td colspan="6" class="empty-state">Brak aktywnych blokad (banów). Twój serwer jest bezpieczny.</td>
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
              <h3>Dodaj do Białej Listy</h3>
              <p class="form-desc">Dodanie adresu IP lub podsieci CIDR sprawi, że CrowdSec zignoruje wszelkie ich działania. Zapis nastąpi do wybranego pliku YAML lub bezpośrednio do Local API (LAPI) wybranej allowlisty. Jeśli dane IP jest obecnie zbanowane, ban zostanie automatycznie zdjęty.</p>
              
              <div class="form-group">
                <label for="wl-target">Lokalizacja zapisu</label>
                <select id="wl-target" bind:value={whitelistTarget}>
                  <option value="yaml">Plik lokalny (jarvis-whitelist.yaml)</option>
                  {#each lapiAllowlists as name}
                    <option value="lapi:{name}">LAPI Allowlist: {name}</option>
                  {/each}
                </select>
              </div>

              <div class="form-group">
                <label for="wl-type">Typ wpisu</label>
                <select id="wl-type" bind:value={newWhitelistType}>
                  <option value="ip">Pojedynczy adres IP (np. 192.168.1.50)</option>
                  <option value="cidr">Podsieć CIDR (np. 192.168.1.0/24)</option>
                </select>
              </div>

              <div class="form-group">
                <label for="wl-ip">Adres IP lub Podsieć</label>
                <input id="wl-ip" type="text" placeholder={newWhitelistType === 'ip' ? '8.8.8.8' : '10.0.0.0/24'} bind:value={newWhitelistIp} />
              </div>

              <button class="primary" onclick={addWhitelistItem} disabled={!newWhitelistIp || isLoading}>
                <Plus size={16} /> Dodaj do białej listy
              </button>
            </div>

            <div class="whitelist-list-box glass" style="display: flex; flex-direction: column;">
              <h3>Aktywne Wykluczenia (Biała lista)</h3>
              
              <div class="table-container" style="border: none; box-shadow: none; padding: 0; overflow-y: auto; max-height: 400px; margin-top: 10px;">
                <table>
                  <thead>
                    <tr>
                      <th>Adres IP / Podsieć</th>
                      <th>Typ</th>
                      <th>Plik / Źródło</th>
                      <th style="width: 5%; text-align: right;">Usuń</th>
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
                              <span class="tag-system">Systemowy</span>
                            {:else if item.file === 'jarvis-whitelist.yaml'}
                              <span class="tag-managed">Jarvis</span>
                            {:else if item.file.startsWith('LAPI: ')}
                              <span class="tag-lapi">LAPI</span>
                            {:else}
                              <span class="tag-custom">Własny</span>
                            {/if}
                          </span>
                        </td>
                        <td style="text-align: right;">
                          {#if item.isSystem}
                            <button class="btn-table" disabled title="Plik systemowy - tylko do odczytu" style="opacity: 0.3; cursor: not-allowed;">
                              <Trash2 size={14} />
                            </button>
                          {:else}
                            <button class="btn-table danger-text" onclick={() => removeWhitelistItem(item)} title={item.file.startsWith('LAPI: ') ? `Usuń z LAPI allowlisty ${item.file.substring(6)}` : `Usuń z pliku ${item.file}`}>
                              <Trash2 size={14} />
                            </button>
                          {/if}
                        </td>
                      </tr>
                    {/each}

                    {#if whitelistItems.length === 0}
                      <tr>
                        <td colspan="4" class="empty-state">Brak aktywnych wykluczeń (białej listy).</td>
                      </tr>
                    {/if}
                  </tbody>
                </table>
              </div>
            </div>
          </div>
        </div>

      {:else if activeSubTab === 'alerts'}
        <!-- ALERTS LOG (HISTORIA INCYDENTÓW) -->
        <div class="sub-tab-panel fade-in">
          <div class="panel-header">
            <div class="search-box">
              <input type="text" placeholder="Szukaj alertów po IP lub scenariuszu..." bind:value={searchAlertQuery} />
            </div>
          </div>

          <div class="table-container glass">
            <table>
              <thead>
                <tr>
                  <SortableTh label="ID" column="id" activeColumn={alertSort.column} direction={alertSort.direction} onsort={setAlertSort} width="10%" />
                  <SortableTh label="Źródło (IP)" column="source" activeColumn={alertSort.column} direction={alertSort.direction} onsort={setAlertSort} width="20%" />
                  <SortableTh label="Scenariusz" column="scenario" activeColumn={alertSort.column} direction={alertSort.direction} onsort={setAlertSort} width="35%" />
                  <SortableTh label="Zdarzenia" column="events_count" activeColumn={alertSort.column} direction={alertSort.direction} onsort={setAlertSort} width="15%" />
                  <SortableTh label="Czas wykrycia" column="created_at" activeColumn={alertSort.column} direction={alertSort.direction} onsort={setAlertSort} width="15%" />
                  <th style="width: 5%; text-align: right;">Szczegóły</th>
                </tr>
              </thead>
              <tbody>
                {#each sortedAlerts as alert}
                  <tr>
                    <td class="mono-stats">{alert.id}</td>
                    <td class="mono-stats font-bold"><code>{alert.source?.value}</code></td>
                    <td class="text-secondary">{alert.scenario}</td>
                    <td class="mono-stats">{alert.events_count}</td>
                    <td class="mono-stats text-muted">{new Date(alert.created_at).toLocaleString('pl-PL')}</td>
                    <td style="text-align: right;">
                      <button class="btn-table" onclick={() => selectedAlert = alert} title="Pokaż szczegóły alertu">
                        <Info size={14} />
                      </button>
                    </td>
                  </tr>
                {/each}

                {#if sortedAlerts.length === 0}
                  <tr>
                    <td colspan="6" class="empty-state">Brak historii alertów. Logi serwera są czyste.</td>
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
              <span class="text-secondary text-sm">Lista bouncerów (agentów odbierających decyzje i nakładających blokady w firewallu).</span>
            </div>
            <div class="panel-actions">
              <button class="primary" onclick={() => showAddBouncerModal = true}>
                <Plus size={16} /> Dodaj Bouncera
              </button>
              <button class="secondary" onclick={pruneBouncers} disabled={bouncers.length === 0}>
                Wyczyść nieaktywne
              </button>
            </div>
          </div>

          <div class="table-container glass">
            <table>
              <thead>
                <tr>
                  <th>Nazwa</th>
                  <th>IP Adres</th>
                  <th>Typ (Silnik)</th>
                  <th>Wersja</th>
                  <th>Ostatnia Aktywność</th>
                  <th style="text-align: right;">Akcje</th>
                </tr>
              </thead>
              <tbody>
                {#each bouncers as bouncer}
                  <tr>
                    <td class="font-bold">{bouncer.name}</td>
                    <td class="mono-stats"><code>{bouncer.ip_address || 'Lokalny / Unix socket'}</code></td>
                    <td><span class="badge warning">{bouncer.type || 'Brak'}</span></td>
                    <td class="mono-stats">{bouncer.version || 'Brak'}</td>
                    <td class="mono-stats text-muted">
                      {bouncer.last_pull ? new Date(bouncer.last_pull).toLocaleString('pl-PL') : 'Nigdy'}
                    </td>
                    <td style="text-align: right;">
                      <button class="btn-table danger-text" onclick={() => deleteBouncer(bouncer.name)} title="Usuń tego bouncera">
                        <Trash2 size={14} />
                      </button>
                    </td>
                  </tr>
                {/each}

                {#if bouncers.length === 0}
                  <tr>
                    <td colspan="6" class="empty-state">Brak zarejestrowanych bouncerów. Bez bouncera, blokady CrowdSec nie są fizycznie nakładane na zaporę sieciową.</td>
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
              <h3>Monitorowane Pliki Logów (Acquisition)</h3>
              <table style="margin-top: 10px;">
                <thead>
                  <tr>
                    <th>Lokalizacja Pliku</th>
                    <th style="text-align: right;">Linie przeczytane</th>
                    <th style="text-align: right;">Linie sparsowane</th>
                    <th style="text-align: right;">Niesparsowane</th>
                    <th style="text-align: right; width: 200px;">Wskaźnik sukcesu</th>
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
                    </tr>
                  {/each}

                  {#if acquisitionList.length === 0}
                    <tr>
                      <td colspan="5" class="empty-state">Brak metryk. Brak aktywnych strumieni logów.</td>
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
              <input type="text" placeholder="Szukaj parserów lub scenariuszy..." bind:value={searchHubQuery} />
            </div>
          </div>

          <div class="table-container glass">
            <table>
              <thead>
                <tr>
                  <SortableTh label="Typ komponentu" column="type" activeColumn={hubSort.column} direction={hubSort.direction} onsort={setHubSort} width="20%" />
                  <SortableTh label="Nazwa Hub" column="name" activeColumn={hubSort.column} direction={hubSort.direction} onsort={setHubSort} width="40%" />
                  <SortableTh label="Status" column="status" activeColumn={hubSort.column} direction={hubSort.direction} onsort={setHubSort} width="20%" />
                  <SortableTh label="Wersja lokalna" column="version" activeColumn={hubSort.column} direction={hubSort.direction} onsort={setHubSort} width="20%" />
                </tr>
              </thead>
              <tbody>
                {#each sortedHubItems as item}
                  <tr>
                    <td><span class="badge {item.item_type === 'scenario' ? 'warning' : 'success'}">{item.item_type.toUpperCase()}</span></td>
                    <td class="font-bold">{item.name}</td>
                    <td>
                      <span class="badge {item.status === 'enabled' ? 'success-glow' : 'muted'}">
                        {item.status === 'enabled' ? 'WŁĄCZONY' : 'WYŁĄCZONY'}
                      </span>
                    </td>
                    <td class="mono-stats">{item.local_version || 'Brak'}</td>
                  </tr>
                {/each}

                {#if sortedHubItems.length === 0}
                  <tr>
                    <td colspan="4" class="empty-state">Brak pobranych elementów z CrowdSec Hub.</td>
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

<!-- ================= MODALE ================= -->

<!-- Modal Ustawień Połączenia -->
{#if showSettingsModal}
  <div class="modal-overlay">
    <div class="modal-content glass settings-modal fade-in">
      <h3>Ustawienia połączenia CrowdSec</h3>
      <p class="modal-desc">Skonfiguruj metodę wywoływania poleceń CrowdSec na tym serwerze. Ustawienia zostaną zapisane w tym profilu połączenia.</p>
      
      <div class="form-group">
        <label>Tryb połączenia</label>
        <div class="radio-group">
          <label class="radio-label">
            <input type="radio" name="conn-mode" value="auto" bind:group={connectionMode} />
            <span>Auto-detekcja</span>
          </label>
          <label class="radio-label">
            <input type="radio" name="conn-mode" value="baremetal" bind:group={connectionMode} />
            <span>Natywny (Systemd / Bare-metal)</span>
          </label>
          <label class="radio-label">
            <input type="radio" name="conn-mode" value="docker" bind:group={connectionMode} />
            <span>Docker (Wewnątrz kontenera)</span>
          </label>
        </div>
      </div>

      {#if connectionMode === 'docker'}
        <div class="form-group">
          <label for="docker-name">Nazwa kontenera Docker</label>
          <input id="docker-name" type="text" placeholder="crowdsec" bind:value={containerName} />
        </div>
      {/if}

      <div class="form-group">
        <label for="custom-prefix">Niestandardowy prefiks polecenia (Opcjonalny - np. do K8s, podmana)</label>
        <input id="custom-prefix" type="text" placeholder="np. podman exec -i crowdsec" bind:value={customPrefix} />
      </div>

      <!-- Wynik testu połączenia -->
      {#if testConnectionResult}
        <div class="test-result-box {testConnectionResult.success ? 'success' : 'error'}">
          <AlertCircle size={16} />
          <span>{testConnectionResult.msg}</span>
        </div>
      {/if}

      <div class="modal-actions">
        <button class="secondary" onclick={testConnection} disabled={isTestingConnection}>
          {#if isTestingConnection}
            <RefreshCw size={14} class="spin" /> Testowanie...
          {:else}
            Testuj Połączenie
          {/if}
        </button>
        <button class="primary" onclick={handleSaveSettings}>Zapisz Ustawienia</button>
        <button class="secondary" onclick={() => { showSettingsModal = false; testConnectionResult = null; }}>Anuluj</button>
      </div>
    </div>
  </div>
{/if}

<!-- Modal Dodawania Decyzji (Banowania IP) -->
{#if showAddDecisionModal}
  <div class="modal-overlay">
    <div class="modal-content glass fade-in">
      <h3>Nałóż manualną blokadę (Ban)</h3>
      <p class="modal-desc">Wpisz dane, aby ręcznie zablokować adres IP lub podsieć w systemie CrowdSec.</p>
      
      <div class="form-group">
        <label for="dec-scope">Zakres blokady</label>
        <select id="dec-scope" bind:value={newDecisionScope}>
          <option value="ip">Pojedyncze IP (np. 185.220.101.5)</option>
          <option value="range">Podsieć CIDR (np. 185.220.101.0/24)</option>
        </select>
      </div>

      <div class="form-group">
        <label for="dec-ip">Adres IP / Zakres</label>
        <input id="dec-ip" type="text" placeholder="1.2.3.4" bind:value={newDecisionIp} />
      </div>

      <div class="form-group">
        <label for="dec-dur">Czas trwania blokady</label>
        <select id="dec-dur" bind:value={newDecisionDuration}>
          <option value="4h">4 godziny (Domyślnie)</option>
          <option value="24h">24 godziny</option>
          <option value="48h">48 godzin</option>
          <option value="7d">7 dni</option>
          <option value="1h">1 godzina</option>
        </select>
      </div>

      <div class="form-group">
        <label for="dec-reason">Powód blokady</label>
        <input id="dec-reason" type="text" placeholder="Ręczne zablokowanie przez administratora" bind:value={newDecisionReason} />
      </div>

      <div class="modal-actions">
        <button class="primary" onclick={addDecision} disabled={!newDecisionIp || isLoading}>Dodaj Blokadę</button>
        <button class="secondary" onclick={() => { showAddDecisionModal = false; newDecisionIp = ''; }}>Anuluj</button>
      </div>
    </div>
  </div>
{/if}

<!-- Modal Dodawania Bouncera -->
{#if showAddBouncerModal}
  <div class="modal-overlay">
    <div class="modal-content glass fade-in">
      <h3>Zarejestruj nowego Bouncera</h3>
      <p class="modal-desc">Nadaj nazwę dla nowego agenta, aby wygenerować unikalny klucz API niezbędny do autoryzacji bouncera (np. crowdsec-nginx-bouncer).</p>
      
      <div class="form-group">
        <label for="bouncer-name">Nazwa Bouncera</label>
        <input id="bouncer-name" type="text" placeholder="np. nginx-bouncer-local" bind:value={newBouncerName} />
      </div>

      <div class="modal-actions">
        <button class="primary" onclick={addBouncer} disabled={!newBouncerName || isLoading}>Generuj Klucz</button>
        <button class="secondary" onclick={() => { showAddBouncerModal = false; newBouncerName = ''; }}>Anuluj</button>
      </div>
    </div>
  </div>
{/if}

<!-- Modal Wyświetlający Wygenerowany Klucz Bouncera -->
{#if showBouncerKeyModal}
  <div class="modal-overlay">
    <div class="modal-content glass fade-in key-modal">
      <div class="modal-header-icon">
        <KeyRound size={32} class="accent-amber-text" />
      </div>
      <h3>Klucz API Bouncera został wygenerowany!</h3>
      <p class="modal-desc">Skopiuj i zapisz poniższy klucz API. Zostanie wyświetlony <strong>tylko raz</strong>. Będzie potrzebny w pliku konfiguracyjnym bouncera.</p>
      
      <div class="key-display-box">
        <code class="key-code">{generatedBouncerKey}</code>
        <button class="copy-btn" onclick={copyBouncerKey} title="Skopiuj klucz">
          <Clipboard size={16} />
        </button>
      </div>

      <div class="modal-actions">
        <button class="primary" onclick={() => { showBouncerKeyModal = false; generatedBouncerKey = ''; }}>Zamknij i Gotowe</button>
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
      <h3>Wymagane uwierzytelnienie Sudo</h3>
      <p class="modal-desc">Operacje CrowdSec wymagają uprawnień roota na serwerze. Wprowadź hasło użytkownika (sudo):</p>
      <input 
        type="password" 
        placeholder="Wpisz hasło..." 
        bind:value={sudoPassword} 
        onkeydown={(e) => e.key === 'Enter' && submitSudoPassword()}
      />
      {#if sudoError}
        <span class="error-text">{sudoError}</span>
      {/if}
      <div class="modal-actions">
        <button class="primary" onclick={submitSudoPassword}>Zatwierdź</button>
        <button class="secondary" onclick={cancelSudoModal}>Anuluj</button>
      </div>
    </div>
  </div>
{/if}

<!-- Panel Szczegółów Alertu (Boczny drawer) -->
{#if selectedAlert}
  <div class="drawer-overlay" onclick={() => selectedAlert = null} onkeydown={(e) => e.key === 'Escape' && (selectedAlert = null)} role="button" tabindex="0">
    <div class="drawer-content glass fade-in-right" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()} role="button" tabindex="-1">
      <div class="drawer-header">
        <h3>Szczegóły Alertu #{selectedAlert.id}</h3>
        <button class="close-drawer-btn" onclick={() => selectedAlert = null}>&times;</button>
      </div>
      
      <div class="drawer-body">
        <div class="detail-row">
          <span class="detail-label">Scenariusz</span>
          <span class="detail-val font-bold">{selectedAlert.scenario}</span>
        </div>
        <div class="detail-row">
          <span class="detail-label">Atakujący IP</span>
          <span class="detail-val font-bold text-red"><code>{selectedAlert.source?.value}</code></span>
        </div>
        <div class="detail-row">
          <span class="detail-label">Opis/Wiadomość</span>
          <span class="detail-val text-secondary">{selectedAlert.message || 'Wykryto zachowanie złośliwe'}</span>
        </div>
        <div class="detail-row">
          <span class="detail-label">Liczba Zdarzeń</span>
          <span class="detail-val mono-stats">{selectedAlert.events_count}</span>
        </div>
        <div class="detail-row">
          <span class="detail-label">Czas utworzenia</span>
          <span class="detail-val mono-stats">{new Date(selectedAlert.created_at).toLocaleString('pl-PL')}</span>
        </div>

        <div class="divider" style="margin: 20px 0;"></div>
        
        <h4>Decyzje powiązane z alertem</h4>
        <div class="alert-decisions">
          {#if selectedAlert.decisions && selectedAlert.decisions.length > 0}
            {#each selectedAlert.decisions as dec}
              <div class="alert-dec-card">
                <div>
                  <span class="badge danger">{dec.type.toUpperCase()}</span>
                  <span class="text-sm text-secondary" style="margin-left: 8px;">Czas: <strong class="mono-stats">{dec.duration}</strong></span>
                </div>
                <div class="text-xs text-muted" style="margin-top: 4px;">Pochodzenie: {dec.origin}</div>
              </div>
            {/each}
          {:else}
            <span class="text-muted text-sm">Brak nałożonych natychmiastowych decyzji.</span>
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

  .error-badge {
    background: var(--accent-red-glow);
    border: 1px solid rgba(239, 68, 68, 0.3);
    color: #ff8585;
    padding: 6px 12px;
    border-radius: var(--radius-sm);
    font-size: 0.8rem;
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
    padding: 14px 16px;
    border-bottom: 1px solid var(--border-color);
    font-size: 0.85rem;
  }

  th {
    font-size: 0.8rem;
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

  /* Drawer / Panel boczny dla szczegółów alertów */
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

  .spin {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }

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
