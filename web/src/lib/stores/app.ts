import { writable } from 'svelte/store';

/// UI app store: theme, sidebar font label, session user (Phase 3 wires it).
export type SessionUser = {
  id: number;
  username: string;
  full_name: string;
  permissions: string[];
};

export const theme = writable<'light' | 'dark'>('light');
export const sessionUser = writable<SessionUser | null>(null);
export const font = writable<string>('Cairo');
export const sidebarCollapsed = writable(false);
