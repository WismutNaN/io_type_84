import { invoke, isTauri } from '@tauri-apps/api/core';
import type { AppInfo } from './generated';

export type ShellConnection =
  | { kind: 'loading' }
  | { kind: 'desktop'; info: AppInfo }
  | { kind: 'browser' }
  | { kind: 'error'; message: string };

export async function connectShell(): Promise<ShellConnection> {
  if (!isTauri()) return { kind: 'browser' };
  try {
    return { kind: 'desktop', info: await invoke<AppInfo>('app_info') };
  } catch (error) {
    return { kind: 'error', message: String(error) };
  }
}
