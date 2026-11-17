/// Tiny toast queue — success/error feedback without blocking the UI.
import { writable } from 'svelte/store';

export type Toast = {
  id: number;
  kind: 'ok' | 'error' | 'info';
  text: string;
};

export const toasts = writable<Toast[]>([]);

let nextId = 1;

function push(kind: Toast['kind'], text: string, ms = 3800) {
  const id = nextId++;
  toasts.update((list) => [...list, { id, kind, text }]);
  setTimeout(() => dismiss(id), ms);
}

export const toast = {
  ok: (text: string) => push('ok', text),
  error: (text: string) => push('error', text, 6000),
  info: (text: string) => push('info', text)
};

export function dismiss(id: number) {
  toasts.update((list) => list.filter((t) => t.id !== id));
}
