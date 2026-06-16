/**
 * One-shot generator for en/pl locale files. Run: node scripts/build-i18n-locales.mjs
 * Output is written to src/lib/i18n/en/index.ts and src/lib/i18n/pl/index.ts
 */
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const root = path.join(__dirname, '..')

/** @type {Record<string, Record<string, string>>} */
const errors = {
	NO_SSH_CONNECTION: { en: 'No active SSH connection', pl: 'Brak aktywnego połączenia SSH' },
	PROFILE_NOT_FOUND: { en: 'Profile not found', pl: 'Profil nie istnieje' },
	APP_CONFIG_DIR_FAILED: { en: 'Cannot resolve config directory: {details}', pl: 'Nie można ustalić katalogu konfiguracji: {details}' },
	CONFIG_DIR_CREATE_FAILED: { en: 'Cannot create config directory: {details}', pl: 'Nie można utworzyć katalogu konfiguracji: {details}' },
	PROFILES_READ_FAILED: { en: 'Failed to read profiles: {details}', pl: 'Błąd odczytu profili: {details}' },
	PROFILES_WRITE_FAILED: { en: 'Failed to write profiles file: {details}', pl: 'Błąd zapisu pliku profili: {details}' },
	KEYRING_INIT_FAILED: { en: 'Keyring initialization failed: {details}', pl: 'Błąd inicjalizacji Keyringa: {details}' },
	KEYRING_PASSWORD_SAVE_FAILED: { en: 'Failed to save password in Keyring: {details}', pl: 'Błąd zapisu hasła w Keyringu: {details}' },
	KEYRING_PASSPHRASE_SAVE_FAILED: { en: 'Failed to save key passphrase in Keyring: {details}', pl: 'Błąd zapisu hasła klucza w Keyringu: {details}' },
	JSON_SERIALIZE_FAILED: { en: 'Data serialization failed: {details}', pl: 'Błąd serializacji danych: {details}' },
	SUDO_PASSWORD_REQUIRED: { en: 'Sudo password required', pl: 'Wymagane hasło sudo' },
	SUDO_PASSWORD_INCORRECT: { en: 'Incorrect sudo password', pl: 'Niepoprawne hasło sudo' },
	SUDO_VERIFICATION_FAILED: { en: 'Sudo verification failed: {details}', pl: 'Błąd weryfikacji sudo: {details}' },
	REMOTE_COMMAND_FAILED: { en: 'Remote command failed: {details}', pl: 'Błąd wykonania polecenia: {details}' },
	INVALID_SHELL: { en: 'Invalid shell', pl: 'Nieprawidłowa powłoka' },
	INVALID_CONTAINER_ID: { en: 'Invalid container ID', pl: 'Nieprawidłowy identyfikator kontenera' },
	TERMINAL_NOT_RUNNING: { en: 'Terminal is not running', pl: 'Terminal nie jest uruchomiony' },
	TERMINAL_SEND_FAILED: { en: 'Failed to send data to terminal: {details}', pl: 'Nie można wysłać danych do terminala: {details}' },
	EXTERNAL_TERMINAL_WINDOWS_ONLY: { en: 'External terminal is currently supported on Windows only', pl: 'Zewnętrzny terminal jest wspierany obecnie na systemie Windows' },
	EXTERNAL_TERMINAL_OPEN_FAILED: { en: 'Failed to open external terminal: {details}', pl: 'Nie można otworzyć terminala CMD: {details}' },
	TRANSFER_ALREADY_RUNNING: { en: 'A transfer is already in progress. Wait or cancel the current operation.', pl: 'Transfer już trwa. Poczekaj lub anuluj bieżącą operację.' },
	TRANSFER_CANCELLED: { en: 'Cancelled', pl: 'Anulowano' },
	TRANSFER_IN_PROGRESS: { en: 'Transfer in progress', pl: 'Transfer w toku' },
	DOWNLOAD_DIR_CREATE_FAILED: { en: 'Cannot create download directory: {details}', pl: 'Nie można utworzyć katalogu docelowego: {details}' },
	HOME_DIR_READ_FAILED: { en: 'Failed to read home directory: {details}', pl: 'Błąd pobierania katalogu domowego: {details}' },
	DNS_RESOLUTION_FAILED: { en: 'DNS resolution failed: {details}', pl: 'Błąd DNS/Rozwiązywania hosta: {details}' },
	HOST_NOT_FOUND: { en: 'No IP address found for host: {details}', pl: 'Nie znaleziono adresu IP dla hosta: {details}' },
	SSH_CONNECTION_FAILED: { en: 'SSH connection failed: {details}', pl: 'Błąd połączenia SSH: {details}' },
	SSH_PRIVATE_KEY_LOAD_FAILED: { en: 'Failed to load private key: {details}', pl: 'Błąd ładowania klucza prywatnego: {details}' },
	SSH_RSA_HASH_NEGOTIATION_FAILED: { en: 'RSA hash negotiation failed: {details}', pl: 'Błąd uzgadniania hasha RSA: {details}' },
	SSH_PUBLIC_KEY_AUTH_FAILED: { en: 'Public key authentication failed: {details}', pl: 'Błąd autoryzacji kluczem publicznym: {details}' },
	SSH_PASSWORD_AUTH_FAILED: { en: 'Password authentication failed: {details}', pl: 'Błąd autoryzacji hasłem: {details}' },
	SSH_NO_CREDENTIALS: { en: 'No credentials for authorization (provide password or private key)', pl: 'Brak poświadczeń do autoryzacji (podaj hasło lub klucz prywatny)' },
	SSH_AUTH_FAILED: { en: 'SSH authorization failed', pl: 'Autoryzacja SSH nie powiodła się' },
	SSH_CHANNEL_OPEN_FAILED: { en: 'Failed to open SSH channel: {details}', pl: 'Błąd otwarcia kanału: {details}' },
	SSH_COMMAND_EXEC_FAILED: { en: 'Failed to execute command: {details}', pl: 'Błąd uruchomienia komendy: {details}' },
	SSH_PTY_FAILED: { en: 'PTY error: {details}', pl: 'Błąd PTY: {details}' },
	SSH_SHELL_FAILED: { en: 'Shell error: {details}', pl: 'Błąd Shell: {details}' },
	CONTAINER_SHELL_START_FAILED: { en: 'Failed to start container shell: {details}', pl: 'Błąd uruchomienia shella kontenera: {details}' },
	SFTP_CHANNEL_OPEN_FAILED: { en: 'Failed to open SFTP channel: {details}', pl: 'Błąd otwarcia kanału SFTP: {details}' },
	SFTP_SUBSYSTEM_FAILED: { en: 'SFTP subsystem error: {details}', pl: 'Błąd podsystemu SFTP: {details}' },
	SFTP_INIT_FAILED: { en: 'SFTP initialization failed: {details}', pl: 'Błąd inicjalizacji SFTP: {details}' },
	SFTP_DIR_READ_FAILED: { en: 'Failed to read directory: {details}', pl: 'Błąd odczytu katalogu: {details}' },
	DU_COMMAND_FAILED: { en: 'du command failed: {details}', pl: 'Polecenie du nie powiodło się: {details}' },
	DU_EMPTY_OUTPUT: { en: 'Empty du output', pl: 'Pusty wynik du' },
	DU_INVALID_FORMAT: { en: 'Invalid du format: {details}', pl: 'Nieprawidłowy format du: {details}' },
	DU_PARSE_FAILED: { en: 'Cannot parse size: {details}', pl: 'Nie można sparsować rozmiaru: {details}' },
	FIND_COMMAND_FAILED: { en: 'find command failed: {details}', pl: 'Polecenie find nie powiodło się: {details}' },
	SFTP_FILE_OPEN_FAILED: { en: 'Cannot open file: {details}', pl: 'Nie można otworzyć pliku: {details}' },
	SFTP_FILE_READ_FAILED: { en: 'File read error: {details}', pl: 'Błąd odczytu zawartości pliku: {details}' },
	SFTP_FILE_CREATE_FAILED: { en: 'Cannot create file: {details}', pl: 'Nie można utworzyć pliku: {details}' },
	SFTP_FILE_WRITE_FAILED: { en: 'File write error: {details}', pl: 'Błąd zapisu do pliku: {details}' },
	SFTP_FILE_CLOSE_FAILED: { en: 'File close error: {details}', pl: 'Błąd zamykania pliku: {details}' },
	SFTP_DIR_CREATE_FAILED: { en: 'Cannot create directory: {details}', pl: 'Nie można utworzyć katalogu: {details}' },
	SFTP_DIR_DELETE_FAILED: { en: 'Cannot delete directory: {details}', pl: 'Nie można usunąć katalogu: {details}' },
	SFTP_FILE_DELETE_FAILED: { en: 'Cannot delete file: {details}', pl: 'Nie można usunąć pliku: {details}' },
	SFTP_RENAME_FAILED: { en: 'Rename/move error: {details}', pl: 'Błąd zmiany nazwy/przeniesienia: {details}' },
	SFTP_METADATA_FAILED: { en: 'Metadata error: {details}', pl: 'Błąd metadata: {details}' },
	LOCAL_FILE_READ_FAILED: { en: 'Local file read error: {details}', pl: 'Błąd odczytu lokalnego pliku: {details}' },
	LOCAL_FILE_OPEN_FAILED: { en: 'Cannot open local file: {details}', pl: 'Nie można otworzyć pliku lokalnego: {details}' },
	LOCAL_FILE_CREATE_FAILED: { en: 'Cannot create local file: {details}', pl: 'Nie można utworzyć pliku lokalnego: {details}' },
	LOCAL_DIR_CREATE_FAILED: { en: 'Cannot create local directory: {details}', pl: 'Nie można utworzyć katalogu lokalnego: {details}' },
	REMOTE_FILE_CREATE_FAILED: { en: 'Cannot create remote file: {details}', pl: 'Nie można utworzyć zdalnego pliku: {details}' },
	LOCAL_READ_FAILED: { en: 'Local read error: {details}', pl: 'Błąd odczytu lokalnego: {details}' },
	REMOTE_WRITE_FAILED: { en: 'Remote write error: {details}', pl: 'Błąd zapisu zdalnego: {details}' },
	REMOTE_FILE_CLOSE_FAILED: { en: 'Remote file close error: {details}', pl: 'Błąd zamykania zdalnego pliku: {details}' },
	TRANSFER_VERIFY_FAILED: { en: 'Transfer verification failed: {details}', pl: 'Weryfikacja transferu nie powiodła się: {details}' },
	UPLOAD_FINALIZE_FAILED: { en: 'Upload finalize error: {details}', pl: 'Błąd finalizacji uploadu: {details}' },
	REMOTE_FILE_OPEN_FAILED: { en: 'Cannot open remote file: {details}', pl: 'Nie można otworzyć zdalnego pliku: {details}' },
	REMOTE_READ_FAILED: { en: 'Remote read error: {details}', pl: 'Błąd odczytu zdalnego: {details}' },
	LOCAL_WRITE_FAILED: { en: 'Local write error: {details}', pl: 'Błąd zapisu lokalnego: {details}' },
	LOCAL_FLUSH_FAILED: { en: 'Local flush error: {details}', pl: 'Błąd flush lokalnego: {details}' },
	DOWNLOAD_FINALIZE_FAILED: { en: 'Download finalize error: {details}', pl: 'Błąd finalizacji pobierania: {details}' },
	SFTP_MOVE_FAILED: { en: 'Move error: {details}', pl: 'Błąd przeniesienia: {details}' },
	SFTP_COPY_FAILED: { en: 'Copy error: {details}', pl: 'Błąd kopiowania: {details}' },
	SFTP_COPY_VERIFY_FAILED: { en: 'Copy verification failed: {details}', pl: 'Kopiowanie nieudane: {details}' },
	LOCAL_PATH_NOT_FOUND: { en: 'Path does not exist: {details}', pl: 'Ścieżka nie istnieje: {details}' },
	INVALID_LOCAL_PATH_NAME: { en: 'Invalid name: {details}', pl: 'Nieprawidłowa nazwa: {details}' },
	LOCAL_DIR_READ_FAILED: { en: 'Local directory read error: {details}', pl: 'Błąd odczytu katalogu lokalnego: {details}' },
	NO_LOCAL_PATH: { en: 'No local path', pl: 'Brak ścieżki lokalnej' },
	NO_DEST_PATH: { en: 'No destination path', pl: 'Brak ścieżki docelowej' },
	DOCKER_LOGS_START_FAILED: { en: 'Failed to start docker logs: {details}', pl: 'Błąd uruchomienia docker logs: {details}' },
	COMPOSE_PULL_START_FAILED: { en: 'Failed to start compose pull: {details}', pl: 'Błąd uruchomienia compose pull: {details}' },
	PANGOLIN_CONFIG_DIR_FAILED: { en: 'Cannot create Pangolin config directory: {details}', pl: 'Nie można utworzyć katalogu konfiguracji Pangolin: {details}' },
	PANGOLIN_CONFIG_READ_FAILED: { en: 'Failed to read Pangolin config: {details}', pl: 'Błąd odczytu konfiguracji Pangolin: {details}' },
	PANGOLIN_CONFIG_WRITE_FAILED: { en: 'Failed to write Pangolin config: {details}', pl: 'Błąd zapisu konfiguracji Pangolin: {details}' },
	PANGOLIN_HEALTH_CHECK_FAILED: { en: 'No response from Pangolin server at: {details}', pl: 'Brak odpowiedzi z serwera Pangolin pod adresem: {details}' },
	PANGOLIN_HTTP_CLIENT_FAILED: { en: 'Cannot create HTTP client: {details}', pl: 'Nie można utworzyć klienta HTTP: {details}' },
	PANGOLIN_KEYRING_INIT_FAILED: { en: 'Pangolin Keyring initialization failed: {details}', pl: 'Błąd inicjalizacji Keyringa Pangolin: {details}' },
	PANGOLIN_KEYRING_SAVE_FAILED: { en: 'Failed to save Pangolin API key: {details}', pl: 'Błąd zapisu klucza API Pangolin: {details}' },
	PANGOLIN_API_KEY_NOT_CONFIGURED: { en: 'Pangolin API key is not configured. Go to settings.', pl: 'Klucz API Pangolin nie jest skonfigurowany. Przejdź do ustawień.' },
	PANGOLIN_UNSUPPORTED_METHOD: { en: 'Unsupported HTTP method: {details}', pl: 'Nieobsługiwana metoda HTTP: {details}' },
	PANGOLIN_HTTP_REQUEST_FAILED: { en: 'HTTP request failed: {details}', pl: 'Żądanie HTTP nie powiodło się: {details}' },
	PANGOLIN_RESPONSE_READ_FAILED: { en: 'Cannot read HTTP response: {details}', pl: 'Nie można odczytać odpowiedzi HTTP: {details}' },
	PANGOLIN_API_ERROR: { en: 'Pangolin API error: {details}', pl: 'Błąd API Pangolin: {details}' },
	PANGOLIN_HTTP_ERROR: { en: 'HTTP error: {details}', pl: 'Błąd HTTP: {details}' },
	PANGOLIN_JSON_PARSE_FAILED: { en: 'Cannot parse Pangolin response: {details}', pl: 'Nie można sparsować odpowiedzi Pangolin: {details}' },
	PROFILE_EXTRAS_CONFIG_DIR_FAILED: { en: 'Cannot create config directory: {details}', pl: 'Nie można utworzyć katalogu konfiguracji: {details}' },
	PROFILE_EXTRAS_READ_FAILED: { en: 'Failed to read profile extras: {details}', pl: 'Błąd odczytu dodatków profilu: {details}' },
	PROFILE_EXTRAS_WRITE_FAILED: { en: 'Failed to write profile extras: {details}', pl: 'Błąd zapisu dodatków profilu: {details}' },
}

function tsString(s) {
	return `'${String(s)
		.replace(/\\/g, '\\\\')
		.replace(/'/g, "\\'")
		.replace(/\r/g, '\\r')
		.replace(/\n/g, '\\n')
		.replace(/\t/g, '\\t')}'`
}

function isLeafTranslation(val) {
	return (
		typeof val === 'object' &&
		val !== null &&
		typeof val.en === 'string' &&
		typeof val.pl === 'string' &&
		Object.keys(val).every((k) => k === 'en' || k === 'pl')
	)
}

function renderObject(obj, locale, indent = 1) {
	const pad = '\t'.repeat(indent)
	const lines = []
	for (const [key, val] of Object.entries(obj)) {
		if (isLeafTranslation(val)) {
			lines.push(`${pad}${/^[a-zA-Z_$][\w$]*$/.test(key) ? key : `'${key}'`}: ${tsString(val[locale])},`)
		} else if (typeof val === 'object' && val !== null) {
			lines.push(`${pad}${/^[a-zA-Z_$][\w$]*$/.test(key) ? key : `'${key}'`}: {`)
			lines.push(renderObject(val, locale, indent + 1))
			lines.push(`${pad}},`)
		}
	}
	return lines.join('\n')
}

// Import the rest from a JSON data file we'll inline below
const dataPath = path.join(__dirname, 'i18n-locale-data.json')
if (!fs.existsSync(dataPath)) {
	console.error('Missing', dataPath)
	process.exit(1)
}
const data = JSON.parse(fs.readFileSync(dataPath, 'utf8'))

function buildLocale(locale, satisfies, importLine) {
	const errorsBlock = Object.fromEntries(
		Object.entries(errors).map(([k, v]) => [k, v]),
	)
	const full = { ...data, errors: errorsBlock }
	const body = renderObject(full, locale)
	return `${importLine}\n\nconst ${locale} = {\n${body}\n} satisfies ${satisfies}\n\nexport default ${locale}\n`
}

const enPath = path.join(root, 'src/lib/i18n/en/index.ts')
const plPath = path.join(root, 'src/lib/i18n/pl/index.ts')

fs.writeFileSync(
	enPath,
	buildLocale('en', 'BaseTranslation', "import type { BaseTranslation } from '../i18n-types'"),
)
fs.writeFileSync(
	plPath,
	buildLocale('pl', 'Translation', "import type { Translation } from '../i18n-types'"),
)
console.log('Wrote', enPath, 'and', plPath)
