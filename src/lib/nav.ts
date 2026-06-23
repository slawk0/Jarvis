export const TAB_IDS = [
	'dashboard',
	'maintenance',
	'backups',
	'restic',
	'network',
	'runbooks',
	'files',
	'services',
	'docker',
	'cron',
	'users',
	'firewall',
	'crowdsec',
	'pangolin',
	'logs',
	'loganalysis',
	'terminal',
	'disks',
	'webserver',
	'processes',
	'database',
	'envvars',
	'netdiag',
	'timers',
] as const;

export type TabId = (typeof TAB_IDS)[number];

export const NAV_LABELS: Record<TabId, string> = {
	dashboard: 'Dashboard',
	maintenance: 'Maintenance',
	backups: 'Backups',
	restic: 'Restic Backups',
	network: 'Network / Ports',
	runbooks: 'Runbooks',
	files: 'Files (SFTP)',
	services: 'Services (Systemd)',
	docker: 'Docker',
	cron: 'Tasks (Cron)',
	users: 'Users',
	firewall: 'Firewall',
	crowdsec: 'CrowdSec',
	pangolin: 'Pangolin Proxy',
	logs: 'Logs',
	loganalysis: 'Log Analysis',
	terminal: 'Terminal',
	disks: 'Disk Management',
	webserver: 'Nginx Manager',
	processes: 'Processes',
	database: 'Databases',
	envvars: 'Env Variables',
	netdiag: 'Net Diagnostics',
	timers: 'Systemd Timers',
};

export function getNavLabel(tabId: string): string {
	return NAV_LABELS[tabId as TabId] || tabId;
}

export function getNavLabels(): Record<string, string> {
	return NAV_LABELS;
}
