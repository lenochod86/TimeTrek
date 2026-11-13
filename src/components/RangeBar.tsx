import { useCallback, useEffect, useMemo, useState } from 'react';
import { api, type DaySummary } from '../api';
import { t } from '../i18n';
import { addDays, fmtDate, fromInputDate, startOfDay, toInputDate } from '../utils/time';
import { IconChevLeft, IconChevRight } from './Icons';

type Preset = 'today' | 'thisWeek' | 'lastWeek' | 'thisMonth' | 'lastMonth' | 'last7' | 'last30' | 'custom';

interface Range {
  from: Date;
  to: Date; // inclusive day
  preset: Preset;
}

function mondayOf(d: Date): Date {
  const wd = (d.getDay() + 6) % 7;
  return addDays(startOfDay(d), -wd);
}

function presetRange(p: Preset, today = startOfDay(new Date())): Range {
  switch (p) {
    case 'today':
      return { from: today, to: today, preset: p };
    case 'thisWeek': {
      const m = mondayOf(today);
      return { from: m, to: addDays(m, 6), preset: p };
    }
    case 'lastWeek': {
      const m = addDays(mondayOf(today), -7);
      return { from: m, to: addDays(m, 6), preset: p };
    }
    case 'thisMonth':
      return { from: new Date(today.getFullYear(), today.getMonth(), 1), to: new Date(today.getFullYear(), today.getMonth() + 1, 0), preset: p };
    case 'lastMonth':
      return { from: new Date(today.getFullYear(), today.getMonth() - 1, 1), to: new Date(today.getFullYear(), today.getMonth(), 0), preset: p };
    case 'last7':
      return { from: addDays(today, -6), to: today, preset: p };
    case 'last30':
      return { from: addDays(today, -29), to: today, preset: p };
    default:
      return { from: today, to: today, preset: 'custom' };
  }
}

/** Date range selection + the per-day summaries for it. */
export function useRangeSummaries() {
  const [range, setRange] = useState<Range>(() => presetRange('thisWeek'));
  const [days, setDays] = useState<DaySummary[]>([]);

  const dayList = useMemo(() => {
    const out: Date[] = [];
    for (let d = range.from; d <= range.to && out.length < 400; d = addDays(d, 1)) out.push(d);
    return out;
  }, [range]);

  const load = useCallback(async () => {
    const res = await api.getSummaries(dayList.map((d) => ({ fromMs: d.getTime(), toMs: addDays(d, 1).getTime() })));
    setDays(res);
  }, [dayList]);

  useEffect(() => {
    load();
    const id = setInterval(() => !document.hidden && load(), 10_000);
    return () => clearInterval(id);
  }, [load]);

  return { range, setRange, dayList, days };
}

export type RangeState = ReturnType<typeof useRangeSummaries>;

export function RangeBar({ state }: { state: RangeState }) {
  const { range, setRange } = state;

  const shift = (dir: 1 | -1) => {
    if (range.preset === 'thisMonth' || range.preset === 'lastMonth') {
      const from = new Date(range.from.getFullYear(), range.from.getMonth() + dir, 1);
      const to = new Date(from.getFullYear(), from.getMonth() + 1, 0);
      return setRange({ from, to, preset: 'custom' });
    }
    const len = Math.round((range.to.getTime() - range.from.getTime()) / 86_400_000) + 1;
    setRange({ from: addDays(range.from, dir * len), to: addDays(range.to, dir * len), preset: 'custom' });
  };

  const presets: [Preset, string][] = [
    ['today', t.presetToday],
    ['thisWeek', t.presetThisWeek],
    ['lastWeek', t.presetLastWeek],
    ['thisMonth', t.presetThisMonth],
    ['lastMonth', t.presetLastMonth],
    ['last7', t.presetLast7],
    ['last30', t.presetLast30],
    ['custom', t.presetCustom],
  ];

  const setFrom = (s: string) => {
    const d = fromInputDate(s);
    if (d) setRange({ from: d, to: d > range.to ? d : range.to, preset: 'custom' });
  };
  const setTo = (s: string) => {
    const d = fromInputDate(s);
    if (d) setRange({ from: d < range.from ? d : range.from, to: d, preset: 'custom' });
  };

  return (
    <div className="toolbar">
      <label className="range-label">
        {t.from}
        <input type="date" className="date-input" value={toInputDate(range.from)} onChange={(e) => setFrom(e.target.value)} />
      </label>
      <label className="range-label">
        {t.to}
        <input type="date" className="date-input" value={toInputDate(range.to)} onChange={(e) => setTo(e.target.value)} />
      </label>
      <button className="icon-btn boxed" onClick={() => shift(-1)}>
        <IconChevLeft />
      </button>
      <button className="icon-btn boxed" onClick={() => shift(1)}>
        <IconChevRight />
      </button>
      <select
        className="select"
        value={range.preset}
        onChange={(e) => {
          const p = e.target.value as Preset;
          if (p !== 'custom') setRange(presetRange(p));
          else setRange({ ...range, preset: 'custom' });
        }}
      >
        {presets.map(([id, label]) => (
          <option key={id} value={id}>
            {label}
          </option>
        ))}
      </select>
      <span className="toolbar-spacer" />
      <span className="hint">
        {fmtDate(range.from)} – {fmtDate(range.to)}
      </span>
    </div>
  );
}
