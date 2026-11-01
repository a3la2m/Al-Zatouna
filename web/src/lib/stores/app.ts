import { writable } from 'svelte/store';

/// UI app store: theme, session user (Phase 3 wires auth), sidebar font.
import type { SessionUserDto } from '$lib/api/auth';

export type SessionUser = SessionUserDto;

export const theme = writable<'light' | 'dark'>('light');
export const sessionUser = writable<SessionUser | null>(null);
export const sessionToken = writable<string | null>(null);
export const font = writable<string>('Cairo');
export const sidebarCollapsed = writable(false);
