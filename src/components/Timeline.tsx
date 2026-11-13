import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { api, KIND_ACTIVE, type Segment, type Tag, type TagRange } from '../api';
import { t } from '../i18n';
import { TAG_COLORS, colorFor } from '../utils/color';
import { DAY, HOUR, MIN, fmtClock, fmtDuration } from '../utils/time';
import { Modal } from './Dialogs';
import { IconTarget } from './Icons';

const GUTTER = 126;
const LANE_H = 50;
// Pruh (canvas) má navíc 1px ohraničení nahoře a dole; popisek vlevo musí mít stejnou celkovou výšku,
// jinak na sebe linky nenavazují.
const LANE_BORDER = 2;
const AXIS_H = 28;
const OVERVIEW_H = 30;
const USAGE_ACTIVE = '#4c9a1f';
const USAGE_IDLE = '#d4180c';

interface Item {
  id: number;
  start: number;
  end: number;
  color: string;
  label?: string;
  tip: string;
}

interface Props {
  dayStart: number;
  segments: Segment[];
  appNames: Map<number, string>;
  checkedApps: Set<number>;
  tags: Tag[];
  tagRanges: TagRange[];
  viewStart: number;
  viewEnd: number;
  fit: boolean;
  onView: (start: number, end: number) => void;
  onFit: () => void;
  selectedId: number | null;
  onSelect: (id: number) => void;
  onTagsChanged: () => void;
  nowMs: number | null;
}

export function Timeline(p: Props) {
  const areaRef = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(800);
  const [tip, setTip] = useState<{ x: number; y: number; text: string } | null>(null);
  const [pendingTag, setPendingTag] = useState<{ start: number; end: number } | null>(null);
  const dayEnd = p.dayStart + DAY;
  const span = p.viewEnd - p.viewStart;

  useLayoutEffect(() => {
    const el = areaRef.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setWidth(Math.max(100, el.clientWidth)));
    ro.observe(el);
    setWidth(Math.max(100, el.clientWidth));
    return () => ro.disconnect();
  }, []);

  const filtered = p.checkedApps.size > 0;

  const usageItems = useMemo<Item[]>(
    () =>
      p.segments.map((s) => ({
        id: s.id,
        start: s.startMs,
        end: s.endMs,
        color: s.kind === KIND_ACTIVE ? USAGE_ACTIVE : USAGE_IDLE,
        tip: `${s.kind === KIND_ACTIVE ? t.activeLabel : t.idleLabel}  ${fmtClock(s.startMs)} - ${fmtClock(s.endMs)}`,
      })),
    [p.segments],
  );

  const activeSegs = useMemo(
    () => p.segments.filter((s) => s.kind === KIND_ACTIVE && (!filtered || (s.appId != null && p.checkedApps.has(s.appId)))),
    [p.segments, filtered, p.checkedApps],
  );

  const appItems = useMemo<Item[]>(
    () =>
      activeSegs.map((s) => ({
        id: s.id,
        start: s.startMs,
        end: s.endMs,
        color: colorFor(`app:${s.appId}`),
        tip: `${p.appNames.get(s.appId ?? -1) ?? ''}\n${s.title}\n${fmtClock(s.startMs)} - ${fmtClock(s.endMs)} (${fmtDuration(s.endMs - s.startMs)})`,
      })),
    [activeSegs, p.appNames],
  );

  const docItems = useMemo<Item[]>(
    () =>
      activeSegs.map((s) => ({
        id: s.id,
        start: s.startMs,
        end: s.endMs,
        color: colorFor(`doc:${s.appId}:${s.domain ?? ''}:${s.title}`),
        tip: `${s.title}\n${fmtClock(s.startMs)} - ${fmtClock(s.endMs)} (${fmtDuration(s.endMs - s.startMs)})`,
      })),
    [activeSegs],
  );

  const tagItems = useMemo<Item[]>(() => {
    const byId = new Map(p.tags.map((x) => [x.id, x]));
    return p.tagRanges.map((r) => {
      const tag = byId.get(r.tagId);
      return {
        id: r.id,
        start: r.startMs,
        end: r.endMs,
        color: tag?.color ?? '#888',
        label: tag?.name,
        tip: `${tag?.name ?? ''}\n${fmtClock(r.startMs)} - ${fmtClock(r.endMs)}`,
      };
    });
  }, [p.tags, p.tagRanges]);

  // --- zoom / pan helpers -------------------------------------------------
  const clampView = useCallback(
    (s: number, e: number) => {
      const len = Math.min(Math.max(e - s, 2 * MIN), DAY);
      const start = Math.max(p.dayStart, Math.min(s, dayEnd - len));
      p.onView(start, start + len);
    },
    [p, dayEnd],
  );

  const onWheel = (e: React.WheelEvent) => {
    const rect = areaRef.current!.getBoundingClientRect();
    const frac = (e.clientX - rect.left) / rect.width;
    const pivot = p.viewStart + frac * span;
    const factor = e.deltaY > 0 ? 1.25 : 0.8;
    const len = span * factor;
    clampView(pivot - frac * len, pivot - frac * len + len);
  };

  const xToTime = (clientX: number) => {
    const rect = areaRef.current!.getBoundingClientRect();
    return p.viewStart + ((clientX - rect.left) / rect.width) * span;
  };

  // --- time axis ticks ----------------------------------------------------
  const ticks = useMemo(() => {
    const steps = [MIN, 5 * MIN, 10 * MIN, 15 * MIN, 30 * MIN, HOUR, 2 * HOUR, 6 * HOUR];
    const pxPerMs = width / span;
    const step = steps.find((s) => s * pxPerMs >= 64) ?? steps[steps.length - 1];
    const out: { x: number; label: string; major: boolean }[] = [];
    const first = Math.ceil((p.viewStart - p.dayStart) / step) * step + p.dayStart;
    for (let tm = first; tm <= p.viewEnd; tm += step) {
      const d = new Date(tm);
      out.push({
        x: (tm - p.viewStart) * pxPerMs,
        label: `${d.getHours()}:${String(d.getMinutes()).padStart(2, '0')}`,
        major: d.getMinutes() === 0,
      });
    }
    return out;
  }, [p.viewStart, p.viewEnd, p.dayStart, span, width]);

  const nowX = p.nowMs != null && p.nowMs >= p.viewStart && p.nowMs <= p.viewEnd ? ((p.nowMs - p.viewStart) / span) * width : null;

  return (
    <div className="timeline">
      <div className="tl-axis-row" style={{ height: AXIS_H }}>
        <div className="tl-gutter" style={{ width: GUTTER }} />
        <div className="tl-axis" style={{ width }}>
          {ticks.map((k, i) => (
            <span key={i} className={`tl-tick ${k.major ? 'major' : ''}`} style={{ left: k.x }}>
              {k.label}
            </span>
          ))}
        </div>
      </div>

      <div className="tl-lanes" onWheel={onWheel}>
        <div className="tl-labels" style={{ width: GUTTER }}>
          {[t.laneTags, t.laneUsage, t.laneApps, t.laneDocs].map((n) => (
            <div key={n} className="tl-label" style={{ height: LANE_H + LANE_BORDER }}>
              {n}
            </div>
          ))}
        </div>
        <div className="tl-area" ref={areaRef}>
          <Lane
            kind="tags"
            items={tagItems}
            width={width}
            viewStart={p.viewStart}
            viewEnd={p.viewEnd}
            selectedId={null}
            xToTime={xToTime}
            onTip={setTip}
            onRangeSelected={(s, e) => setPendingTag({ start: Math.round(s), end: Math.round(e) })}
            onContext={async (id) => {
              if (window.confirm(t.tagDelete + '?')) {
                await api.deleteTagRange(id);
                p.onTagsChanged();
              }
            }}
          />
          <Lane kind="plain" items={usageItems} width={width} viewStart={p.viewStart} viewEnd={p.viewEnd} selectedId={null} xToTime={xToTime} onTip={setTip} onPan={(a, b) => clampView(a, b)} />
          <Lane kind="plain" items={appItems} width={width} viewStart={p.viewStart} viewEnd={p.viewEnd} selectedId={p.selectedId} xToTime={xToTime} onTip={setTip} onPan={(a, b) => clampView(a, b)} onPick={p.onSelect} />
          <Lane kind="plain" items={docItems} width={width} viewStart={p.viewStart} viewEnd={p.viewEnd} selectedId={p.selectedId} xToTime={xToTime} onTip={setTip} onPan={(a, b) => clampView(a, b)} onPick={p.onSelect} />
          {nowX != null && <div className="tl-now" style={{ left: nowX }} />}
        </div>
      </div>

      <Overview
        dayStart={p.dayStart}
        usage={usageItems}
        gutter={GUTTER}
        fit={p.fit}
        onFit={p.onFit}
        viewStart={p.viewStart}
        viewEnd={p.viewEnd}
        onView={clampView}
        width={width}
      />

      {tip && (
        <div className="tl-tip" style={{ left: Math.min(tip.x + 14, window.innerWidth - 360), top: tip.y + 18 }}>
          {tip.text.split('\n').map((l, i) => (
            <div key={i}>{l}</div>
          ))}
        </div>
      )}

      {pendingTag && (
        <TagDialog
          range={pendingTag}
          existing={p.tags}
          onClose={() => setPendingTag(null)}
          onSaved={() => {
            setPendingTag(null);
            p.onTagsChanged();
          }}
        />
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------

interface LaneProps {
  kind: 'tags' | 'plain';
  items: Item[];
  width: number;
  viewStart: number;
  viewEnd: number;
  selectedId: number | null;
  xToTime: (clientX: number) => number;
  onTip: (v: { x: number; y: number; text: string } | null) => void;
  onPan?: (start: number, end: number) => void;
  onPick?: (id: number) => void;
  onRangeSelected?: (start: number, end: number) => void;
  onContext?: (id: number) => void;
}

function Lane(p: LaneProps) {
  const ref = useRef<HTMLCanvasElement>(null);
  const [drag, setDrag] = useState<{ a: number; b: number } | null>(null);
  const dragRef = useRef<{ x0: number; vs: number; ve: number; t0: number; moved: boolean } | null>(null);
  const span = p.viewEnd - p.viewStart;

  useEffect(() => {
    const c = ref.current;
    if (!c) return;
    const dpr = window.devicePixelRatio || 1;
    c.width = p.width * dpr;
    c.height = LANE_H * dpr;
    const ctx = c.getContext('2d')!;
    ctx.scale(dpr, dpr);
    ctx.clearRect(0, 0, p.width, LANE_H);
    const k = p.width / span;
    ctx.font = '12px "Segoe UI", system-ui, sans-serif';
    ctx.textBaseline = 'middle';
    for (const it of p.items) {
      if (it.end < p.viewStart || it.start > p.viewEnd) continue;
      const x = (it.start - p.viewStart) * k;
      const w = Math.max(1, (it.end - it.start) * k);
      ctx.fillStyle = it.color;
      ctx.fillRect(x, 0, w, LANE_H);
      if (it.label && w > 40) {
        ctx.fillStyle = 'rgba(255,255,255,0.92)';
        ctx.save();
        ctx.beginPath();
        ctx.rect(x, 0, w, LANE_H);
        ctx.clip();
        ctx.fillText(it.label, Math.max(x, 0) + 6, LANE_H / 2);
        ctx.restore();
      }
      if (it.id === p.selectedId) {
        ctx.strokeStyle = '#fff';
        ctx.lineWidth = 2;
        ctx.strokeRect(x + 1, 1, Math.max(w - 2, 1), LANE_H - 2);
      }
    }
    if (drag) {
      const x = (Math.min(drag.a, drag.b) - p.viewStart) * k;
      const w = Math.abs(drag.b - drag.a) * k;
      ctx.fillStyle = 'rgba(80, 220, 160, 0.35)';
      ctx.fillRect(x, 0, w, LANE_H);
      ctx.strokeStyle = '#50dca0';
      ctx.strokeRect(x + 0.5, 0.5, w, LANE_H - 1);
    }
  }, [p.items, p.width, p.viewStart, p.viewEnd, p.selectedId, drag, span]);

  const hit = (tm: number): Item | undefined => {
    for (let i = p.items.length - 1; i >= 0; i--) {
      const it = p.items[i];
      if (tm >= it.start && tm <= it.end) return it;
    }
    // Allow a few pixels of tolerance for very short items.
    const tol = (3 * span) / p.width;
    return p.items.find((it) => tm >= it.start - tol && tm <= it.end + tol);
  };

  const onDown = (e: React.MouseEvent) => {
    if (e.button !== 0) return;
    const tm = p.xToTime(e.clientX);
    if (p.kind === 'tags') {
      dragRef.current = { x0: e.clientX, vs: tm, ve: tm, t0: tm, moved: false };
      setDrag({ a: tm, b: tm });
    } else {
      dragRef.current = { x0: e.clientX, vs: p.viewStart, ve: p.viewEnd, t0: tm, moved: false };
    }
    const move = (ev: MouseEvent) => {
      const d = dragRef.current;
      if (!d) return;
      if (Math.abs(ev.clientX - d.x0) > 3) d.moved = true;
      if (p.kind === 'tags') {
        setDrag({ a: d.t0, b: p.xToTime(ev.clientX) });
      } else if (d.moved && p.onPan) {
        const dt = ((ev.clientX - d.x0) / p.width) * (d.ve - d.vs);
        p.onPan(d.vs - dt, d.ve - dt);
      }
    };
    const up = (ev: MouseEvent) => {
      window.removeEventListener('mousemove', move);
      window.removeEventListener('mouseup', up);
      const d = dragRef.current;
      dragRef.current = null;
      if (!d) return;
      if (p.kind === 'tags') {
        const end = p.xToTime(ev.clientX);
        setDrag(null);
        if (Math.abs(end - d.t0) > 30_000) p.onRangeSelected?.(Math.min(end, d.t0), Math.max(end, d.t0));
      } else if (!d.moved) {
        const it = hit(p.xToTime(ev.clientX));
        if (it) p.onPick?.(it.id);
      }
    };
    window.addEventListener('mousemove', move);
    window.addEventListener('mouseup', up);
  };

  return (
    <canvas
      ref={ref}
      className={`tl-lane ${p.kind}`}
      style={{ width: p.width, height: LANE_H }}
      onMouseDown={onDown}
      onMouseMove={(e) => {
        if (dragRef.current) return p.onTip(null);
        const it = hit(p.xToTime(e.clientX));
        p.onTip(it ? { x: e.clientX, y: e.clientY, text: it.tip } : null);
      }}
      onMouseLeave={() => p.onTip(null)}
      onContextMenu={(e) => {
        e.preventDefault();
        const it = p.kind === 'tags' ? hit(p.xToTime(e.clientX)) : undefined;
        if (it) p.onContext?.(it.id);
      }}
    />
  );
}

// ---------------------------------------------------------------------------

interface OverviewProps {
  dayStart: number;
  usage: Item[];
  gutter: number;
  fit: boolean;
  onFit: () => void;
  viewStart: number;
  viewEnd: number;
  onView: (s: number, e: number) => void;
  width: number;
}

function Overview(p: OverviewProps) {
  const ref = useRef<HTMLDivElement>(null);
  const canvas = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const c = canvas.current;
    if (!c) return;
    const dpr = window.devicePixelRatio || 1;
    c.width = p.width * dpr;
    c.height = OVERVIEW_H * dpr;
    const ctx = c.getContext('2d')!;
    ctx.scale(dpr, dpr);
    ctx.clearRect(0, 0, p.width, OVERVIEW_H);
    const k = p.width / DAY;
    for (const it of p.usage) {
      if (it.color !== USAGE_ACTIVE) continue;
      ctx.fillStyle = 'rgba(76,154,31,0.55)';
      ctx.fillRect((it.start - p.dayStart) * k, OVERVIEW_H - 8, Math.max(1, (it.end - it.start) * k), 8);
    }
  }, [p.usage, p.width, p.dayStart]);

  const toTime = (clientX: number) => {
    const r = ref.current!.getBoundingClientRect();
    return p.dayStart + ((clientX - r.left) / r.width) * DAY;
  };

  const onDown = (e: React.MouseEvent) => {
    const len = p.viewEnd - p.viewStart;
    let anchor = toTime(e.clientX) - (p.viewStart + len / 2);
    const inside = toTime(e.clientX) >= p.viewStart && toTime(e.clientX) <= p.viewEnd;
    if (inside) anchor = toTime(e.clientX) - p.viewStart - len / 2;
    const move = (ev: MouseEvent) => {
      const c = toTime(ev.clientX) - (inside ? anchor : 0);
      p.onView(c - len / 2, c + len / 2);
    };
    move(e.nativeEvent);
    const up = () => {
      window.removeEventListener('mousemove', move);
      window.removeEventListener('mouseup', up);
    };
    window.addEventListener('mousemove', move);
    window.addEventListener('mouseup', up);
  };

  const left = ((p.viewStart - p.dayStart) / DAY) * p.width;
  const w = Math.max(6, ((p.viewEnd - p.viewStart) / DAY) * p.width);
  const hours = Array.from({ length: 12 }, (_, i) => i * 2);

  return (
    <div className="tl-overview-row" style={{ height: OVERVIEW_H }}>
      <div className="tl-gutter" style={{ width: p.gutter }}>
        <button className={`icon-btn ${p.fit ? 'on' : ''}`} title={t.fitView} onClick={p.onFit}>
          <IconTarget />
        </button>
      </div>
      <div className="tl-overview" ref={ref} style={{ width: p.width }} onMouseDown={onDown}>
        <canvas ref={canvas} style={{ width: p.width, height: OVERVIEW_H }} />
        {hours.map((h) => (
          <span key={h} className="ov-tick" style={{ left: (h / 24) * p.width }}>
            {String(h).padStart(2, '0')}
          </span>
        ))}
        <div className="ov-window" style={{ left, width: w }} />
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------

function TagDialog({
  range,
  existing,
  onClose,
  onSaved,
}: {
  range: { start: number; end: number };
  existing: Tag[];
  onClose: () => void;
  onSaved: () => void;
}) {
  const [name, setName] = useState('');
  const [color, setColor] = useState(TAG_COLORS[0]);

  const onName = (v: string) => {
    setName(v);
    const known = existing.find((x) => x.name === v);
    if (known) setColor(known.color);
  };

  const save = async () => {
    if (!name.trim()) return;
    await api.addTagRange(name.trim(), color, range.start, range.end);
    onSaved();
  };

  return (
    <Modal title={t.tagNew} onClose={onClose}>
      <form
        className="tag-form"
        onSubmit={(e) => {
          e.preventDefault();
          save();
        }}
      >
        <div className="hint">
          {fmtClock(range.start)} - {fmtClock(range.end)} ({fmtDuration(range.end - range.start)})
        </div>
        <label>
          {t.tagName}
          <input autoFocus list="tag-names" value={name} onChange={(e) => onName(e.target.value)} />
          <datalist id="tag-names">
            {existing.map((x) => (
              <option key={x.id} value={x.name} />
            ))}
          </datalist>
        </label>
        <div className="swatches">
          {TAG_COLORS.map((c) => (
            <button type="button" key={c} className={`swatch ${c === color ? 'on' : ''}`} style={{ background: c }} onClick={() => setColor(c)} aria-label={c} />
          ))}
        </div>
        <div className="form-actions">
          <button type="button" className="btn" onClick={onClose}>
            {t.cancel}
          </button>
          <button type="submit" className="btn primary" disabled={!name.trim()}>
            {t.save}
          </button>
        </div>
      </form>
    </Modal>
  );
}
