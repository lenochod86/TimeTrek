import { invoke } from '@tauri-apps/api/core';

export interface Segment {
  id: number;
  kind: number; // 0 = active, 1 = idle / locked
  appId: number | null;
  title: string;
  domain: string | null;
  startMs: number;
  endMs: number;
}
export interface AppInfo {
  id: number;
  name: string;
}
export interface Tag {
  id: number;
  name: string;
  color: string;
}
export interface TagRange {
  id: number;
  tagId: number;
  startMs: number;
  endMs: number;
}
export interface RangeData {
  segments: Segment[];
  apps: AppInfo[];
  tags: Tag[];
  tagRanges: TagRange[];
}
export interface Shot {
  id: number;
  segmentId: number;
  ts: number;
}
export interface DaySummary {
  fromMs: number;
  startMs: number | null;
  endMs: number | null;
  activeMs: number;
  hourly: number[];
  apps: [number, number][];
  domains: [string, number][];
}
export interface AboutInfo {
  version: string;
  dataDir: string;
}

export const KIND_ACTIVE = 0;
export const KIND_IDLE = 1;

export const api = {
  getRange: (fromMs: number, toMs: number) => invoke<RangeData>('get_range', { fromMs, toMs }),
  getAppIcons: (ids: number[]) => invoke<Record<number, string>>('get_app_icons', { ids }),
  getFavicons: (domains: string[]) => invoke<Record<string, string>>('get_favicons', { domains }),
  getSegmentShots: (segmentId: number) =>
    invoke<Shot[]>('get_screenshots', { segmentId, fromMs: 0, toMs: 0 }),
  getRangeShots: (fromMs: number, toMs: number) =>
    invoke<Shot[]>('get_screenshots', { segmentId: null, fromMs, toMs }),
  getScreenshot: (id: number) => invoke<string | null>('get_screenshot', { id }),
  getSummaries: (days: { fromMs: number; toMs: number }[]) =>
    invoke<DaySummary[]>('get_summaries', { days }),
  addTagRange: (name: string, color: string, startMs: number, endMs: number) =>
    invoke<void>('add_tag_range', { name, color, startMs, endMs }),
  deleteTagRange: (id: number) => invoke<void>('delete_tag_range', { id }),
  getAppInfo: () => invoke<AboutInfo>('get_app_info'),
};
