<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import {
    RefreshCw, Globe2, Plus, RotateCw, Trash2, ShieldCheck, ShieldAlert, X, FlaskConical,
    FileCode, Power, PowerOff, CheckCircle, Save, Download, Server, Lock, Network, Settings2,
  } from 'lucide-svelte';
  import SudoModal from './SudoModal.svelte';
  import { LL } from '$lib/i18n/i18n-svelte';
  import { get } from 'svelte/store';
  import { wrapCmd, shQuote, isDocker, type ExecTarget, listContainers } from '$lib/exec/target';
  import { stickToBottom } from '$lib/stickToBottom';
  import { notifications } from '$lib/notifications.svelte';
  import { formatInvokeError, isSudoPasswordRequired } from '$lib/i18n/backendErrors';

  // ---------------------------------------------------------------------------
  // Types
  // ---------------------------------------------------------------------------
  type SubTab = 'hosts' | 'certs' | 'files' | 'control';

  interface NginxProfile {
    id: string;
    name: string;
    target: ExecTarget;
  }

  interface ProxyMeta {
    version: number;
    domain: string;
    scheme: 'http' | 'https';
    forwardHost: string;
    forwardPort: number;
    websockets: boolean;
    blockExploits: boolean;
    cacheAssets: boolean;
    sslEnabled: boolean;
    sslCertName: string;
    forceSsl: boolean;
    http2: boolean;
    hstsEnabled: boolean;
    hstsSubdomains: boolean;
    hstsPreload: boolean;
    advancedConfig: string;
  }

  interface ProxyHost {
    meta: ProxyMeta;
    path: string;
    group: 'site' | 'confd';
    enabled: boolean;
  }

  interface NginxFile {
    name: string;
    path: string;
    group: 'available' | 'enabled' | 'confd';
  }

  interface Cert {
    name: string;
    domains: string;
    expiry: string;
    daysLeft: number | null;
    valid: boolean;
    path: string;
  }

  type DnsProvider = 'cloudflare' | 'digitalocean' | 'route53' | 'manual';

  // ---------------------------------------------------------------------------
  // Shared state
  // ---------------------------------------------------------------------------
  const NGINX = {
    avail: '/etc/nginx/sites-available',
    enabled: '/etc/nginx/sites-enabled',
    confd: '/etc/nginx/conf.d',
  };
  const JARVIS_MARKER = '# --- JARVIS PROXY HOST METADATA ---';
  const JARVIS_END = '# --- END JARVIS PROXY HOST METADATA ---';

  let { profileId = '' } = $props();

  const profilesStoreKey = $derived(`jarvis-nginx-profiles-${profileId}`);
  const activeProfileStoreKey = $derived(`jarvis-nginx-active-profile-${profileId}`);

  let profiles = $state<NginxProfile[]>([]);
  let activeProfileId = $state('');

  let target = $state<ExecTarget>({ kind: 'host' });
  let subTab = $state<SubTab>('hosts');
  let isLoading = $state(false);
  let errorMsg = $state('');

  let showSudoModal = $state(false);
  let pendingAction: (() => Promise<void>) | null = null;

  // streamed output modal
  let showOutput = $state(false);
  let outputText = $state('');
  let outputTitle = $state('');

  // profile editor state
  let showProfilesModal = $state(false);
  let isEditingProfile = $state(false);
  let profileFormId = $state('');
  let profileFormName = $state('');
  let profileFormKind = $state<'host' | 'docker'>('host');
  let profileFormContainer = $state('');

  let containers = $state<string[]>([]);
  let loadingContainers = $state(false);

  async function loadContainers() {
    loadingContainers = true;
    containers = await listContainers(true);
    if (containers.length > 0 && !profileFormContainer) {
      profileFormContainer = containers[0];
    }
    loadingContainers = false;
  }

  function loadProfiles() {
    const stored = localStorage.getItem(profilesStoreKey);
    const storedActive = localStorage.getItem(activeProfileStoreKey);
    if (stored) {
      try {
        profiles = JSON.parse(stored);
      } catch {
        profiles = [];
      }
    } else {
      profiles = [];
    }

    if (storedActive && profiles.some((p) => p.id === storedActive)) {
      activeProfileId = storedActive;
    } else if (profiles.length > 0) {
      activeProfileId = profiles[0].id;
    } else {
      activeProfileId = '';
    }

    applyActiveProfile();

    if (profiles.length === 0) {
      openAddProfile();
    }
  }

  function saveProfiles() {
    localStorage.setItem(profilesStoreKey, JSON.stringify(profiles));
  }

  function applyActiveProfile() {
    const active = profiles.find((p) => p.id === activeProfileId);
    if (active) {
      target = active.target;
      localStorage.setItem(activeProfileStoreKey, activeProfileId);
      loadCurrent();
    } else {
      target = { kind: 'host' };
    }
  }

  function handleProfileChange() {
    applyActiveProfile();
  }

  function openAddProfile() {
    isEditingProfile = false;
    profileFormId = Math.random().toString(36).substring(7);
    profileFormName = '';
    profileFormKind = 'host';
    profileFormContainer = containers[0] || '';
  }

  function openEditProfile(p: NginxProfile) {
    isEditingProfile = true;
    profileFormId = p.id;
    profileFormName = p.name;
    profileFormKind = p.target.kind;
    profileFormContainer = p.target.kind === 'docker' ? p.target.container : (containers[0] || '');
  }

  function saveProfileForm() {
    if (!profileFormName.trim()) {
      notifications.error('Please enter a profile name');
      return;
    }
    const t: ExecTarget = profileFormKind === 'host' ? { kind: 'host' } : { kind: 'docker', container: profileFormContainer };
    if (isEditingProfile) {
      profiles = profiles.map((p) => (p.id === profileFormId ? { id: profileFormId, name: profileFormName.trim(), target: t } : p));
    } else {
      profiles = [...profiles, { id: profileFormId, name: profileFormName.trim(), target: t }];
      if (profiles.length === 1) {
        activeProfileId = profileFormId;
      }
    }
    saveProfiles();
    applyActiveProfile();
    profileFormId = '';
    profileFormName = '';
  }

  function deleteProfile(id: string) {
    const p = profiles.find((x) => x.id === id);
    if (!p) return;
    if (!confirm(get(LL).webserver.deleteProfileConfirm({ name: p.name }))) return;
    profiles = profiles.filter((x) => x.id !== id);
    saveProfiles();
    if (activeProfileId === id) {
      if (profiles.length > 0) {
        activeProfileId = profiles[0].id;
      } else {
        activeProfileId = '';
      }
    }
    applyActiveProfile();
  }

  function installAllRequirements() {
    const cmd = wrapCmd(
      target,
      `if command -v apt-get >/dev/null 2>&1; then ` +
        `apt-get update && apt-get install -y certbot python3-certbot-dns-cloudflare; ` +
      `elif command -v apk >/dev/null 2>&1; then ` +
        `apk add --no-cache certbot certbot-cloudflare || ` +
        `(pip3 install certbot certbot-dns-cloudflare); ` +
      `elif command -v dnf >/dev/null 2>&1; then ` +
        `dnf install -y certbot python3-certbot-dns-cloudflare; ` +
      `elif command -v yum >/dev/null 2>&1; then ` +
        `yum install -y certbot python3-certbot-dns-cloudflare; ` +
      `else ` +
        `echo 'No supported package manager found'; ` +
      `fi`
    );
    runStreamed(cmd, get(LL).webserver.installingRequirements(), loadCerts);
  }

  $effect(() => {
    if (profileId) {
      untrack(() => {
        loadProfiles();
      });
    }
  });

  $effect(() => {
    if (errorMsg) {
      notifications.error(errorMsg);
      errorMsg = '';
    }
  });

  function utf8b64(s: string): string {
    return btoa(unescape(encodeURIComponent(s)));
  }
  function b64utf8(s: string): string {
    return decodeURIComponent(escape(atob(s)));
  }

  async function withSudo(run: () => Promise<void>) {
    try {
      await run();
    } catch (err) {
      if (isSudoPasswordRequired(err)) {
        pendingAction = () => withSudo(run);
        showSudoModal = true;
      } else {
        errorMsg = formatInvokeError(err);
      }
    }
  }

  async function runStreamed(cmd: string, title: string, after?: () => Promise<void>) {
    showOutput = true;
    outputTitle = title;
    outputText = `$ ${cmd}\n`;
    const eventId = Math.random().toString(36).substring(7);
    const unStdout = await listen<string>(`exec-stdout-${eventId}`, (e) => (outputText += e.payload));
    const unStderr = await listen<string>(`exec-stderr-${eventId}`, (e) => (outputText += e.payload));
    const run = async () => {
      try {
        await invoke('exec_custom_command_stream', { cmd, useSudo: true, eventId });
        if (after) await after();
      } catch (err) {
        if (isSudoPasswordRequired(err)) {
          pendingAction = run;
          showSudoModal = true;
        } else {
          outputText += `\n${formatInvokeError(err)}`;
        }
      }
    };
    try {
      await run();
    } finally {
      unStdout();
      unStderr();
    }
  }

  /** Write text content to a (possibly root-owned) path on the target. */
  function writeFileCmd(path: string, content: string): string {
    return `printf %s ${shQuote(utf8b64(content))} | base64 -d > ${shQuote(path)}`;
  }

  // ---------------------------------------------------------------------------
  // Proxy Hosts
  // ---------------------------------------------------------------------------
  let hosts = $state<ProxyHost[]>([]);

  function parseHosts(out: string, enabledSet: Set<string>): ProxyHost[] {
    const result: ProxyHost[] = [];
    const blocks = out.split('===FILE: ').slice(1);
    for (const block of blocks) {
      const lines = block.split('\n');
      const path = (lines.shift() || '').trim();
      if (!path) continue;
      const markerIdx = lines.findIndex((l) => l.trim() === JARVIS_MARKER);
      if (markerIdx === -1) continue;
      const metaLine = (lines[markerIdx + 1] || '').replace(/^#\s?/, '').trim();
      if (!metaLine) continue;
      let meta: ProxyMeta;
      try {
        meta = JSON.parse(b64utf8(metaLine));
      } catch {
        continue;
      }
      const group: ProxyHost['group'] = path.startsWith(NGINX.confd) ? 'confd' : 'site';
      const base = path.split('/').pop() || '';
      const enabled = group === 'site' ? enabledSet.has(base) : !base.endsWith('.disabled');
      result.push({ meta, path, group, enabled });
    }
    return result;
  }

  async function loadHosts() {
    isLoading = true;
    errorMsg = '';
    const cmd = wrapCmd(
      target,
      `echo '@@@ENABLED'; ls -1 ${shQuote(NGINX.enabled)} 2>/dev/null || true; echo '@@@FILES'; ` +
        `for d in ${shQuote(NGINX.avail)} ${shQuote(NGINX.confd)}; do ` +
        `if [ -d "$d" ]; then for f in "$d"/*; do ` +
        `if [ -f "$f" ]; then echo "===FILE: $f"; head -n 30 "$f" 2>/dev/null || true; fi; ` +
        `done; fi; done`,
    );
    await withSudo(async () => {
      const out = await invoke<string>('exec_custom_command', { cmd, useSudo: true });
      const [enabledPart, filesPart = ''] = out.split('@@@FILES');
      const enabledSet = new Set(
        enabledPart.replace('@@@ENABLED', '').split('\n').map((l) => l.trim()).filter(Boolean),
      );
      hosts = parseHosts(filesPart, enabledSet).sort((a, b) => a.meta.domain.localeCompare(b.meta.domain));
    });
    isLoading = false;
  }

  function blankMeta(): ProxyMeta {
    return {
      version: 1,
      domain: '',
      scheme: 'http',
      forwardHost: '127.0.0.1',
      forwardPort: 8080,
      websockets: false,
      blockExploits: true,
      cacheAssets: false,
      sslEnabled: false,
      sslCertName: '',
      forceSsl: false,
      http2: true,
      hstsEnabled: false,
      hstsSubdomains: false,
      hstsPreload: false,
      advancedConfig: '',
    };
  }

  // ---- host editor modal ----
  let showHostModal = $state(false);
  let hostModalTab = $state<'details' | 'ssl' | 'advanced'>('details');
  let editing = $state<ProxyHost | null>(null);
  let form = $state<ProxyMeta>(blankMeta());

  function openAddHost() {
    editing = null;
    form = blankMeta();
    hostModalTab = 'details';
    showHostModal = true;
    loadCerts();
  }
  function openEditHost(h: ProxyHost) {
    editing = h;
    form = { ...h.meta };
    hostModalTab = 'details';
    showHostModal = true;
    loadCerts();
  }

  function genConfig(m: ProxyMeta): string {
    const meta = utf8b64(JSON.stringify(m));
    const out: string[] = [];
    out.push(JARVIS_MARKER);
    out.push('# ' + meta);
    out.push(JARVIS_END);
    out.push('# Managed by Jarvis Nginx Proxy Manager. Do not remove the metadata block above.');
    out.push('');

    const proxyPass = `${m.scheme}://${m.forwardHost}:${m.forwardPort}`;
    const body = (): string[] => {
      const b: string[] = [];
      if (m.blockExploits) {
        b.push('    if ($request_uri ~* "(<|>|%0A|%0D|%27|%3C|%3E|%00)") { return 403; }');
        b.push('');
      }
      b.push('    location / {');
      b.push(`        proxy_pass ${proxyPass};`);
      b.push('        proxy_set_header Host $host;');
      b.push('        proxy_set_header X-Real-IP $remote_addr;');
      b.push('        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;');
      b.push('        proxy_set_header X-Forwarded-Proto $scheme;');
      if (m.websockets) {
        b.push('        proxy_http_version 1.1;');
        b.push('        proxy_set_header Upgrade $http_upgrade;');
        b.push('        proxy_set_header Connection "upgrade";');
      }
      b.push('    }');
      if (m.cacheAssets) {
        b.push('');
        b.push('    location ~* \\.(jpg|jpeg|png|gif|ico|css|js|woff2?|svg)$ {');
        b.push(`        proxy_pass ${proxyPass};`);
        b.push('        proxy_set_header Host $host;');
        b.push('        expires 30d;');
        b.push('        add_header Cache-Control "public, no-transform";');
        b.push('    }');
      }
      if (m.advancedConfig.trim()) {
        b.push('');
        for (const line of m.advancedConfig.replace(/\r/g, '').split('\n')) {
          b.push('    ' + line);
        }
      }
      return b;
    };

    // HTTP server (port 80)
    out.push('server {');
    out.push('    listen 80;');
    out.push('    listen [::]:80;');
    out.push(`    server_name ${m.domain};`);
    if (m.sslEnabled && m.sslCertName && m.forceSsl) {
      out.push('');
      out.push('    location / {');
      out.push('        return 301 https://$host$request_uri;');
      out.push('    }');
    } else {
      out.push('');
      out.push(...body());
    }
    out.push('}');

    // HTTPS server (port 443)
    if (m.sslEnabled && m.sslCertName) {
      out.push('');
      out.push('server {');
      out.push('    listen 443 ssl;');
      out.push('    listen [::]:443 ssl;');
      if (m.http2) out.push('    http2 on;');
      out.push(`    server_name ${m.domain};`);
      out.push('');
      out.push(`    ssl_certificate /etc/letsencrypt/live/${m.sslCertName}/fullchain.pem;`);
      out.push(`    ssl_certificate_key /etc/letsencrypt/live/${m.sslCertName}/privkey.pem;`);
      if (m.hstsEnabled) {
        let hsts = 'max-age=63072000';
        if (m.hstsSubdomains) hsts += '; includeSubDomains';
        if (m.hstsPreload) hsts += '; preload';
        out.push(`    add_header Strict-Transport-Security "${hsts}" always;`);
      }
      out.push('');
      out.push(...body());
      out.push('}');
    }
    return out.join('\n') + '\n';
  }

  function reloadCmd(): string {
    return isDocker(target) ? 'nginx -t && nginx -s reload' : 'nginx -t && systemctl reload nginx';
  }

  async function saveHost() {
    const m = form;
    if (!m.domain.trim() || !m.forwardHost.trim()) {
      errorMsg = get(LL).webserver.needDomainForward();
      return;
    }
    const content = genConfig(m);
    const safe = m.domain.trim().replace(/[^a-zA-Z0-9._-]/g, '_');

    let path: string;
    let group: ProxyHost['group'];
    let makeSymlink = false;
    if (editing) {
      path = editing.path;
      group = editing.group;
    } else if (isDocker(target)) {
      path = `${NGINX.confd}/${safe}.conf`;
      group = 'confd';
    } else {
      path = `${NGINX.avail}/${safe}.conf`;
      group = 'site';
      makeSymlink = true;
    }

    const parts = [
      `mkdir -p ${shQuote(path.substring(0, path.lastIndexOf('/')))}`,
      writeFileCmd(path, content),
    ];
    if (makeSymlink) {
      parts.push(`ln -sf ${shQuote(path)} ${shQuote(`${NGINX.enabled}/${safe}.conf`)}`);
    }
    parts.push(reloadCmd());
    const cmd = wrapCmd(target, parts.join(' && '));

    showHostModal = false;
    await withSudo(async () => {
      await invoke('exec_custom_command', { cmd, useSudo: true });
      notifications.success(get(LL).webserver.savedHost({ name: m.domain }));
      await loadHosts();
    });
  }

  function toggleHost(h: ProxyHost) {
    let cmd: string;
    if (h.group === 'site') {
      const base = h.path.split('/').pop() || '';
      cmd = h.enabled
        ? `rm -f ${shQuote(`${NGINX.enabled}/${base}`)}`
        : `ln -sf ${shQuote(h.path)} ${shQuote(`${NGINX.enabled}/${base}`)}`;
    } else {
      const newPath = h.enabled ? `${h.path}.disabled` : h.path.replace(/\.disabled$/, '');
      cmd = `mv ${shQuote(h.path)} ${shQuote(newPath)}`;
    }
    withSudo(async () => {
      await invoke('exec_custom_command', { cmd: wrapCmd(target, `${cmd} && ${reloadCmd()}`), useSudo: true });
      notifications.success(
        h.enabled
          ? get(LL).webserver.hostDisabled({ name: h.meta.domain })
          : get(LL).webserver.hostEnabled({ name: h.meta.domain }),
      );
      await loadHosts();
    });
  }

  function deleteHost(h: ProxyHost) {
    if (!confirm(get(LL).webserver.confirmDeleteHost({ name: h.meta.domain }))) return;
    const cmds = [`rm -f ${shQuote(h.path)}`];
    if (h.group === 'site') {
      const base = h.path.split('/').pop() || '';
      cmds.push(`rm -f ${shQuote(`${NGINX.enabled}/${base}`)}`);
    }
    cmds.push(reloadCmd());
    withSudo(async () => {
      await invoke('exec_custom_command', { cmd: wrapCmd(target, cmds.join('; ')), useSudo: true });
      notifications.success(get(LL).common.delete());
      await loadHosts();
    });
  }

  // ---------------------------------------------------------------------------
  // SSL Certificates
  // ---------------------------------------------------------------------------
  let certs = $state<Cert[]>([]);
  let certbotInstalled = $state(false);
  let installedPlugins = $state<string[]>([]);

  function parseCerts(out: string): Cert[] {
    const result: Cert[] = [];
    const blocks = out.split(/Certificate Name:/).slice(1);
    for (const b of blocks) {
      const name = (b.split('\n')[0] || '').trim();
      const domains = (b.match(/Domains:\s*(.*)/)?.[1] || '').trim();
      const expiryLine = (b.match(/Expiry Date:\s*(.*)/)?.[1] || '').trim();
      const path = (b.match(/Certificate Path:\s*(.*)/)?.[1] || '').trim();
      const daysMatch = expiryLine.match(/VALID:\s*(\d+)\s*day/);
      const valid = /VALID/.test(expiryLine) && !/INVALID/.test(expiryLine);
      result.push({
        name,
        domains,
        expiry: expiryLine.replace(/\s*\((VALID|INVALID).*\)/, '').trim(),
        daysLeft: daysMatch ? parseInt(daysMatch[1]) : null,
        valid,
        path,
      });
    }
    return result;
  }

  async function loadCerts() {
    isLoading = true;
    errorMsg = '';
    certbotInstalled = false;
    const cmd = wrapCmd(
      target,
      `if command -v certbot >/dev/null 2>&1; then echo '@@@HAS_CERTBOT'; echo '@@@PLUGINS'; certbot plugins 2>/dev/null || true; echo '@@@CERTS'; certbot certificates 2>&1 || true; else echo '@@@NO_CERTBOT'; fi`,
    );
    await withSudo(async () => {
      const out = await invoke<string>('exec_custom_command', { cmd, useSudo: true });
      if (out.includes('@@@NO_CERTBOT') || !out.includes('@@@HAS_CERTBOT')) {
        certbotInstalled = false;
        certs = [];
        installedPlugins = [];
        return;
      }
      certbotInstalled = true;
      const pluginsPart = out.split('@@@PLUGINS')[1]?.split('@@@CERTS')[0] || '';
      installedPlugins = (pluginsPart.match(/dns-[a-z0-9]+/g) || []).map((s) => s);
      const certsPart = out.split('@@@CERTS')[1] || '';
      certs = parseCerts(certsPart);
    });
    isLoading = false;
  }

  function installCertbot() {
    const cmd = wrapCmd(
      target,
      `if command -v apt-get >/dev/null 2>&1; then apt-get update && apt-get install -y certbot; ` +
        `elif command -v apk >/dev/null 2>&1; then apk add --no-cache certbot; ` +
        `elif command -v dnf >/dev/null 2>&1; then dnf install -y certbot; ` +
        `elif command -v yum >/dev/null 2>&1; then yum install -y certbot; ` +
        `else echo 'No supported package manager found'; fi`,
    );
    runStreamed(cmd, get(LL).webserver.installingCertbot(), loadCerts);
  }

  function installPlugin(provider: DnsProvider) {
    if (provider === 'manual') return;
    const cmd = wrapCmd(
      target,
      `if command -v apt-get >/dev/null 2>&1; then apt-get update && apt-get install -y python3-certbot-dns-${provider}; ` +
        `elif command -v apk >/dev/null 2>&1; then apk add --no-cache certbot-${provider} || pip3 install certbot-dns-${provider}; ` +
        `elif command -v pip3 >/dev/null 2>&1; then pip3 install certbot-dns-${provider}; ` +
        `else echo 'No supported package manager found'; fi`,
    );
    runStreamed(cmd, get(LL).webserver.installingPlugin({ plugin: provider }), loadCerts);
  }

  // ---- issue modal ----
  let showIssue = $state(false);
  let issueDomains = $state('');
  let issueEmail = $state('');
  let issueMethod = $state<'nginx' | 'webroot' | 'standalone' | 'dns'>('nginx');
  let issueWebroot = $state('/var/www/html');
  let dnsProvider = $state<DnsProvider>('cloudflare');
  let dnsToken = $state('');
  let dnsAccessKey = $state('');
  let dnsSecretKey = $state('');

  const pluginInstalled = $derived(
    dnsProvider === 'manual' || installedPlugins.includes(`dns-${dnsProvider}`),
  );

  function credentialsContent(provider: DnsProvider): string {
    if (provider === 'cloudflare') return `dns_cloudflare_api_token = ${dnsToken.trim()}\n`;
    if (provider === 'digitalocean') return `dns_digitalocean_token = ${dnsToken.trim()}\n`;
    return '';
  }

  function submitIssue() {
    const domains = issueDomains.split(',').map((d) => d.trim()).filter(Boolean);
    if (domains.length === 0) {
      errorMsg = get(LL).webserver.needDomains();
      return;
    }
    const dArgs = domains.map((d) => `-d ${shQuote(d)}`).join(' ');
    const email = issueEmail.trim()
      ? `-m ${shQuote(issueEmail.trim())} --agree-tos --no-eff-email`
      : `--register-unsafely-without-email --agree-tos`;

    let pre = '';
    let base: string;
    if (issueMethod === 'nginx') {
      base = `certbot --nginx ${dArgs} ${email} --non-interactive`;
    } else if (issueMethod === 'standalone') {
      base = `certbot certonly --standalone ${dArgs} ${email} --non-interactive`;
    } else if (issueMethod === 'webroot') {
      base = `certbot certonly --webroot -w ${shQuote(issueWebroot)} ${dArgs} ${email} --non-interactive`;
    } else {
      // DNS challenge
      const p = dnsProvider;
      if (p === 'manual') {
        base = `certbot certonly --manual --preferred-challenges dns ${dArgs} ${email}`;
      } else if (p === 'route53') {
        base =
          `AWS_ACCESS_KEY_ID=${shQuote(dnsAccessKey.trim())} AWS_SECRET_ACCESS_KEY=${shQuote(dnsSecretKey.trim())} ` +
          `certbot certonly --dns-route53 ${dArgs} ${email} --non-interactive`;
      } else {
        const credPath = `/etc/letsencrypt/credentials/dns-${p}.ini`;
        pre =
          `mkdir -p /etc/letsencrypt/credentials && ` +
          `${writeFileCmd(credPath, credentialsContent(p))} && chmod 600 ${shQuote(credPath)} && `;
        base = `certbot certonly --dns-${p} --dns-${p}-credentials ${shQuote(credPath)} ${dArgs} ${email} --non-interactive`;
      }
    }
    const cmd = wrapCmd(target, pre + base);
    showIssue = false;
    runStreamed(cmd, get(LL).webserver.issueTitle(), loadCerts);
  }

  function renewAll() {
    runStreamed(wrapCmd(target, 'certbot renew'), get(LL).webserver.renewAll(), loadCerts);
  }
  function dryRun() {
    runStreamed(wrapCmd(target, 'certbot renew --dry-run'), get(LL).webserver.dryRun(), loadCerts);
  }
  function renewCert(c: Cert) {
    runStreamed(
      wrapCmd(target, `certbot renew --cert-name ${shQuote(c.name)} --force-renewal`),
      `${get(LL).webserver.renew()}: ${c.name}`,
      loadCerts,
    );
  }
  function deleteCert(c: Cert) {
    if (!confirm(get(LL).webserver.confirmDeleteCert({ name: c.name }))) return;
    runStreamed(
      wrapCmd(target, `certbot delete --cert-name ${shQuote(c.name)} --non-interactive`),
      `${get(LL).common.delete()}: ${c.name}`,
      loadCerts,
    );
  }

  // ---------------------------------------------------------------------------
  // Nginx Files (raw)
  // ---------------------------------------------------------------------------
  let files = $state<NginxFile[]>([]);

  async function loadFiles() {
    isLoading = true;
    errorMsg = '';
    const cmd = wrapCmd(
      target,
      `echo '@@@AVAIL'; ls -1 ${shQuote(NGINX.avail)} 2>/dev/null || true; ` +
        `echo '@@@ENABLED'; ls -1 ${shQuote(NGINX.enabled)} 2>/dev/null || true; ` +
        `echo '@@@CONFD'; ls -1 ${shQuote(NGINX.confd)} 2>/dev/null || true`,
    );
    await withSudo(async () => {
      const out = await invoke<string>('exec_custom_command', { cmd, useSudo: true });
      const sec: Record<string, string[]> = { AVAIL: [], ENABLED: [], CONFD: [] };
      let cur = '';
      for (const line of out.split('\n')) {
        if (line.startsWith('@@@')) { cur = line.slice(3).trim(); continue; }
        const v = line.trim();
        if (v && sec[cur]) sec[cur].push(v);
      }
      const list: NginxFile[] = [];
      for (const name of sec.AVAIL) list.push({ name, path: `${NGINX.avail}/${name}`, group: 'available' });
      for (const name of sec.ENABLED) list.push({ name, path: `${NGINX.enabled}/${name}`, group: 'enabled' });
      for (const name of sec.CONFD) list.push({ name, path: `${NGINX.confd}/${name}`, group: 'confd' });
      files = list;
    });
    isLoading = false;
  }

  // ---- raw editor ----
  let editPath = $state<string | null>(null);
  let editContent = $state('');
  let editSaving = $state(false);

  async function openEdit(path: string) {
    editPath = path;
    editContent = get(LL).common.loading();
    try {
      if (isDocker(target)) {
        editContent = await invoke<string>('exec_custom_command', { cmd: wrapCmd(target, `cat ${shQuote(path)}`), useSudo: true });
      } else {
        editContent = await invoke<string>('sftp_read', { path });
      }
    } catch {
      try {
        editContent = await invoke<string>('exec_custom_command', { cmd: wrapCmd(target, `cat ${shQuote(path)}`), useSudo: true });
      } catch (e2) {
        editContent = '';
        errorMsg = get(LL).webserver.readFailed({ error: formatInvokeError(e2) });
      }
    }
  }

  async function saveEdit() {
    if (!editPath) return;
    editSaving = true;
    const path = editPath;
    const content = editContent;
    await withSudo(async () => {
      const cmd = wrapCmd(target, writeFileCmd(path, content));
      await invoke('exec_custom_command', { cmd, useSudo: true });
      notifications.success(get(LL).webserver.saved());
      editPath = null;
    });
    editSaving = false;
  }

  // ---------------------------------------------------------------------------
  // Nginx Control
  // ---------------------------------------------------------------------------
  let controlOutput = $state('');

  function testConfig() {
    withSudo(async () => {
      const out = await invoke<string>('exec_custom_command', { cmd: wrapCmd(target, 'nginx -t 2>&1'), useSudo: true });
      controlOutput = out.trim();
      if (/successful/i.test(out)) notifications.success(get(LL).webserver.configOk());
      else notifications.error(out.trim().slice(0, 300));
    });
  }
  function reloadNginx() {
    withSudo(async () => {
      await invoke('exec_custom_command', { cmd: wrapCmd(target, reloadCmd()), useSudo: true });
      notifications.success(get(LL).webserver.reloaded());
    });
  }
  function restartNginx() {
    if (!confirm(get(LL).webserver.confirmRestart())) return;
    const cmd = isDocker(target) ? 'nginx -s stop; nginx' : 'systemctl restart nginx';
    withSudo(async () => {
      await invoke('exec_custom_command', { cmd: wrapCmd(target, cmd), useSudo: true });
      notifications.success(get(LL).webserver.restarted());
    });
  }
  function checkStatus() {
    const cmd = isDocker(target)
      ? 'nginx -v 2>&1; ps aux | grep -i "[n]ginx" || true'
      : 'systemctl status nginx --no-pager 2>&1 | head -n 20 || true';
    withSudo(async () => {
      controlOutput = await invoke<string>('exec_custom_command', { cmd: wrapCmd(target, cmd), useSudo: true });
    });
  }

  // ---------------------------------------------------------------------------
  // sub-tab dispatch
  // ---------------------------------------------------------------------------
  function loadCurrent() {
    if (!activeProfileId) return;
    if (subTab === 'hosts') loadHosts();
    else if (subTab === 'certs') loadCerts();
    else if (subTab === 'files') loadFiles();
    else if (subTab === 'control') checkStatus();
  }

  function switchTab(t: SubTab) {
    subTab = t;
    loadCurrent();
  }

  onMount(async () => {
    loadProfiles();
    await loadContainers();
  });
</script>

<div class="npm manager-shell fade-in">
  <header class="manager-header">
    <h1 class="page-title">{$LL.webserver.title()}</h1>
    <div class="header-actions">
      {#if profiles.length > 0}
        <!-- Profile Selector -->
        <div class="profile-selector glass">
          <span class="ps-label">{$LL.webserver.activeProfile()}:</span>
          <select bind:value={activeProfileId} class="profile-select" onchange={handleProfileChange}>
            {#each profiles as p}
              <option value={p.id}>{p.name} ({p.target.kind === 'host' ? $LL.execTarget.host() : p.target.container})</option>
            {/each}
          </select>
          <button class="icon-btn-compact" onclick={() => { showProfilesModal = true; }} title={$LL.webserver.manageProfiles()}>
            <Settings2 size={14} />
          </button>
        </div>
      {/if}

      <button class="secondary btn-compact" disabled={isLoading || profiles.length === 0} onclick={loadCurrent}>
        <RefreshCw size={14} class={isLoading ? 'spin' : ''} /> {$LL.common.refresh()}
      </button>
    </div>
  </header>

  {#if profiles.length === 0}
    <!-- SETUP VIEW -->
    <div class="setup-container fade-in">
      <div class="setup-card glass">
        <div class="setup-header">
          <Server size={32} class="accent" />
          <h2>{$LL.webserver.setupTitle()}</h2>
          <p>{$LL.webserver.setupDesc()}</p>
        </div>

        <div class="form-group">
          <label for="setup-name">{$LL.webserver.profileName()}</label>
          <input id="setup-name" type="text" bind:value={profileFormName} placeholder={$LL.webserver.profileNamePlaceholder()} />
        </div>

        <div class="form-group">
          <label>{$LL.webserver.nginxTarget()}</label>
          <div class="target-options">
            <label class="target-radio" class:active={profileFormKind === 'host'}>
              <input type="radio" name="setup-kind" value="host" bind:group={profileFormKind} />
              <span>{$LL.webserver.nginxTargetHost()}</span>
            </label>
            <label class="target-radio" class:active={profileFormKind === 'docker'}>
              <input type="radio" name="setup-kind" value="docker" bind:group={profileFormKind} />
              <span>{$LL.webserver.nginxTargetDocker()}</span>
            </label>
          </div>
        </div>

        {#if profileFormKind === 'docker'}
          <div class="form-group fade-in">
            <label for="setup-container">{$LL.webserver.selectContainer()}</label>
            <div class="select-row">
              <select id="setup-container" bind:value={profileFormContainer}>
                {#each containers as c}
                  <option value={c}>{c}</option>
                {/each}
              </select>
              <button class="secondary btn-compact" onclick={loadContainers} disabled={loadingContainers}>
                <RefreshCw size={14} class={loadingContainers ? 'spin' : ''} />
              </button>
            </div>
          </div>
        {/if}

        <div class="setup-actions">
          <button class="primary" onclick={saveProfileForm} disabled={!profileFormName.trim()}>
            <Save size={14} /> {$LL.webserver.saveProfile()}
          </button>
        </div>
      </div>
    </div>
  {:else}
    <!-- TABS VIEW -->
    <div class="subtabs">
      <button class="subtab" class:active={subTab === 'hosts'} onclick={() => switchTab('hosts')}><Network size={14} /> {$LL.webserver.tabHosts()}</button>
      <button class="subtab" class:active={subTab === 'certs'} onclick={() => switchTab('certs')}><Lock size={14} /> {$LL.webserver.tabCerts()}</button>
      <button class="subtab" class:active={subTab === 'files'} onclick={() => switchTab('files')}><FileCode size={14} /> {$LL.webserver.tabFiles()}</button>
      <button class="subtab" class:active={subTab === 'control'} onclick={() => switchTab('control')}><Settings2 size={14} /> {$LL.webserver.tabControl()}</button>
    </div>

  <!-- ================= PROXY HOSTS ================= -->
  {#if subTab === 'hosts'}
    <div class="tab-toolbar">
      <button class="secondary btn-compact" onclick={openAddHost}><Plus size={14} /> {$LL.webserver.addHost()}</button>
    </div>
    <div class="card-list">
      {#each hosts as h (h.path)}
        <div class="card glass">
          <Globe2 size={16} class="glyph" />
          <div class="card-info">
            <span class="card-name">{h.meta.domain}</span>
            <span class="card-sub mono">{$LL.webserver.forwardsTo()} {h.meta.scheme}://{h.meta.forwardHost}:{h.meta.forwardPort}</span>
          </div>
          {#if h.meta.sslEnabled}<span class="badge success"><Lock size={11} /> SSL</span>{/if}
          <span class="badge" class:success={h.enabled} class:muted={!h.enabled}>{h.enabled ? $LL.webserver.enabledLabel() : $LL.webserver.disabledLabel()}</span>
          <div class="card-actions">
            <button class="row-btn" onclick={() => openEditHost(h)} title={$LL.common.edit()}><FileCode size={13} /></button>
            {#if h.enabled}
              <button class="row-btn warn" onclick={() => toggleHost(h)} title={$LL.common.disable()}><PowerOff size={13} /></button>
            {:else}
              <button class="row-btn" onclick={() => toggleHost(h)} title={$LL.common.enable()}><Power size={13} /></button>
            {/if}
            <button class="row-btn danger" onclick={() => deleteHost(h)} title={$LL.common.delete()}><Trash2 size={13} /></button>
          </div>
        </div>
      {/each}
      {#if hosts.length === 0 && !isLoading}
        <div class="empty glass"><Globe2 size={22} /> {$LL.webserver.noHosts()}</div>
      {/if}
    </div>
  {/if}

  <!-- ================= SSL CERTIFICATES ================= -->
  {#if subTab === 'certs'}
    <div class="tab-toolbar">
      <button class="secondary btn-compact" disabled={!certbotInstalled} onclick={() => (showIssue = true)}><Plus size={14} /> {$LL.webserver.issueBtn()}</button>
      <button class="secondary btn-compact" disabled={!certbotInstalled} onclick={dryRun}><FlaskConical size={14} /> {$LL.webserver.dryRun()}</button>
      <button class="secondary btn-compact" disabled={!certbotInstalled} onclick={renewAll}><RotateCw size={14} /> {$LL.webserver.renewAll()}</button>
    </div>

    {#if !certbotInstalled}
      <div class="hint glass warn">
        <ShieldAlert size={16} />
        <span>{$LL.webserver.certNotInstalled()}</span>
        <button class="secondary btn-compact" onclick={installCertbot}><Download size={13} /> {$LL.webserver.installCertbot()}</button>
      </div>
    {/if}

    <div class="card-list">
      {#each certs as c (c.name)}
        <div class="card glass">
          <div class="cert-icon" class:ok={c.valid && (c.daysLeft ?? 0) > 14} class:soon={c.valid && (c.daysLeft ?? 0) <= 14} class:bad={!c.valid}>
            {#if c.valid}<ShieldCheck size={18} />{:else}<ShieldAlert size={18} />{/if}
          </div>
          <div class="card-info">
            <span class="card-name">{c.name}</span>
            <span class="card-sub">{c.domains}</span>
            <span class="card-sub mono">
              {c.expiry}
              {#if c.daysLeft !== null}· <span class="days" class:soon={c.daysLeft <= 14} class:bad={c.daysLeft <= 3}>{$LL.webserver.daysLeft({ days: c.daysLeft })}</span>{/if}
            </span>
          </div>
          <div class="card-actions">
            <button class="row-btn" onclick={() => renewCert(c)} title={$LL.webserver.renew()}><RotateCw size={13} /></button>
            <button class="row-btn danger" onclick={() => deleteCert(c)} title={$LL.common.delete()}><Trash2 size={13} /></button>
          </div>
        </div>
      {/each}
      {#if certs.length === 0 && certbotInstalled && !isLoading}
        <div class="empty glass"><Lock size={22} /> {$LL.webserver.noCerts()}</div>
      {/if}
    </div>
  {/if}

  <!-- ================= NGINX FILES ================= -->
  {#if subTab === 'files'}
    <div class="card-list">
      {#each files as f (f.path)}
        <div class="card glass">
          <FileCode size={16} class="glyph" />
          <div class="card-info">
            <span class="card-name">{f.name}</span>
            <span class="card-sub mono">{f.path}</span>
          </div>
          <span class="badge">{f.group}</span>
          <div class="card-actions">
            <button class="row-btn" onclick={() => openEdit(f.path)} title={$LL.common.edit()}><FileCode size={13} /></button>
          </div>
        </div>
      {/each}
      {#if files.length === 0 && !isLoading}
        <div class="empty glass"><FileCode size={22} /> {$LL.webserver.noFiles()}</div>
      {/if}
    </div>
  {/if}

  <!-- ================= CONTROL ================= -->
  {#if subTab === 'control'}
    <div class="tab-toolbar">
      <button class="secondary btn-compact" onclick={testConfig}><CheckCircle size={14} /> {$LL.webserver.testConfig()}</button>
      <button class="secondary btn-compact" onclick={reloadNginx}><RotateCw size={14} /> {$LL.webserver.reload()}</button>
      <button class="secondary btn-compact" onclick={restartNginx}><Power size={14} /> {$LL.webserver.restart()}</button>
      <button class="secondary btn-compact" onclick={checkStatus}><Server size={14} /> {$LL.webserver.checkStatus()}</button>
    </div>
    <pre class="control-output glass">{controlOutput || $LL.webserver.statusHint()}</pre>
  {/if}
  {/if}
</div>

<!-- ===================== Profiles Manager Modal ===================== -->
{#if showProfilesModal}
  <div class="modal-overlay" role="presentation" onclick={() => (showProfilesModal = false)}>
    <div class="modal-content glass profiles-modal" role="dialog" onclick={(e) => e.stopPropagation()}>
      <div class="modal-header">
        <h3>{$LL.webserver.manageProfiles()}</h3>
        <button class="icon-btn-compact" onclick={() => (showProfilesModal = false)}><X size={16} /></button>
      </div>

      <div class="modal-body flex-col gap-md">
        <!-- List of Profiles -->
        <div class="profiles-list-section">
          <h4>{$LL.webserver.profilesTitle()}</h4>
          <div class="profiles-grid">
            {#each profiles as p}
              <div class="profile-item glass" class:active={p.id === activeProfileId}>
                <div class="profile-details">
                  <span class="profile-title">{p.name}</span>
                  <span class="profile-subtitle mono">{p.target.kind === 'host' ? $LL.execTarget.host() : p.target.container}</span>
                </div>
                <div class="profile-actions">
                  <button class="row-btn" onclick={() => openEditProfile(p)} title={$LL.common.edit()}><FileCode size={13} /></button>
                  <button class="row-btn danger" onclick={() => deleteProfile(p.id)} title={$LL.common.delete()}><Trash2 size={13} /></button>
                </div>
              </div>
            {/each}
          </div>
          <button class="secondary btn-compact mt-sm" onclick={openAddProfile}>
            <Plus size={14} /> {$LL.webserver.addProfile()}
          </button>
        </div>

        <!-- Dependency Status for Active Profile -->
        {#if activeProfileId}
          <div class="dep-status-section glass mt-sm">
            <h4 class="flex-align gap-xs"><Settings2 size={14} /> {$LL.webserver.requirementsTitle()}</h4>
            <p class="desc-text mt-xs">{$LL.webserver.requirementsDesc()}</p>
            
            <div class="dep-list mt-sm">
              <div class="dep-item">
                <span>Certbot:</span>
                {#if certbotInstalled}
                  <span class="badge success-badge">{$LL.webserver.enabledLabel()}</span>
                {:else}
                  <span class="badge danger-badge">{$LL.webserver.disabledLabel()}</span>
                {/if}
              </div>
              <div class="dep-item">
                <span>DNS Cloudflare:</span>
                {#if installedPlugins.includes('dns-cloudflare')}
                  <span class="badge success-badge">{$LL.webserver.enabledLabel()}</span>
                {:else}
                  <span class="badge danger-badge">{$LL.webserver.disabledLabel()}</span>
                {/if}
              </div>
            </div>

            {#if !certbotInstalled || !installedPlugins.includes('dns-cloudflare')}
              <button class="primary btn-compact mt-md w-full flex-center gap-xs" onclick={installAllRequirements}>
                <Download size={14} /> {$LL.webserver.installBtn()}
              </button>
            {:else}
              <div class="success-message mt-md">
                <CheckCircle size={14} /> {$LL.webserver.requirementsInstalled()}
              </div>
            {/if}
          </div>
        {/if}
      </div>

      <!-- Add/Edit form inline in the modal when active -->
      {#if showProfilesModal && (profileFormId && !profiles.some(p => p.id === profileFormId) || isEditingProfile)}
        <div class="profile-form-section glass mt-md p-md">
          <h4>{isEditingProfile ? $LL.webserver.editProfile() : $LL.webserver.addProfile()}</h4>
          
          <div class="form-group mt-sm">
            <label for="form-name">{$LL.webserver.profileName()}</label>
            <input id="form-name" type="text" bind:value={profileFormName} placeholder={$LL.webserver.profileNamePlaceholder()} />
          </div>

          <div class="form-group">
            <label>{$LL.webserver.nginxTarget()}</label>
            <div class="target-options">
              <label class="target-radio" class:active={profileFormKind === 'host'}>
                <input type="radio" name="form-kind" value="host" bind:group={profileFormKind} />
                <span>{$LL.webserver.nginxTargetHost()}</span>
              </label>
              <label class="target-radio" class:active={profileFormKind === 'docker'}>
                <input type="radio" name="form-kind" value="docker" bind:group={profileFormKind} />
                <span>{$LL.webserver.nginxTargetDocker()}</span>
              </label>
            </div>
          </div>

          {#if profileFormKind === 'docker'}
            <div class="form-group">
              <label for="form-container">{$LL.webserver.selectContainer()}</label>
              <div class="select-row">
                <select id="form-container" bind:value={profileFormContainer}>
                  {#each containers as c}
                    <option value={c}>{c}</option>
                  {/each}
                </select>
                <button class="secondary btn-compact" onclick={loadContainers} disabled={loadingContainers}>
                  <RefreshCw size={14} class={loadingContainers ? 'spin' : ''} />
                </button>
              </div>
            </div>
          {/if}

          <div class="modal-actions mt-md">
            <button class="secondary" onclick={() => { isEditingProfile = false; profileFormId = ''; }}>{$LL.common.cancel()}</button>
            <button class="primary" onclick={saveProfileForm} disabled={!profileFormName.trim()}><Save size={14} /> {$LL.common.save()}</button>
          </div>
        </div>
      {/if}
    </div>
  </div>
{/if}

<!-- ===================== Host editor modal ===================== -->
{#if showHostModal}
  <div class="modal-overlay" role="presentation" onclick={() => (showHostModal = false)}>
    <div class="modal-content glass host-modal" role="dialog" onclick={(e) => e.stopPropagation()}>
      <div class="modal-header">
        <h3>{editing ? $LL.webserver.modalEditTitle() : $LL.webserver.modalAddTitle()}</h3>
        <button class="icon-btn-compact" onclick={() => (showHostModal = false)}><X size={16} /></button>
      </div>

      <div class="modal-tabs">
        <button class="mtab" class:active={hostModalTab === 'details'} onclick={() => (hostModalTab = 'details')}>{$LL.webserver.tabDetails()}</button>
        <button class="mtab" class:active={hostModalTab === 'ssl'} onclick={() => (hostModalTab = 'ssl')}>{$LL.webserver.tabSsl()}</button>
        <button class="mtab" class:active={hostModalTab === 'advanced'} onclick={() => (hostModalTab = 'advanced')}>{$LL.webserver.tabAdvanced()}</button>
      </div>

      <div class="modal-body">
        {#if hostModalTab === 'details'}
          <div class="form-group">
            <label for="np-domain">{$LL.webserver.domain()}</label>
            <input id="np-domain" type="text" bind:value={form.domain} placeholder={$LL.webserver.domainPlaceholder()} />
          </div>
          <div class="form-row">
            <div class="form-group">
              <label for="np-scheme">{$LL.webserver.scheme()}</label>
              <select id="np-scheme" bind:value={form.scheme}>
                <option value="http">http</option>
                <option value="https">https</option>
              </select>
            </div>
            <div class="form-group grow">
              <label for="np-fhost">{$LL.webserver.forwardHost()}</label>
              <input id="np-fhost" type="text" bind:value={form.forwardHost} placeholder={$LL.webserver.forwardHostPlaceholder()} />
            </div>
            <div class="form-group">
              <label for="np-fport">{$LL.webserver.forwardPort()}</label>
              <input id="np-fport" type="number" bind:value={form.forwardPort} />
            </div>
          </div>
          <label class="check"><input type="checkbox" bind:checked={form.websockets} /> {$LL.webserver.websockets()}</label>
          <label class="check"><input type="checkbox" bind:checked={form.blockExploits} /> {$LL.webserver.blockExploits()}</label>
          <label class="check"><input type="checkbox" bind:checked={form.cacheAssets} /> {$LL.webserver.cacheAssets()}</label>
        {/if}

        {#if hostModalTab === 'ssl'}
          <label class="check"><input type="checkbox" bind:checked={form.sslEnabled} /> {$LL.webserver.sslEnabled()}</label>
          {#if form.sslEnabled}
            <div class="form-group">
              <label for="np-cert">{$LL.webserver.sslCert()}</label>
              <select id="np-cert" bind:value={form.sslCertName}>
                <option value="">{$LL.webserver.sslCertNone()}</option>
                {#each certs as c}<option value={c.name}>{c.name}</option>{/each}
              </select>
              <span class="field-hint">{$LL.webserver.sslCertHint()}</span>
            </div>
            <label class="check"><input type="checkbox" bind:checked={form.forceSsl} /> {$LL.webserver.forceSsl()}</label>
            <label class="check"><input type="checkbox" bind:checked={form.http2} /> {$LL.webserver.http2()}</label>
            <label class="check"><input type="checkbox" bind:checked={form.hstsEnabled} /> {$LL.webserver.hsts()}</label>
            {#if form.hstsEnabled}
              <label class="check indent"><input type="checkbox" bind:checked={form.hstsSubdomains} /> {$LL.webserver.hstsSubdomains()}</label>
              <label class="check indent"><input type="checkbox" bind:checked={form.hstsPreload} /> {$LL.webserver.hstsPreload()}</label>
            {/if}
          {/if}
        {/if}

        {#if hostModalTab === 'advanced'}
          <div class="form-group">
            <label for="np-adv">{$LL.webserver.advancedConfig()}</label>
            <textarea id="np-adv" class="adv-area" bind:value={form.advancedConfig} spellcheck="false" placeholder={$LL.webserver.advancedPlaceholder()}></textarea>
          </div>
        {/if}
      </div>

      <div class="modal-actions">
        <button class="secondary" onclick={() => (showHostModal = false)}>{$LL.common.cancel()}</button>
        <button class="primary" onclick={saveHost}><Save size={14} /> {$LL.common.save()}</button>
      </div>
    </div>
  </div>
{/if}

<!-- ===================== Issue certificate modal ===================== -->
{#if showIssue}
  <div class="modal-overlay" role="presentation" onclick={() => (showIssue = false)}>
    <div class="modal-content glass" role="dialog" onclick={(e) => e.stopPropagation()}>
      <div class="modal-header"><h3>{$LL.webserver.issueTitle()}</h3><button class="icon-btn-compact" onclick={() => (showIssue = false)}><X size={16} /></button></div>
      <div class="form-group">
        <label for="ssl-domains">{$LL.webserver.domainsField()}</label>
        <input id="ssl-domains" type="text" bind:value={issueDomains} placeholder={$LL.webserver.domainsPlaceholder()} />
      </div>
      <div class="form-group">
        <label for="ssl-email">{$LL.webserver.email()}</label>
        <input id="ssl-email" type="text" bind:value={issueEmail} placeholder={$LL.webserver.emailPlaceholder()} />
      </div>
      <div class="form-group">
        <label for="ssl-method">{$LL.webserver.method()}</label>
        <select id="ssl-method" bind:value={issueMethod}>
          <option value="nginx">{$LL.webserver.methodNginx()}</option>
          <option value="webroot">{$LL.webserver.methodWebroot()}</option>
          <option value="standalone">{$LL.webserver.methodStandalone()}</option>
          <option value="dns">{$LL.webserver.methodDns()}</option>
        </select>
      </div>
      {#if issueMethod === 'webroot'}
        <div class="form-group">
          <label for="ssl-webroot">{$LL.webserver.webrootPath()}</label>
          <input id="ssl-webroot" type="text" bind:value={issueWebroot} />
        </div>
      {/if}
      {#if issueMethod === 'dns'}
        <div class="form-group">
          <label for="ssl-dns">{$LL.webserver.dnsProvider()}</label>
          <select id="ssl-dns" bind:value={dnsProvider}>
            <option value="cloudflare">Cloudflare</option>
            <option value="digitalocean">DigitalOcean</option>
            <option value="route53">AWS Route53</option>
            <option value="manual">{$LL.webserver.providerManual()}</option>
          </select>
        </div>
        {#if !pluginInstalled}
          <div class="hint warn">
            <ShieldAlert size={14} />
            <span>{$LL.webserver.pluginMissing({ plugin: `dns-${dnsProvider}` })}</span>
            <button class="secondary btn-compact" onclick={() => installPlugin(dnsProvider)}><Download size={12} /> {$LL.webserver.installPlugin()}</button>
          </div>
        {/if}
        {#if dnsProvider === 'cloudflare' || dnsProvider === 'digitalocean'}
          <div class="form-group">
            <label for="dns-token">{$LL.webserver.dnsToken()}</label>
            <input id="dns-token" type="password" bind:value={dnsToken} placeholder={$LL.webserver.dnsTokenPlaceholder()} />
          </div>
        {/if}
        {#if dnsProvider === 'route53'}
          <div class="form-group">
            <label for="dns-ak">{$LL.webserver.dnsAccessKey()}</label>
            <input id="dns-ak" type="text" bind:value={dnsAccessKey} />
          </div>
          <div class="form-group">
            <label for="dns-sk">{$LL.webserver.dnsSecretKey()}</label>
            <input id="dns-sk" type="password" bind:value={dnsSecretKey} />
          </div>
        {/if}
        {#if dnsProvider === 'manual'}
          <span class="field-hint">{$LL.webserver.manualHint()}</span>
        {/if}
      {/if}
      <div class="modal-actions">
        <button class="secondary" onclick={() => (showIssue = false)}>{$LL.common.cancel()}</button>
        <button class="primary" onclick={submitIssue}>{$LL.webserver.issue()}</button>
      </div>
    </div>
  </div>
{/if}

<!-- ===================== Raw file editor modal ===================== -->
{#if editPath}
  <div class="modal-overlay" role="presentation" onclick={() => (editPath = null)}>
    <div class="modal-content glass edit-modal" role="dialog" onclick={(e) => e.stopPropagation()}>
      <div class="modal-header"><h3 class="mono">{editPath}</h3><button class="icon-btn-compact" onclick={() => (editPath = null)}><X size={16} /></button></div>
      <textarea class="config-area" bind:value={editContent} spellcheck="false"></textarea>
      <div class="modal-actions">
        <button class="secondary" onclick={() => (editPath = null)}>{$LL.common.cancel()}</button>
        <button class="primary" disabled={editSaving} onclick={saveEdit}><Save size={14} /> {editSaving ? $LL.common.loading() : $LL.common.save()}</button>
      </div>
    </div>
  </div>
{/if}

<!-- ===================== Streamed output modal ===================== -->
{#if showOutput}
  <div class="modal-overlay" role="presentation" onclick={() => (showOutput = false)}>
    <div class="modal-content glass output-modal" role="dialog" onclick={(e) => e.stopPropagation()}>
      <div class="modal-header"><h3>{outputTitle}</h3><button class="icon-btn-compact" onclick={() => (showOutput = false)}><X size={16} /></button></div>
      <pre class="output" use:stickToBottom>{outputText}</pre>
    </div>
  </div>
{/if}

<SudoModal bind:open={showSudoModal} onSuccess={() => { const a = pendingAction; pendingAction = null; if (a) a(); }} onCancel={() => (pendingAction = null)} />

<style>
  .header-actions { display: flex; gap: 6px; flex-wrap: wrap; }
  .control-bar { display: flex; align-items: center; gap: 12px; padding: 10px; border-radius: var(--radius-md); flex-shrink: 0; flex-wrap: wrap; }
  .subtabs { display: flex; gap: 4px; flex-shrink: 0; border-bottom: 1px solid var(--border-color); }
  .subtab { display: flex; align-items: center; gap: 6px; background: transparent; border: none; color: var(--text-secondary); padding: 8px 14px; font-size: 0.8rem; cursor: pointer; border-bottom: 2px solid transparent; }
  .subtab:hover { color: var(--text-primary); }
  .subtab.active { color: var(--accent-amber); border-bottom-color: var(--accent-amber); }
  .tab-toolbar { display: flex; gap: 6px; flex-wrap: wrap; flex-shrink: 0; }
  .card-list { flex: 1; overflow: auto; display: flex; flex-direction: column; gap: 8px; }
  .card { display: flex; align-items: center; gap: 12px; padding: 12px 14px; border-radius: var(--radius-md); }
  .card-info { flex: 1; min-width: 0; display: flex; flex-direction: column; }
  .card-name { font-weight: 600; color: var(--text-primary); font-size: 0.85rem; }
  .card-sub { font-size: 0.72rem; color: var(--text-muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .mono { font-family: var(--font-mono); }
  .badge.muted { opacity: 0.55; }
  .badge.success { display: inline-flex; align-items: center; gap: 4px; }
  .card-actions { display: flex; gap: 6px; flex-shrink: 0; }
  .row-btn { background: transparent; border: 1px solid var(--border-color); color: var(--text-secondary); border-radius: var(--radius-sm); padding: 6px 8px; cursor: pointer; }
  .row-btn:hover { background: var(--bg-hover); color: var(--text-primary); }
  .row-btn.warn:hover { color: var(--accent-amber); border-color: rgba(245,158,11,0.3); }
  .row-btn.danger:hover { color: var(--accent-red); border-color: rgba(239,68,68,0.3); }
  .cert-icon { display: flex; align-items: center; justify-content: center; width: 36px; height: 36px; border-radius: var(--radius-sm); flex-shrink: 0; }
  .cert-icon.ok { color: var(--accent-green); background: rgba(34,197,94,0.1); }
  .cert-icon.soon { color: var(--accent-amber); background: rgba(245,158,11,0.1); }
  .cert-icon.bad { color: var(--accent-red); background: rgba(239,68,68,0.1); }
  .days { font-weight: 600; color: var(--accent-green); }
  .days.soon { color: var(--accent-amber); }
  .days.bad { color: var(--accent-red); }
  .empty { display: flex; flex-direction: column; align-items: center; gap: 10px; padding: 40px; color: var(--text-muted); border-radius: var(--radius-md); }
  .hint { display: flex; align-items: center; gap: 8px; padding: 10px 12px; border-radius: var(--radius-md); font-size: 0.8rem; color: var(--text-secondary); flex-shrink: 0; flex-wrap: wrap; }
  .hint.warn { color: var(--accent-amber); border: 1px solid rgba(245,158,11,0.25); }
  .control-output { flex: 1; overflow: auto; padding: 12px; border-radius: var(--radius-md); font-family: var(--font-mono); font-size: 0.76rem; color: var(--text-secondary); white-space: pre-wrap; word-break: break-word; }

  .modal-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 12px; }
  .modal-actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 12px; }
  .modal-tabs { display: flex; gap: 4px; border-bottom: 1px solid var(--border-color); margin-bottom: 12px; }
  .mtab { background: transparent; border: none; color: var(--text-secondary); padding: 6px 12px; font-size: 0.78rem; cursor: pointer; border-bottom: 2px solid transparent; }
  .mtab.active { color: var(--accent-amber); border-bottom-color: var(--accent-amber); }
  .modal-body { min-height: 220px; }
  .host-modal { width: 560px; max-width: 94vw; max-height: 88vh; display: flex; flex-direction: column; }
  .form-group { display: flex; flex-direction: column; gap: 5px; margin-bottom: 12px; }
  .form-group.grow { flex: 1; }
  .form-row { display: flex; gap: 10px; align-items: flex-end; }
  .form-group label { font-size: 0.78rem; color: var(--text-secondary); }
  .form-group input, .form-group select, .adv-area { background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: var(--radius-sm); padding: 8px 10px; color: var(--text-primary); font-size: 0.85rem; }
  .field-hint { font-size: 0.7rem; color: var(--text-muted); }
  .check { display: flex; align-items: center; gap: 8px; font-size: 0.82rem; color: var(--text-secondary); margin-bottom: 10px; cursor: pointer; }
  .check.indent { margin-left: 22px; }
  .check input { width: auto; }
  .adv-area { min-height: 200px; font-family: var(--font-mono); resize: vertical; line-height: 1.5; }
  .edit-modal { width: 880px; max-width: 94vw; max-height: 86vh; display: flex; flex-direction: column; }
  .config-area { flex: 1; min-height: 420px; background: var(--bg-primary); border: 1px solid var(--border-color); border-radius: var(--radius-sm); padding: 12px; color: var(--text-primary); font-family: var(--font-mono); font-size: 0.8rem; resize: none; line-height: 1.5; }
  .output-modal { width: 820px; max-width: 94vw; max-height: 82vh; display: flex; flex-direction: column; }
  .output { flex: 1; overflow: auto; background: var(--bg-primary); border: 1px solid var(--border-color); border-radius: var(--radius-sm); padding: 12px; font-family: var(--font-mono); font-size: 0.76rem; color: var(--text-secondary); white-space: pre-wrap; word-break: break-word; margin-top: 4px; }

  /* Modal Overlay and Content styling (fixes missing modal styles) */
  .modal-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
    backdrop-filter: blur(4px);
  }
  .modal-content {
    background: var(--bg-primary);
    border: 1px solid var(--border-color);
    border-radius: var(--radius-lg);
    padding: 20px;
    box-shadow: var(--shadow-lg);
    display: flex;
    flex-direction: column;
  }

  /* Profile selector in header */
  .profile-selector {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 10px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-color);
  }
  .ps-label {
    font-size: 0.72rem;
    color: var(--text-muted);
    font-weight: 500;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }
  .profile-select {
    background: transparent;
    border: none;
    color: var(--accent-amber);
    font-size: 0.8rem;
    font-weight: 600;
    cursor: pointer;
    outline: none;
    padding-right: 4px;
  }
  .profile-select option {
    background: var(--bg-primary);
    color: var(--text-primary);
  }

  /* Setup view styling */
  .setup-container {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 40px 20px;
    min-height: 70vh;
  }
  .setup-card {
    width: 460px;
    padding: 30px;
    border-radius: var(--radius-lg);
    display: flex;
    flex-direction: column;
    gap: 16px;
    box-shadow: var(--shadow-md);
  }
  .setup-header {
    text-align: center;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    margin-bottom: 10px;
  }
  .setup-header h2 {
    font-size: 1.4rem;
    font-weight: 700;
    color: var(--text-primary);
  }
  .setup-header p {
    font-size: 0.82rem;
    color: var(--text-muted);
    line-height: 1.5;
  }
  .setup-header .accent {
    color: var(--accent-amber);
  }
  .target-options {
    display: flex;
    gap: 12px;
    margin-top: 4px;
  }
  .target-radio {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 10px;
    border: 1px solid var(--border-color);
    border-radius: var(--radius-sm);
    cursor: pointer;
    font-size: 0.82rem;
    color: var(--text-secondary);
    transition: all 0.2s ease;
  }
  .target-radio:hover {
    background: var(--bg-hover);
  }
  .target-radio input {
    margin: 0;
  }
  .target-radio input:checked + span {
    color: var(--accent-amber);
    font-weight: 600;
  }
  .select-row {
    display: flex;
    gap: 8px;
  }
  .select-row select {
    flex: 1;
  }
  .setup-actions {
    margin-top: 10px;
    display: flex;
    justify-content: flex-end;
  }

  /* Profiles Manager Modal Specifics */
  .profiles-modal {
    width: 500px;
    max-width: 94vw;
    max-height: 90vh;
    overflow-y: auto;
  }
  .profiles-list-section h4, .dep-status-section h4, .profile-form-section h4 {
    font-size: 0.85rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-muted);
    margin-bottom: 8px;
  }
  .profiles-grid {
    display: flex;
    flex-direction: column;
    gap: 8px;
    max-height: 180px;
    overflow-y: auto;
    padding-right: 4px;
  }
  .profile-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 8px 12px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-color);
  }
  .profile-item.active {
    border-color: rgba(245, 158, 11, 0.25);
    background: var(--bg-active);
  }
  .profile-details {
    display: flex;
    flex-direction: column;
  }
  .profile-title {
    font-weight: 600;
    font-size: 0.85rem;
    color: var(--text-primary);
  }
  .profile-subtitle {
    font-size: 0.7rem;
    color: var(--text-muted);
  }
  .profile-actions {
    display: flex;
    gap: 6px;
  }
  .dep-status-section {
    padding: 14px;
    border-radius: var(--radius-md);
    border: 1px solid var(--border-color);
  }
  .desc-text {
    font-size: 0.76rem;
    color: var(--text-secondary);
    line-height: 1.4;
  }
  .dep-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .dep-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-size: 0.8rem;
    color: var(--text-secondary);
  }
  .success-badge {
    background: rgba(34, 197, 94, 0.15) !important;
    color: var(--accent-green) !important;
    border: 1px solid rgba(34, 197, 94, 0.25);
  }
  .danger-badge {
    background: rgba(239, 68, 68, 0.15) !important;
    color: var(--accent-red) !important;
    border: 1px solid rgba(239, 68, 68, 0.25);
  }
  .success-message {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    font-size: 0.78rem;
    color: var(--accent-green);
    background: rgba(34, 197, 94, 0.08);
    padding: 8px;
    border-radius: var(--radius-sm);
    border: 1px solid rgba(34, 197, 94, 0.15);
  }
  .profile-form-section {
    border-radius: var(--radius-md);
    border: 1px solid var(--border-color);
  }
  .mt-xs { margin-top: 4px; }
  .mt-sm { margin-top: 8px; }
  .mt-md { margin-top: 16px; }
  .p-md { padding: 16px; }
  .flex-col { display: flex; flex-direction: column; }
  .flex-align { display: flex; align-items: center; }
  .flex-center { display: flex; align-items: center; justify-content: center; }
  .gap-xs { gap: 4px; }
  .gap-sm { gap: 8px; }
  .gap-md { gap: 16px; }
  .w-full { width: 100%; }
</style>
