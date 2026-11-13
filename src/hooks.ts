import { useCallback, useRef, useState } from 'react';
import { api } from './api';

export interface Icons {
  apps: Record<number, string>;
  sites: Record<string, string>;
}

/**
 * Keeps app icons and website favicons in memory. Favicons are downloaded by the
 * tracker in the background, so missing ones are simply asked for again on the
 * next refresh.
 */
export function useIcons() {
  const [icons, setIcons] = useState<Icons>({ apps: {}, sites: {} });
  const ref = useRef(icons);
  ref.current = icons;

  const ensure = useCallback(async (appIds: number[], domains: string[]) => {
    const missingApps = appIds.filter((id) => !(id in ref.current.apps));
    const missingSites = domains.filter((d) => !(d in ref.current.sites));
    if (!missingApps.length && !missingSites.length) return;
    const [a, s] = await Promise.all([
      missingApps.length ? api.getAppIcons(missingApps) : Promise.resolve({}),
      missingSites.length ? api.getFavicons(missingSites) : Promise.resolve({}),
    ]);
    if (Object.keys(a).length || Object.keys(s).length) {
      setIcons((prev) => ({ apps: { ...prev.apps, ...a }, sites: { ...prev.sites, ...s } }));
    }
  }, []);

  return { icons, ensure };
}

const shotCache = new Map<number, string>();

/** Screenshot data URL with a small in-memory cache. */
export async function loadShot(id: number): Promise<string | null> {
  const hit = shotCache.get(id);
  if (hit) return hit;
  const url = await api.getScreenshot(id);
  if (url) {
    if (shotCache.size > 60) shotCache.delete(shotCache.keys().next().value as number);
    shotCache.set(id, url);
  }
  return url;
}
