export const SEC = 1000;
export const MIN = 60 * SEC;
export const HOUR = 60 * MIN;
export const DAY = 24 * HOUR;

const pad = (n: number, len = 2) => String(Math.floor(n)).padStart(len, '0');

/** Local midnight of the given date. */
export function startOfDay(d: Date): Date {
  return new Date(d.getFullYear(), d.getMonth(), d.getDate());
}

export function addDays(d: Date, n: number): Date {
  return new Date(d.getFullYear(), d.getMonth(), d.getDate() + n);
}

export function sameDay(a: Date, b: Date): boolean {
  return a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate();
}

/** `YYYY-MM-DD` for <input type="date">. */
export function toInputDate(d: Date): string {
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

export function fromInputDate(s: string): Date | null {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(s);
  return m ? new Date(+m[1], +m[2] - 1, +m[3]) : null;
}

export function fmtDate(d: Date): string {
  return `${pad(d.getDate())}.${pad(d.getMonth() + 1)}.${d.getFullYear()}`;
}

/** HH:MM:SS clock time. */
export function fmtClock(ms: number): string {
  const d = new Date(ms);
  return `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
}

/** H:MM clock time. */
export function fmtHM(ms: number | null): string {
  if (ms == null) return '';
  const d = new Date(ms);
  return `${d.getHours()}:${pad(d.getMinutes())}`;
}

/** Duration as HH:MM:SS (hours not limited to 24). */
export function fmtDuration(ms: number): string {
  const s = Math.max(0, Math.round(ms / 1000));
  return `${pad(s / 3600)}:${pad((s % 3600) / 60)}:${pad(s % 60)}`;
}

/** Duration as H:MM (header of the day). */
export function fmtDurationHM(ms: number): string {
  const m = Math.max(0, Math.round(ms / 60000));
  return `${Math.floor(m / 60)}:${pad(m % 60)}`;
}

/** Decimal hours with a Czech comma, e.g. `4,36`. */
export function fmtHours(ms: number): string {
  return (ms / HOUR).toFixed(2).replace('.', ',');
}

/** Minutes since local midnight of the day that contains `ms`, for averages of start times. */
export function minutesOfDay(ms: number): number {
  const d = new Date(ms);
  return d.getHours() * 60 + d.getMinutes();
}

export function fmtMinutesOfDay(min: number): string {
  const m = Math.round(min);
  return `${Math.floor(m / 60)}:${pad(m % 60)}`;
}
