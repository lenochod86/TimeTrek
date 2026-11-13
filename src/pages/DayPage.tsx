import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { api, KIND_ACTIVE, type RangeData, type Segment } from '../api';
import { AppRanking, type AppRow } from '../components/AppRanking';
import { IconChevLeft, IconChevRight } from '../components/Icons';
import { SegmentList } from '../components/SegmentList';
import { ShotPreview, ShotViewer } from '../components/Shots';
import { Timeline } from '../components/Timeline';
import { useIcons } from '../hooks';
import { t } from '../i18n';
import { DAY, MIN, addDays, fmtDuration, fmtDurationHM, fmtHM, fromInputDate, sameDay, startOfDay, toInputDate } from '../utils/time';

const EMPTY: RangeData = { segments: [], apps: [], tags: [], tagRanges: [] };

export function DayPage() {
  const [date, setDate] = useState(() => startOfDay(new Date()));
  const [data, setData] = useState<RangeData>(EMPTY);
  const [nowMs, setNowMs] = useState(Date.now());
  const [fit, setFit] = useState(true);
  const [view, setView] = useState<{ start: number; end: number }>({ start: 0, end: 1 });
  const [checkedApps, setCheckedApps] = useState<Set<number>>(new Set());
  const [checkedRows, setCheckedRows] = useState<Set<number>>(new Set());
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [scrollToken, setScrollToken] = useState(0);
  const [hover, setHover] = useState<{ seg: Segment; x: number; y: number } | null>(null);
  const [locked, setLocked] = useState<{ seg: Segment; x: number; y: number } | null>(null);
  const [viewerOpen, setViewerOpen] = useState(false);
  const { icons, ensure } = useIcons();

  const dayStart = date.getTime();
  const dayEnd = addDays(date, 1).getTime();
  const isToday = sameDay(date, new Date());

  const load = useCallback(async () => {
    const d = await api.getRange(dayStart, dayEnd);
    setData(d);
    setNowMs(Date.now());
    const domains = [...new Set(d.segments.map((s) => s.domain).filter((x): x is string => !!x))];
    ensure(d.apps.map((a) => a.id), domains);
  }, [dayStart, dayEnd, ensure]);

  useEffect(() => {
    setData(EMPTY);
    setFit(true);
    setCheckedApps(new Set());
    setCheckedRows(new Set());
    setSelectedId(null);
    setLocked(null);
    load();
  }, [load]);

  // Live refresh for today (only while the window is actually visible).
  useEffect(() => {
    if (!isToday) return;
    const id = setInterval(() => !document.hidden && load(), 3000);
    return () => clearInterval(id);
  }, [isToday, load]);

  const appNames = useMemo(() => new Map(data.apps.map((a) => [a.id, a.name])), [data.apps]);
  const segById = useMemo(() => new Map(data.segments.map((s) => [s.id, s])), [data.segments]);

  // Segments clipped to the day (a segment may span midnight).
  const clipped = useMemo(
    () => data.segments.map((s) => ({ ...s, startMs: Math.max(s.startMs, dayStart), endMs: Math.min(s.endMs, dayEnd) })),
    [data.segments, dayStart, dayEnd],
  );
  const active = useMemo(() => clipped.filter((s) => s.kind === KIND_ACTIVE), [clipped]);

  const dayInfo = useMemo(() => {
    if (!active.length) return null;
    const start = Math.min(...active.map((s) => s.startMs));
    const end = Math.max(...active.map((s) => s.endMs));
    return { start, end };
  }, [active]);

  // Keep the visible range fitted to the day until the user zooms / pans.
  useEffect(() => {
    if (!fit) return;
    if (dayInfo) {
      const end = Math.max(dayInfo.end, isToday ? nowMs : 0);
      setView({ start: Math.max(dayStart, dayInfo.start - 5 * MIN), end: Math.min(dayEnd, end + 5 * MIN) });
    } else {
      setView({ start: dayStart + 8 * 3600_000, end: dayStart + 18 * 3600_000 });
    }
  }, [fit, dayInfo, dayStart, dayEnd, isToday, nowMs]);

  const filtered = checkedApps.size > 0;
  const rows = useMemo(() => active.filter((s) => !filtered || (s.appId != null && checkedApps.has(s.appId))), [active, filtered, checkedApps]);

  const ranking = useMemo<AppRow[]>(() => {
    const sum = new Map<number, number>();
    for (const s of active) if (s.appId != null) sum.set(s.appId, (sum.get(s.appId) ?? 0) + (s.endMs - s.startMs));
    return [...sum.entries()]
      .map(([id, ms]) => ({ id, ms, name: appNames.get(id) ?? '?' }))
      .sort((a, b) => b.ms - a.ms);
  }, [active, appNames]);
  const allTotal = ranking.reduce((a, r) => a + r.ms, 0);
  const shownTotal = filtered ? ranking.filter((r) => checkedApps.has(r.id)).reduce((a, r) => a + r.ms, 0) : allTotal;

  const selectedSum = useMemo(() => rows.filter((s) => checkedRows.has(s.id)).reduce((a, s) => a + (s.endMs - s.startMs), 0), [rows, checkedRows]);

  // --- hover preview + keyboard -------------------------------------------
  const hoverTimer = useRef<number | undefined>(undefined);
  const onHover = useCallback((seg: Segment | null, x: number, y: number) => {
    window.clearTimeout(hoverTimer.current);
    if (!seg) return setHover(null);
    hoverTimer.current = window.setTimeout(() => setHover({ seg, x, y }), 200);
  }, []);

  const shown = locked ?? hover;

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'F11') {
        e.preventDefault();
        setLocked((l) => (l ? null : hover));
      } else if (e.key === 'F12') {
        e.preventDefault();
        setViewerOpen((v) => !v);
      } else if (e.key === 'Escape') {
        setLocked(null);
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [hover]);

  const toggleApp = (id: number) =>
    setCheckedApps((s) => {
      const n = new Set(s);
      n.has(id) ? n.delete(id) : n.add(id);
      return n;
    });
  const toggleRow = (id: number) =>
    setCheckedRows((s) => {
      const n = new Set(s);
      n.has(id) ? n.delete(id) : n.add(id);
      return n;
    });

  const pickFromTimeline = (id: number) => {
    setSelectedId(id);
    setScrollToken((x) => x + 1);
  };

  const viewerFocus = shown?.seg ?? (selectedId != null ? segById.get(selectedId) ?? null : null);

  return (
    <div className="page day">
      <div className="toolbar">
        <input
          type="date"
          className="date-input"
          value={toInputDate(date)}
          onChange={(e) => {
            const d = fromInputDate(e.target.value);
            if (d) setDate(d);
          }}
        />
        <button className="icon-btn boxed" title={t.prevDay} onClick={() => setDate(addDays(date, -1))}>
          <IconChevLeft />
        </button>
        <button className="icon-btn boxed" title={t.nextDay} onClick={() => setDate(addDays(date, 1))}>
          <IconChevRight />
        </button>
        <button className="btn" onClick={() => setDate(startOfDay(new Date()))} disabled={isToday}>
          {t.today}
        </button>
        <span className="toolbar-spacer" />
        <span className="day-stat">
          {t.dayStart} <strong>{fmtHM(dayInfo?.start ?? null) || '–'}</strong>
        </span>
        <span className="day-stat">
          {t.dayEnd} <strong>{fmtHM(dayInfo?.end ?? null) || '–'}</strong>
        </span>
        <span className="day-stat">
          {t.dayLength} <strong>{dayInfo ? fmtDurationHM(dayInfo.end - dayInfo.start) : '–'}</strong>
        </span>
      </div>

      <Timeline
        dayStart={dayStart}
        segments={clipped}
        appNames={appNames}
        checkedApps={checkedApps}
        tags={data.tags}
        tagRanges={data.tagRanges}
        viewStart={view.start}
        viewEnd={view.end}
        fit={fit}
        onView={(s, e) => {
          setFit(false);
          setView({ start: s, end: e });
        }}
        onFit={() => setFit(true)}
        selectedId={selectedId}
        onSelect={pickFromTimeline}
        onTagsChanged={load}
        nowMs={isToday ? nowMs : null}
      />

      <div className="panels">
        <div className="panel list-panel">
          <SegmentList
            rows={rows}
            icons={icons}
            appNames={appNames}
            selectedId={selectedId}
            scrollToken={scrollToken}
            checked={checkedRows}
            onToggle={toggleRow}
            onToggleAll={(on) => setCheckedRows(on ? new Set(rows.map((r) => r.id)) : new Set())}
            onSelect={setSelectedId}
            onHover={onHover}
          />
          {checkedRows.size > 0 && (
            <div className="list-foot">
              {t.selected}: {checkedRows.size} · <strong>{fmtDuration(selectedSum)}</strong>
            </div>
          )}
        </div>
        <div className="panel rank-panel">
          <AppRanking
            rows={ranking}
            total={shownTotal}
            icons={icons}
            checked={checkedApps}
            onToggle={toggleApp}
          />
        </div>
      </div>

      {shown && (
        <ShotPreview
          segment={shown.seg}
          appName={appNames.get(shown.seg.appId ?? -1) ?? ''}
          icon={(shown.seg.domain && icons.sites[shown.seg.domain]) || (shown.seg.appId != null ? icons.apps[shown.seg.appId] : undefined)}
          x={shown.x}
          y={shown.y}
          locked={!!locked}
        />
      )}
      {viewerOpen && (
        <ShotViewer
          dayStart={dayStart}
          dayEnd={dayStart + DAY}
          focus={viewerFocus}
          segments={segById}
          appNames={appNames}
          onClose={() => setViewerOpen(false)}
        />
      )}
    </div>
  );
}
