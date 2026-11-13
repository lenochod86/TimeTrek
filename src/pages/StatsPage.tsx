import { useMemo } from 'react';
import { RangeBar, useRangeSummaries } from '../components/RangeBar';
import { t } from '../i18n';
import { addDays, fmtDate, fmtHM, fmtHours, fmtMinutesOfDay, minutesOfDay } from '../utils/time';

export function StatsPage() {
  const state = useRangeSummaries();
  const { dayList, days } = state;

  const rows = useMemo(
    () =>
      dayList.map((d, i) => {
        const s = days[i];
        const has = !!s && s.startMs != null && s.endMs != null;
        return {
          date: d,
          // "Délka" is the span of the working day (first to last activity), like the day header.
          length: has ? s.endMs! - s.startMs! : 0,
          start: has ? s.startMs! : null,
          end: has ? s.endMs! : null,
        };
      }),
    [dayList, days],
  );

  const worked = rows.filter((r) => r.start != null);
  const sum = worked.reduce((a, r) => a + r.length, 0);
  const n = worked.length;
  const agg = (f: (r: (typeof rows)[number]) => number, pick: 'avg' | 'min' | 'max') => {
    if (!n) return null;
    const v = worked.map(f);
    return pick === 'avg' ? v.reduce((a, b) => a + b, 0) / n : pick === 'min' ? Math.min(...v) : Math.max(...v);
  };

  const summaryRow = (label: string, pick: 'avg' | 'min' | 'max') => {
    const len = agg((r) => r.length, pick);
    const st = agg((r) => minutesOfDay(r.start!), pick);
    const en = agg((r) => minutesOfDay(r.end!), pick);
    return (
      <tr key={label} className="summary">
        <td />
        <td>{label}</td>
        <td className="num">{len == null ? '' : `${fmtHours(len)} ${t.hoursShort}`}</td>
        <td className="num">{st == null ? '' : fmtMinutesOfDay(st)}</td>
        <td className="num">{en == null ? '' : fmtMinutesOfDay(en)}</td>
      </tr>
    );
  };

  return (
    <div className="page stats">
      <RangeBar state={state} />
      <div className="panel table-panel">
        <table className="stats-table">
          <thead>
            <tr>
              <th>{t.from}</th>
              <th>{t.to}</th>
              <th className="num">{t.colLength}</th>
              <th className="num">{t.dayStart}</th>
              <th className="num">{t.dayEnd}</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((r) => (
              <tr key={r.date.getTime()}>
                <td>{fmtDate(r.date)}</td>
                <td>{fmtDate(addDays(r.date, 0))}</td>
                <td className="num">{`${fmtHours(r.length)} ${t.hoursShort}`}</td>
                <td className="num">{fmtHM(r.start)}</td>
                <td className="num">{fmtHM(r.end)}</td>
              </tr>
            ))}
            <tr className="total">
              <td />
              <td>{t.rowTotal}</td>
              <td className="num">{`${fmtHours(sum)} ${t.hoursShort}`}</td>
              <td />
              <td />
            </tr>
            {summaryRow(t.rowAvg, 'avg')}
            {summaryRow(t.rowMin, 'min')}
            {summaryRow(t.rowMax, 'max')}
          </tbody>
        </table>
      </div>
    </div>
  );
}
