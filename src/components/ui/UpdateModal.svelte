<script lang="ts">
  import { onMount } from 'svelte';
  import { check } from '@tauri-apps/plugin-updater';
  import { relaunch } from '@tauri-apps/plugin-process';
  import { notifications } from '$lib/notifications.svelte';
  import { Download, RefreshCw, X, AlertCircle } from 'lucide-svelte';

  type UpdateState = 'idle' | 'checking' | 'available' | 'downloading' | 'installing' | 'relaunching' | 'error';

  let status = $state<UpdateState>('idle');
  let newVersion = $state<string>('');
  let updateNotes = $state<string>('');
  let downloadProgress = $state<number>(0);
  let errorMessage = $state<string>('');
  let showModal = $state<boolean>(false);
  let tauriUpdateObj = $state<any>(null);

  async function checkUpdate(silent = true) {
    try {
      if (!silent) {
        status = 'checking';
        showModal = true;
      }
      
      const update = await check();
      
      if (update && update.available) {
        tauriUpdateObj = update;
        newVersion = update.version;
        updateNotes = update.body || 'No release notes provided.';
        status = 'available';
        showModal = true;
      } else {
        if (!silent) {
          status = 'idle';
          showModal = false;
          notifications.success('Application is up to date.');
        }
      }
    } catch (err: any) {
      console.error('Update check failed:', err);
      if (!silent) {
        status = 'error';
        errorMessage = err.toString() || 'Failed to check for updates.';
        showModal = true;
      }
    }
  }

  async function startUpdate() {
    if (!tauriUpdateObj) return;

    try {
      status = 'downloading';
      downloadProgress = 0;
      let downloadedBytes = 0;
      let totalBytes = 0;

      await tauriUpdateObj.downloadAndInstall((event: any) => {
        switch (event.event) {
          case 'Started':
            totalBytes = event.data.contentLength || 0;
            status = 'downloading';
            break;
          case 'Progress':
            downloadedBytes += event.data.chunkLength;
            if (totalBytes > 0) {
              downloadProgress = Math.round((downloadedBytes / totalBytes) * 100);
            }
            break;
          case 'Finished':
            status = 'installing';
            break;
        }
      });

      status = 'relaunching';
      notifications.success('Update installed! Relaunching application...');
      
      // Delay slightly so user reads the status before relaunch
      setTimeout(async () => {
        await relaunch();
      }, 1500);

    } catch (err: any) {
      console.error('Download and install failed:', err);
      status = 'error';
      errorMessage = err.toString() || 'Failed to download or install the update.';
    }
  }

  function closeModal() {
    if (status !== 'downloading' && status !== 'installing' && status !== 'relaunching') {
      showModal = false;
      status = 'idle';
    }
  }

  onMount(() => {
    // Check silently on startup after a small delay
    setTimeout(() => {
      checkUpdate(true);
    }, 3000);
  });
</script>

{#if showModal}
  <div class="modal-overlay" onclick={closeModal} role="presentation">
    <div class="modal-content glass update-modal" onclick={(e) => e.stopPropagation()} role="dialog">
      
      <div class="modal-header">
        <div class="modal-header-left">
          <RefreshCw class="header-icon {status === 'checking' || status === 'downloading' ? 'spin' : ''}" size={20} />
          <h3>
            {#if status === 'checking'}
              Checking for Updates...
            {:else if status === 'available'}
              New Version Available
            {:else if status === 'downloading'}
              Downloading Update
            {:else if status === 'installing'}
              Installing Update
            {:else if status === 'relaunching'}
              Relaunching Application
            {:else if status === 'error'}
              Update Error
            {/if}
          </h3>
        </div>
        {#if status !== 'downloading' && status !== 'installing' && status !== 'relaunching'}
          <button class="close-btn" onclick={closeModal} aria-label="Close modal">
            <X size={18} />
          </button>
        {/if}
      </div>

      <div class="modal-body">
        {#if status === 'checking'}
          <div class="loading-state">
            <RefreshCw class="spin loader" size={32} />
            <p>Checking GitHub for newer versions...</p>
          </div>
        {:else if status === 'available'}
          <div class="update-info">
            <div class="version-badge">Version {newVersion}</div>
            <p class="section-title">What's New:</p>
            <div class="notes-container">
              {@html updateNotes.replace(/\n/g, '<br />')}
            </div>
          </div>
        {:else if status === 'downloading'}
          <div class="progress-container">
            <p>Downloading update files...</p>
            <div class="progress-bar-bg">
              <div class="progress-bar-fill" style="width: {downloadProgress}%"></div>
            </div>
            <div class="progress-text">
              <span>{downloadProgress}%</span>
            </div>
          </div>
        {:else if status === 'installing'}
          <div class="loading-state">
            <RefreshCw class="spin loader" size={32} />
            <p>Installing update. Please do not close the app...</p>
          </div>
        {:else if status === 'relaunching'}
          <div class="loading-state">
            <RefreshCw class="spin loader" size={32} />
            <p>Application is restarting...</p>
          </div>
        {:else if status === 'error'}
          <div class="error-state">
            <AlertCircle size={32} class="error-icon" />
            <p class="error-title">An error occurred during update:</p>
            <pre class="error-details">{errorMessage}</pre>
          </div>
        {/if}
      </div>

      <div class="modal-footer">
        {#if status === 'available'}
          <button class="btn btn-secondary" onclick={closeModal}>Later</button>
          <button class="btn btn-primary" onclick={startUpdate}>
            <Download size={16} />
            Update Now
          </button>
        {:else if status === 'error'}
          <button class="btn btn-secondary" onclick={closeModal}>Close</button>
          <button class="btn btn-primary" onclick={() => checkUpdate(false)}>
            <RefreshCw size={16} />
            Try Again
          </button>
        {/if}
      </div>

    </div>
  </div>
{/if}

<style>
  .modal-overlay {
    position: fixed;
    top: 0;
    left: 0;
    width: 100vw;
    height: 100vh;
    background: rgba(0, 0, 0, 0.7);
    backdrop-filter: blur(4px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 10000;
  }

  .update-modal {
    width: 460px;
    max-width: 90%;
    padding: 24px;
    border-radius: var(--radius-lg);
    display: flex;
    flex-direction: column;
    gap: 20px;
    box-shadow: 0 20px 40px rgba(0, 0, 0, 0.5);
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
  }

  .modal-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    border-bottom: 1px solid var(--border-subtle);
    padding-bottom: 12px;
  }

  .modal-header-left {
    display: flex;
    align-items: center;
    gap: 12px;
    color: var(--text-primary);
  }

  .modal-header-left h3 {
    font-size: 1.15rem;
    font-weight: 600;
    margin: 0;
    font-family: var(--font-display);
  }

  .header-icon {
    color: var(--accent-primary);
  }

  .spin {
    animation: spin 1.5s linear infinite;
  }

  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }

  .close-btn {
    background: transparent;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    padding: 4px;
    border-radius: var(--radius-sm);
    display: flex;
    align-items: center;
    justify-content: center;
    transition: var(--transition-fast);
  }

  .close-btn:hover {
    color: var(--text-primary);
    background: rgba(255, 255, 255, 0.05);
  }

  .modal-body {
    min-height: 120px;
    display: flex;
    flex-direction: column;
    justify-content: center;
  }

  .loading-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 16px;
    text-align: center;
    color: var(--text-muted);
    font-size: 0.95rem;
  }

  .loader {
    color: var(--accent-primary);
  }

  .update-info {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .version-badge {
    align-self: flex-start;
    background: var(--accent-muted);
    color: var(--text-primary);
    border: 1px solid var(--accent-primary);
    padding: 4px 10px;
    border-radius: 12px;
    font-size: 0.8rem;
    font-weight: 600;
    font-family: var(--font-mono);
  }

  .section-title {
    font-weight: 500;
    font-size: 0.9rem;
    color: var(--text-muted);
    margin-top: 4px;
  }

  .notes-container {
    background: var(--bg-element);
    border: 1px solid var(--border-subtle);
    padding: 12px;
    border-radius: var(--radius-md);
    font-size: 0.85rem;
    max-height: 180px;
    overflow-y: auto;
    color: var(--text-primary);
    line-height: 1.5;
    font-family: var(--font-sans);
  }

  .progress-container {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .progress-container p {
    font-size: 0.9rem;
    color: var(--text-muted);
  }

  .progress-bar-bg {
    background: var(--bg-element);
    height: 8px;
    border-radius: 4px;
    overflow: hidden;
    border: 1px solid var(--border-subtle);
  }

  .progress-bar-fill {
    background: var(--accent-primary);
    height: 100%;
    border-radius: 4px;
    transition: width 0.2s ease;
  }

  .progress-text {
    display: flex;
    justify-content: flex-end;
    font-size: 0.8rem;
    font-weight: 600;
    color: var(--text-primary);
    font-family: var(--font-mono);
  }

  .error-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    color: var(--text-primary);
  }

  .error-icon {
    color: var(--accent-red);
  }

  .error-title {
    font-weight: 500;
    color: var(--accent-red);
    font-size: 0.95rem;
  }

  .error-details {
    width: 100%;
    background: rgba(239, 68, 68, 0.05);
    border: 1px solid var(--accent-red-glow);
    padding: 12px;
    border-radius: var(--radius-md);
    font-family: var(--font-mono);
    font-size: 0.8rem;
    max-height: 120px;
    overflow-y: auto;
    white-space: pre-wrap;
    color: var(--text-primary);
  }

  .modal-footer {
    display: flex;
    justify-content: flex-end;
    gap: 12px;
    margin-top: 8px;
  }

  .btn {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 8px 16px;
    font-size: 0.85rem;
    font-weight: 500;
    border-radius: var(--radius-md);
    cursor: pointer;
    transition: var(--transition-fast);
    font-family: var(--font-sans);
  }

  .btn-primary {
    background: var(--accent-primary);
    border: 1px solid var(--accent-primary);
    color: white;
  }

  .btn-primary:hover {
    background: var(--accent-hover);
    border-color: var(--accent-hover);
  }

  .btn-secondary {
    background: var(--bg-element);
    border: 1px solid var(--border-subtle);
    color: var(--text-primary);
  }

  .btn-secondary:hover {
    background: rgba(255, 255, 255, 0.05);
    border-color: var(--text-muted);
  }
</style>
