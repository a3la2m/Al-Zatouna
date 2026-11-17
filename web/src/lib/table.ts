import type { Snippet } from 'svelte';

/// A single column of `DataTable`.
///
/// A cell is rendered with, in order of preference:
///   1. `render` — a snippet, when the cell needs markup (badges, links, buttons)
///   2. `format` — a pure value → string mapping
///   3. the raw value, with `—` for null/undefined
export type Column<T> = {
  key: string;
  label: string;
  /// Right-align + tabular numerals (money, counts, ids).
  num?: boolean;
  align?: 'start' | 'end' | 'center';
  width?: string;
  format?: (value: unknown, row: T) => string;
  render?: Snippet<[T]>;
};
