export type UpdatePhase =
  'idle' | 'checking' | 'up_to_date' | 'update_available' | 'unavailable' | 'unsupported';

export type UpdateStatus = {
  policy?: 'stable' | 'prereleases';
  prereleaseSupported?: boolean;
  generation?: number;
  phase: UpdatePhase;
  installedVersion: string;
  availableVersion: string | null;
  edition: string;
  lastSuccessfulCheck: number | null;
  action: { type: 'open_url'; url: string } | null;
  message: string | null;
  automaticChecks: boolean;
  updateNotifications: boolean;
  promptDismissed: boolean;
  offerStale?: boolean;
};

const editions: Record<string, string> = {
  direct_macos: 'Direct download for macOS',
  direct_windows: 'Direct download for Windows',
  microsoft_store: 'Microsoft Store',
  mac_app_store: 'Mac App Store',
  debian: 'Debian package',
  windows_sideload: 'Sideloaded Windows package',
  development: 'Development build',
  unknown: 'Unknown or custom build',
};

export function updateStateText(status: UpdateStatus): string {
  switch (status.phase) {
    case 'checking':
      return 'Checking for updates…';
    case 'up_to_date':
      return 'This edition is up to date.';
    case 'update_available':
      return status.offerStale
        ? `Version ${status.availableVersion ?? ''} was available at the last successful check. Check again before opening it.`
        : `Version ${status.availableVersion ?? ''} is available.`;
    case 'unavailable':
      return status.message ?? 'Update information is currently unavailable.';
    case 'unsupported':
      return 'Automatic update checks are unavailable for this build.';
    default:
      return 'Not checked yet.';
  }
}

export function editionLabel(edition: string): string {
  return editions[edition] ?? 'Unknown or custom build';
}

export function releasePolicyControl(status: UpdateStatus): string {
  if (!status.prereleaseSupported) return '';
  return `<label class="toggle"><span>Release policy</span><select id="update-policy" aria-label="Release policy"><option value="stable"${status.policy !== 'prereleases' ? ' selected' : ''}>Stable only</option><option value="prereleases"${status.policy === 'prereleases' ? ' selected' : ''}>Include prereleases</option></select></label><p class="setting-note">Include prereleases includes public Alpha/Beta releases and newer stable releases. Switching to Stable only never downgrades: you will receive the next newer stable release.</p>`;
}
