/// Typed Tauri IPC wrapper — the ONLY way the frontend talks to Rust.
/// When running in a plain browser (dev preview) calls return a friendly
/// error instead of pretending success.
import { isTauri } from '$lib/env';

type InvokeArgs = Record<string, unknown> | undefined;

let invokeImpl: <T>(cmd: string, args?: InvokeArgs) => Promise<T> | null = null;

if (isTauri) {
  // dynamic import keeps the plain-browser dev preview working
  const tauriApi = await import('@tauri-apps/api/core');
  invokeImpl = tauriApi.invoke;
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
  if (!invokeImpl) {
    throw new ApiError({
      code: 'not_in_tauri',
      message: 'يمكن تشغيل هذا الإجراء داخل التطبيق فقط'
    });
  }
  return await invokeImpl<T>(cmd, args);
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
