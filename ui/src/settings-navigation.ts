export const settingsPages = [
  'devices',
  'speaker',
  'schedule',
  'volume',
  'general',
  'updates',
  'diagnostics',
  'about',
] as const;
export type SettingsPage = (typeof settingsPages)[number];

export function adjacentPage(page: SettingsPage, direction: -1 | 1): SettingsPage | undefined {
  return settingsPages[settingsPages.indexOf(page) + direction];
}

export interface SettingsNavigationRequest {
  revision: number;
  page: string | null;
}

/** Native requests, cancellation events, and startup IPC share one revision order. */
export class SettingsNavigation {
  private revision = 0;

  private readonly open: (page: string | null) => void;

  constructor(open: (page: string | null) => void) {
    this.open = open;
  }

  accept(request: SettingsNavigationRequest | null): void {
    if (!request || !Number.isSafeInteger(request.revision) || request.revision <= this.revision)
      return;
    this.revision = request.revision;
    // A null page cancels an older response without changing the visible page.
    if (request.page) this.open(request.page);
  }
}
