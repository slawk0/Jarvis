export const TAB_IDS = [
	// Overview
	'dashboard',
	'terminal',
	'runbooks',

	// System
	'services',
	'docker',
	'processes',
	'timers',
	'cron',
	'disks',
	'maintenance',
	'users',
	'envvars',

	// Network & Web
	'webserver',
	'pangolin',
	'network',
	'netdiag',

	// Security
	'firewall',
	'crowdsec',

	// Data & Storage
	'files',
	'database',
	'backups',
	'restic',

	// Monitoring
	'logs',
	'loganalysis',
] as const;

export type TabId = (typeof TAB_IDS)[number];

export const NAV_LABELS: Record<TabId, string> = {
	dashboard: 'Dashboard',
	terminal: 'Terminal',
	runbooks: 'Runbooks',
	services: 'Services (Systemd)',
	docker: 'Docker',
	processes: 'Processes',
	timers: 'Systemd Timers',
	cron: 'Tasks (Cron)',
	disks: 'Disk Management',
	maintenance: 'Maintenance',
	users: 'Users',
	envvars: 'Env Variables',
	webserver: 'Nginx Manager',
	pangolin: 'Pangolin Proxy',
	network: 'Network / Ports',
	netdiag: 'Net Diagnostics',
	firewall: 'Firewall',
	crowdsec: 'CrowdSec',
	files: 'Files (SFTP)',
	database: 'Databases',
	backups: 'Backups',
	restic: 'Restic Backups',
	logs: 'Logs',
	loganalysis: 'Log Analysis',
};

export interface NavCategory {
	id: string;
	label: string;
	items: readonly TabId[];
}

export const NAV_CATEGORIES: readonly NavCategory[] = [
	{
		id: 'overview',
		label: 'Overview',
		items: ['dashboard', 'terminal', 'runbooks'] as const,
	},
	{
		id: 'system',
		label: 'System',
		items: ['services', 'docker', 'processes', 'timers', 'cron', 'disks', 'maintenance', 'users', 'envvars'] as const,
	},
	{
		id: 'network',
		label: 'Network & Web',
		items: ['webserver', 'pangolin', 'network', 'netdiag'] as const,
	},
	{
		id: 'security',
		label: 'Security',
		items: ['firewall', 'crowdsec'] as const,
	},
	{
		id: 'data',
		label: 'Data & Storage',
		items: ['files', 'database', 'backups', 'restic'] as const,
	},
	{
		id: 'monitoring',
		label: 'Monitoring',
		items: ['logs', 'loganalysis'] as const,
	},
] as const;

export function getNavLabel(tabId: string): string {
	return NAV_LABELS[tabId as TabId] || tabId;
}

export function getNavLabels(): Record<string, string> {
	return NAV_LABELS;
}

