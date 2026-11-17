import { writable } from 'svelte/store';

/// UI app store: theme, session user (Phase 3 wires auth), sidebar font.
import type { SessionUserDto } from '$lib/api/auth';

export type SessionUser = SessionUserDto;

export const theme = writable<'light' | 'dark'>('light');
export const sessionUser = writable<SessionUser | null>(null);
export const sessionToken = writable<string | null>(null);
export const font = writable<string>('Cairo');
export const sidebarCollapsed = writable(false);

/// Set by the IPC layer when the server reports an expired/invalid session.
export const sessionExpired = writable(false);

/* ---------- persistent ("remember me") session ---------- */
const LS_KEY = 'ec.session';

export function persistSession(token: string, user: SessionUser): void {
  try { localStorage.setItem(LS_KEY, JSON.stringify({ token, user })); } catch { /* ignore */ }
}

export function clearPersistedSession(): void {
  try { localStorage.removeItem(LS_KEY); } catch { /* ignore */ }
}

export function restoreSession(): boolean {
  try {
    const raw = localStorage.getItem(LS_KEY);
    if (!raw) return false;
    const data = JSON.parse(raw) as { token: string; user: SessionUser };
    if (!data.token || !data.user) return false;
    sessionToken.set(data.token);
    sessionUser.set(data.user);
    return true;
  } catch { return false; }
}
