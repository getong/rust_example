import { request } from './api.ts';
export type Theme = 'ocean' | 'sunset' | 'forest';
export interface StudioSnapshot { theme: Theme; intensity: number; revision: number }
export interface ApplyPalette { action: 'apply'; theme: Theme; intensity: number }
export const themes: Record<Theme, {label: string; color: string}> = {
  ocean: {label: '海洋蓝', color: '#2563eb'},
  sunset: {label: '落日橙', color: '#ea580c'},
  forest: {label: '森林绿', color: '#16a34a'},
};
export function isTheme(value: unknown): value is Theme {
  return value === 'ocean' || value === 'sunset' || value === 'forest';
}
export function decodeStudio(value: unknown): StudioSnapshot {
  if (typeof value !== 'object' || value === null) throw new Error('配色响应无效');
  const data = value as Record<string, unknown>;
  if (!isTheme(data.theme) || typeof data.intensity !== 'number' || !Number.isInteger(data.intensity)
    || data.intensity < 0 || data.intensity > 100 || typeof data.revision !== 'number'
    || !Number.isSafeInteger(data.revision) || data.revision < 0) throw new Error('配色响应无效');
  return {theme: data.theme, intensity: data.intensity, revision: data.revision};
}
export const studioApi = {
  read: () => request('/api/studio', decodeStudio),
  apply: (theme: Theme, intensity: number) => {
    const command: ApplyPalette = {action: 'apply', theme, intensity};
    return request('/api/studio', decodeStudio, JSON.stringify(command));
  },
};
