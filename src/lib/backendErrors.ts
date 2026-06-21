export type AppErrorPayload = {
	code: string;
	details?: string;
};

const SUDO_CODES = new Set(['SUDO_PASSWORD_REQUIRED', 'SUDO_PASSWORD_INCORRECT']);
const TRANSFER_CANCELLED = 'TRANSFER_CANCELLED';

const ERROR_MAP: Record<string, string> = {
	NO_SSH_CONNECTION: 'No active SSH connection',
	PROFILE_NOT_FOUND: 'Profile not found',
	APP_CONFIG_DIR_FAILED: 'Cannot resolve config directory: {details}',
	CONFIG_DIR_CREATE_FAILED: 'Cannot create config directory: {details}',
	PROFILES_READ_FAILED: 'Failed to read profiles: {details}',
	PROFILES_WRITE_FAILED: 'Failed to write profiles file: {details}',
	KEYRING_INIT_FAILED: 'Keyring initialization failed: {details}',
	KEYRING_PASSWORD_SAVE_FAILED: 'Failed to save password in Keyring: {details}',
	KEYRING_PASSPHRASE_SAVE_FAILED: 'Failed to save key passphrase in Keyring: {details}',
	JSON_SERIALIZE_FAILED: 'Data serialization failed: {details}',
	SUDO_PASSWORD_REQUIRED: 'Sudo password required',
	SUDO_PASSWORD_INCORRECT: 'Incorrect sudo password',
	SUDO_VERIFICATION_FAILED: 'Sudo verification failed: {details}',
	REMOTE_COMMAND_FAILED: 'Remote command failed: {details}',
	INVALID_SHELL: 'Invalid shell',
	INVALID_CONTAINER_ID: 'Invalid container ID',
	TERMINAL_NOT_RUNNING: 'Terminal is not running',
	TERMINAL_SEND_FAILED: 'Failed to send data to terminal: {details}',
	EXTERNAL_TERMINAL_WINDOWS_ONLY: 'External terminal is currently supported on Windows only',
	EXTERNAL_TERMINAL_OPEN_FAILED: 'Failed to open external terminal: {details}',
	TRANSFER_ALREADY_RUNNING: 'A transfer is already in progress. Wait or cancel the current operation.',
	TRANSFER_CANCELLED: 'Cancelled',
	TRANSFER_IN_PROGRESS: 'Transfer in progress',
	DOWNLOAD_DIR_CREATE_FAILED: 'Cannot create download directory: {details}',
	HOME_DIR_READ_FAILED: 'Failed to read home directory: {details}',
	DNS_RESOLUTION_FAILED: 'DNS resolution failed: {details}',
	HOST_NOT_FOUND: 'No IP address found for host: {details}',
	SSH_CONNECTION_FAILED: 'SSH connection failed: {details}',
	SSH_PRIVATE_KEY_LOAD_FAILED: 'Failed to load private key: {details}',
	SSH_RSA_HASH_NEGOTIATION_FAILED: 'RSA hash negotiation failed: {details}',
	SSH_PUBLIC_KEY_AUTH_FAILED: 'Public key authentication failed: {details}',
	SSH_PASSWORD_AUTH_FAILED: 'Password authentication failed: {details}',
	SSH_NO_CREDENTIALS: 'No credentials for authorization (provide password or private key)',
	SSH_AUTH_FAILED: 'SSH authorization failed',
	SSH_CHANNEL_OPEN_FAILED: 'Failed to open SSH channel: {details}',
	SSH_COMMAND_EXEC_FAILED: 'Failed to execute command: {details}',
	SSH_PTY_FAILED: 'PTY error: {details}',
	SSH_SHELL_FAILED: 'Shell error: {details}',
	CONTAINER_SHELL_START_FAILED: 'Failed to start container shell: {details}',
	SFTP_CHANNEL_OPEN_FAILED: 'Failed to open SFTP channel: {details}',
	SFTP_SUBSYSTEM_FAILED: 'SFTP subsystem error: {details}',
	SFTP_INIT_FAILED: 'SFTP initialization failed: {details}',
	SFTP_DIR_READ_FAILED: 'Failed to read directory: {details}',
	DU_COMMAND_FAILED: 'du command failed: {details}',
	DU_EMPTY_OUTPUT: 'Empty du output',
	DU_INVALID_FORMAT: 'Invalid du format: {details}',
	DU_PARSE_FAILED: 'Cannot parse size: {details}',
	FIND_COMMAND_FAILED: 'find command failed: {details}',
	SFTP_FILE_OPEN_FAILED: 'Cannot open file: {details}',
	SFTP_FILE_READ_FAILED: 'File read error: {details}',
	SFTP_FILE_CREATE_FAILED: 'Cannot create file: {details}',
	SFTP_FILE_WRITE_FAILED: 'File write error: {details}',
	SFTP_FILE_CLOSE_FAILED: 'File close error: {details}',
	SFTP_DIR_CREATE_FAILED: 'Cannot create directory: {details}',
	SFTP_DIR_DELETE_FAILED: 'Cannot delete directory: {details}',
	SFTP_FILE_DELETE_FAILED: 'Cannot delete file: {details}',
	SFTP_RENAME_FAILED: 'Rename/move error: {details}',
	SFTP_METADATA_FAILED: 'Metadata error: {details}',
	LOCAL_FILE_READ_FAILED: 'Local file read error: {details}',
	LOCAL_FILE_OPEN_FAILED: 'Cannot open local file: {details}',
	LOCAL_FILE_CREATE_FAILED: 'Cannot create local file: {details}',
	LOCAL_DIR_CREATE_FAILED: 'Cannot create local directory: {details}',
	REMOTE_FILE_CREATE_FAILED: 'Cannot create remote file: {details}',
	LOCAL_READ_FAILED: 'Local read error: {details}',
	REMOTE_WRITE_FAILED: 'Remote write error: {details}',
	REMOTE_FILE_CLOSE_FAILED: 'Remote file close error: {details}',
	TRANSFER_VERIFY_FAILED: 'Transfer verification failed: {details}',
	UPLOAD_FINALIZE_FAILED: 'Upload finalize error: {details}',
	REMOTE_FILE_OPEN_FAILED: 'Cannot open remote file: {details}',
	REMOTE_READ_FAILED: 'Remote read error: {details}',
	LOCAL_WRITE_FAILED: 'Local write error: {details}',
	LOCAL_FLUSH_FAILED: 'Local flush error: {details}',
	DOWNLOAD_FINALIZE_FAILED: 'Download finalize error: {details}',
	SFTP_MOVE_FAILED: 'Move error: {details}',
	SFTP_COPY_FAILED: 'Copy error: {details}',
	SFTP_COPY_VERIFY_FAILED: 'Copy verification failed: {details}',
	LOCAL_PATH_NOT_FOUND: 'Path does not exist: {details}',
	INVALID_LOCAL_PATH_NAME: 'Invalid name: {details}',
	LOCAL_DIR_READ_FAILED: 'Local directory read error: {details}',
	NO_LOCAL_PATH: 'No local path',
	NO_DEST_PATH: 'No destination path',
	DOCKER_LOGS_START_FAILED: 'Failed to start docker logs: {details}',
	COMPOSE_PULL_START_FAILED: 'Failed to start compose pull: {details}',
	PANGOLIN_CONFIG_DIR_FAILED: 'Cannot create Pangolin config directory: {details}',
	PANGOLIN_CONFIG_READ_FAILED: 'Failed to read Pangolin config: {details}',
	PANGOLIN_CONFIG_WRITE_FAILED: 'Failed to write Pangolin config: {details}',
	PANGOLIN_HEALTH_CHECK_FAILED: 'No response from Pangolin server at: {details}',
	PANGOLIN_HTTP_CLIENT_FAILED: 'Cannot create HTTP client: {details}',
	PANGOLIN_KEYRING_INIT_FAILED: 'Pangolin Keyring initialization failed: {details}',
	PANGOLIN_KEYRING_SAVE_FAILED: 'Failed to save Pangolin API key: {details}',
	PANGOLIN_API_KEY_NOT_CONFIGURED: 'Pangolin API key is not configured. Go to settings.',
	PANGOLIN_UNSUPPORTED_METHOD: 'Unsupported HTTP method: {details}',
	PANGOLIN_HTTP_REQUEST_FAILED: 'HTTP request failed: {details}',
	PANGOLIN_RESPONSE_READ_FAILED: 'Cannot read HTTP response: {details}',
	PANGOLIN_API_ERROR: 'Pangolin API error: {details}',
	PANGOLIN_HTTP_ERROR: 'HTTP error: {details}',
	PANGOLIN_JSON_PARSE_FAILED: 'Cannot parse Pangolin response: {details}',
	PROFILE_EXTRAS_CONFIG_DIR_FAILED: 'Cannot create config directory: {details}',
	PROFILE_EXTRAS_READ_FAILED: 'Failed to read profile extras: {details}',
	PROFILE_EXTRAS_WRITE_FAILED: 'Failed to write profile extras: {details}',
};

export function isSudoPasswordRequired(err: unknown): boolean {
	return extractErrorCode(err) === 'SUDO_PASSWORD_REQUIRED';
}

export function isSudoPasswordIncorrect(err: unknown): boolean {
	return extractErrorCode(err) === 'SUDO_PASSWORD_INCORRECT';
}

export function isTransferCancelled(err: unknown): boolean {
	const code = extractErrorCode(err);
	return code === TRANSFER_CANCELLED || String(err) === 'Anulowano';
}

export function parseAppError(err: unknown): AppErrorPayload | null {
	if (typeof err === 'object' && err !== null && 'code' in err) {
		const payload = err as AppErrorPayload;
		if (typeof payload.code === 'string') return payload;
	}
	const text = String(err);
	try {
		const parsed = JSON.parse(text) as AppErrorPayload;
		if (parsed?.code) return parsed;
	} catch {
		// legacy plain string
	}
	if (SUDO_CODES.has(text)) return { code: text };
	if (text === 'Anulowano') return { code: TRANSFER_CANCELLED };
	return null;
}

function extractErrorCode(err: unknown): string | null {
	return parseAppError(err)?.code ?? null;
}

function translateCode(code: string, details?: string): string | null {
	const pattern = ERROR_MAP[code];
	if (!pattern) return null;
	if (details !== undefined) {
		return pattern.replace('{details}', details);
	}
	return pattern.replace(': {details}', '').replace('{details}', '');
}

export function formatInvokeError(err: unknown): string {
	const parsed = parseAppError(err);
	if (parsed?.code) {
		const translated = translateCode(parsed.code, parsed.details);
		if (translated) return translated;
		if (parsed.details) return `${parsed.code}: ${parsed.details}`;
		return parsed.code;
	}
	return String(err);
}

export function formatBackendError(code: string, details?: string): string {
	return translateCode(code, details) ?? details ?? code;
}
