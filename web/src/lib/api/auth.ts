/// Auth IPC surface — mirrors src-tauri auth_* commands (Phase 3).
import { invoke } from '$lib/api';

export type SessionUserDto = {
  user_id: number;
  username: string;
  full_name: string;
  permissions: string[];
};

export type LoginResultDto = {
  token: string;
  user: SessionUserDto;
  must_change_password: boolean;
};

export const authApi = {
  createAdmin: (username: string, fullName: string, password: string) =>
    invoke<{ user_id: number }>('auth_create_admin', {
      username,
      full_name: fullName,
      password
    }),

  login: (username: string, password: string) =>
    invoke<LoginResultDto>('auth_login', { username, password }),

  logout: (token: string) => invoke<void>('auth_logout', { token }),

  me: (token: string) => invoke<SessionUserDto>('auth_me', { token }),

  changePassword: (token: string, current: string, newPassword: string) =>
    invoke<void>('auth_change_password', { token, current, new_password: newPassword })
};
