function hash(s: string): number {
  let h = 2166136261;
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  return h >>> 0;
}

const cache = new Map<string, string>();

/** Stable, well distributed colour for any key (app id, document title...). */
export function colorFor(key: string): string {
  let c = cache.get(key);
  if (!c) {
    const h = hash(key);
    const hue = h % 360;
    const sat = 55 + ((h >> 9) % 35);
    const light = 40 + ((h >> 17) % 20);
    c = `hsl(${hue} ${sat}% ${light}%)`;
    cache.set(key, c);
  }
  return c;
}

export const TAG_COLORS = ['#8a9a0a', '#7a6bc4', '#8c1c3f', '#1a8f9a', '#c47a1a', '#3a7bd5', '#b03a8c'];
