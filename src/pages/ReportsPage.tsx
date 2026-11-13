import { useEffect, useMemo, useState } from 'react';
import { api } from '../api';
import { RangeBar, useRangeSummaries } from '../components/RangeBar';
import { useIcons } from '../hooks';
import { t } from '../i18n';
import { colorFor } from '../utils/color';
import { HOUR, fmtDate, fmtDuration, fmtHours } from '../utils/time';

export function ReportsPage() {
  const state = useRangeSummaries();
  const { dayList, days } = state;
  const { icons, ensure } = useIcons();
  const [appNames, setAppNames] = useState<Map<number, string>>(new Map());

  useEffect(() => {
    api.getRange(0, 1).then((d) => setAppNames(new Map(d.apps.map((a) => [a.id, a.name] as [number, string]))));
  }, [days]);

  const apps = useMemo(() => {
    const m = new Map<number, number>();
    days.forEach((d) => d.apps.forEach(([id, ms]) => m.set(id, (m.get(id) ?? 0) + ms)));
    return [...m.entries()].sort((a, b) => b[1] - a[1]).slice(0, 12);
  }, [days]);

  const sites = useMemo(() => {
    const m = new Map<string, number>();
    days.forEach((d) => d.domains.forEach(([dom, ms]) => m.set(dom, (m.get(dom) ?? 0) + ms)));
    return [...m.entries()].sort((a, b) => b[1] - a[1]).slice(0, 12);
  }, [days]);

  useEffect(() => {
    ensure(apps.map((a) => a[0]), sites.map((s) => s[0]));
  }, [apps, sites, ensure]);

  const maxDay = Math.max(1, ...days.map((d) => d.activeMs));
  const maxApp = Math.max(1, ...apps.map((a) => a[1]));
  const maxSite = Math.max(1, ...sites.map((s) => s[1]));

  // weekday (Mon=0) x hour matrix, average active minutes per occurrence of that weekday
  const heat = useMemo(() => {
    const sum = Array.from({ length: 7 }, () => new Array(24).fill(0));
    const count = new Array(7).fill(0);
    days.forEach((d) => {
      const wd = (new Date(d.fromMs).getDay() + 6) % 7;
      count[wd]++;
      d.hourly.forEach((ms, h) => (sum[wd][h] += ms));
    });
    return sum.map((row, wd) => row.map((ms) => (count[wd] ? ms / count[wd] : 0)));
  }, [days]);
  const maxHeat = Math.max(1, ...heat.flat());
  const empty = days.every((d) => d.activeMs === 0);

  return (
    <div className="page reports">
      <RangeBar state={state} />
      <div className="reports-grid">
        <section className="panel card wide">
          <h3>{t.repDaily}</h3>
          {empty ? (
            <div className="empty">{t.repNone}</div>
          ) : (
            <div className="daily-bars">
              {days.map((d, i) => (
                <div className="daily-col" key={d.fromMs} title={`${fmtDate(dayList[i])}: ${fmtHours(d.activeMs)} h`}>
                  <span className="daily-val">{d.activeMs ? fmtHours(d.activeMs) : ''}</span>
                  <div className="daily-bar" style={{ height: `${(d.activeMs / maxDay) * 100}%` }} />
                  {days.length <= 31 && <span className="daily-label">{dayList[i].getDate()}.{dayList[i].getMonth() + 1}.</span>}
                </div>
              ))}
            </div>
          )}
        </section>

        <section className="panel card">
          <h3>{t.repApps}</h3>
          {apps.length === 0 && <div className="empty">{t.repNone}</div>}
          {apps.map(([id, ms]) => (
            <div className="bar-row" key={id}>
              <span className="cell-icon">{icons.apps[id] ? <img src={icons.apps[id]} alt="" width={16} height={16} /> : <span className="icon-ph" />}</span>
              <span className="bar-name">{appNames.get(id) ?? '?'}</span>
              <span className="hbar">
                <span style={{ width: `${(ms / maxApp) * 100}%`, background: colorFor(`app:${id}`) }} />
              </span>
              <span className="bar-time">{fmtDuration(ms)}</span>
            </div>
          ))}
        </section>

        <section className="panel card">
          <h3>{t.repSites}</h3>
          {sites.length === 0 && <div className="empty">{t.repNone}</div>}
          {sites.map(([dom, ms]) => (
            <div className="bar-row" key={dom}>
              <span className="cell-icon">{icons.sites[dom] ? <img src={icons.sites[dom]} alt="" width={16} height={16} /> : <span className="icon-ph" />}</span>
              <span className="bar-name">{dom}</span>
              <span className="hbar">
                <span style={{ width: `${(ms / maxSite) * 100}%`, background: colorFor(`site:${dom}`) }} />
              </span>
              <span className="bar-time">{fmtDuration(ms)}</span>
            </div>
          ))}
        </section>

        <section className="panel card wide">
          <h3>{t.repHeat}</h3>
          <div className="heat">
            <div className="heat-hours">
              <span />
              {Array.from({ length: 24 }, (_, h) => (
                <span key={h}>{h}</span>
              ))}
            </div>
            {heat.map((row, wd) => (
              <div className="heat-row" key={wd}>
                <span className="heat-day">{t.weekdays[wd]}</span>
                {row.map((ms, h) => (
                  <span
                    key={h}
                    className="heat-cell"
                    title={`${t.weekdays[wd]} ${h}:00 – ${Math.round(ms / 60000)} min`}
                    style={{ background: ms > 0 ? `rgba(44, 255, 176, ${0.12 + 0.88 * Math.min(1, ms / Math.min(maxHeat, HOUR))})` : undefined }}
                  />
                ))}
              </div>
            ))}
          </div>
        </section>
      </div>
    </div>
  );
}
