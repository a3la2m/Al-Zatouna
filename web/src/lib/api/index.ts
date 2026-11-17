/// Typed Tauri IPC wrapper — the ONLY way the frontend talks to Rust.
/// When running in a plain browser (dev preview) calls return a friendly
/// error instead of pretending success.
import { isTauri } from '$lib/env';
import { sessionExpired, sessionToken, sessionUser, clearPersistedSession } from '$lib/stores/app';

type InvokeArgs = Record<string, unknown> | undefined;

type InvokeFn = <T>(cmd: string, args?: InvokeArgs) => Promise<T>;

let invokeImpl: InvokeFn | null = null;
let loading: Promise<InvokeFn | null> | null = null;

/// Lazy-load @tauri-apps/api/core on the first real call (no top-level
/// await — keeps the built target compatible with older esbuild targets).
async function loadInvoke(): Promise<InvokeFn | null> {
  if (invokeImpl) return invokeImpl;
  if (!isTauri) return null;
  loading ??= import('@tauri-apps/api/core').then((m) => {
    invokeImpl = m.invoke as InvokeFn;
    return invokeImpl;
  });
  return await loading;
}

export class ApiError extends Error {
  code: string;
  field?: string;
  details?: unknown;

  constructor(dto: { code: string; message: string; field?: string; details?: unknown }) {
    super(dto.message);
    this.code = dto.code;
    this.field = dto.field;
    this.details = dto.details;
  }
}

export async function invoke<T = unknown>(
  cmd: string,
  args?: InvokeArgs
): Promise<T> {
  const fn = await loadInvoke();
  if (!fn) {
    throw new ApiError({
      code: 'not_in_tauri',
      message: 'يمكن تشغيل هذا الإجراء داخل التطبيق فقط'
    });
  }
  try {
    return await fn<T>(cmd, args);
  } catch (e) {
    if (isSessionExpired(e)) {
      sessionToken.set(null);
      sessionUser.set(null);
      clearPersistedSession();
      sessionExpired.set(true);
    }
    throw e;
  }
}

/// The Rust service reports an expired session with this specific code + message.
function isSessionExpired(e: unknown): boolean {
  const msg = (e as { message?: string })?.message ?? '';
  const code = (e as { code?: string })?.code ?? '';
  return code === 'permission_denied' &&
    (msg.includes('الجلسة غير سارية') || msg.includes('أعد تسجيل الدخول'));
}

export async function safeInvoke<T = unknown>(
  cmd: string,
  args?: InvokeArgs
): Promise<{ ok: true; data: T } | { ok: false; error: ApiError }> {
  try {
    return { ok: true, data: await invoke<T>(cmd, args) };
  } catch (e) {
    return {
      ok: false,
      error:
        e instanceof ApiError
          ? e
          : new ApiError({
              code: 'internal',
              message: (e as Error)?.message ?? 'خطأ غير متوقع'
            })
    };
  }
}
