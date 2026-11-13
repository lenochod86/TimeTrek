import { useEffect, useMemo, useRef, useState } from 'react';
import type { Segment } from '../api';
import type { Icons } from '../hooks';
import { t } from '../i18n';
import { fmtClock, fmtDuration } from '../utils/time';
import { IconSortAsc, IconSortDesc } from './Icons';

const ROW_H = 25;

export type SortKey = 'title' | 'start' | 'end' | 'duration';

interface Props {
  rows: Segment[];
  icons: Icons;
  appNames: Map<number, string>;
  selectedId: number | null;
  scrollToken: number;
  checked: Set<number>;
  onToggle: (id: number) => void;
  onToggleAll: (on: boolean) => void;
  onSelect: (id: number) => void;
  onHover: (seg: Segment | null, x: number, y: number) => void;
}

export function SegmentList(p: Props) {
  const [sort, setSort] = useState<{ key: SortKey; dir: 1 | -1 }>({ key: 'start', dir: -1 });
  const [scrollTop, setScrollTop] = useState(0);
  const [height, setHeight] = useState(400);
  const box = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = box.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setHeight(el.clientHeight));
    ro.observe(el);
    setHeight(el.clientHeight);
    return () => ro.disconnect();
  }, []);

  const sorted = useMemo(() => {
    const val = (s: Segment): string | number => {
      switch (sort.key) {
        case 'title':
          return s.title.toLowerCase();
        case 'start':
          return s.startMs;
        case 'end':
          return s.endMs;
        case 'duration':
          return s.endMs - s.startMs;
      }
    };
    return [...p.rows].sort((a, b) => {
      const x = val(a);
      const y = val(b);
      return (x < y ? -1 : x > y ? 1 : 0) * sort.dir;
    });
  }, [p.rows, sort]);

  // Timeline click -> scroll the list to the selected row.
  const lastToken = useRef(p.scrollToken);
  useEffect(() => {
    if (lastToken.current === p.scrollToken) return;
    lastToken.current = p.scrollToken;
    const idx = sorted.findIndex((s) => s.id === p.selectedId);
    if (idx >= 0 && box.current) {
      box.current.scrollTop = Math.max(0, idx * ROW_H - height / 2);
    }
  }, [p.scrollToken, p.selectedId, sorted, height]);

  const first = Math.max(0, Math.floor(scrollTop / ROW_H) - 5);
  const last = Math.min(sorted.length, Math.ceil((scrollTop + height) / ROW_H) + 5);
  const visible = sorted.slice(first, last);
  const allChecked = sorted.length > 0 && sorted.every((s) => p.checked.has(s.id));

  const head = (key: SortKey, label: string, cls: string) => (
    <button className={`th ${cls}`} onClick={() => setSort((s) => (s.key === key ? { key, dir: (-s.dir) as 1 | -1 } : { key, dir: key === 'title' ? 1 : -1 }))}>
      {label}
      {sort.key === key && (sort.dir === 1 ? <IconSortAsc /> : <IconSortDesc />)}
    </button>
  );

  return (
    <div className="list">
      <div className="list-head">
        <span className="cell-check">
          <input type="checkbox" checked={allChecked} onChange={(e) => p.onToggleAll(e.target.checked)} />
        </span>
        <span className="cell-icon" />
        {head('title', t.colName, 'c-title')}
        {head('start', t.colStart, 'c-time')}
        {head('end', t.colStop, 'c-time')}
        {head('duration', t.colDuration, 'c-dur')}
      </div>
      <div className="list-body" ref={box} onScroll={(e) => setScrollTop(e.currentTarget.scrollTop)}>
        {sorted.length === 0 && <div className="empty">{t.noData}</div>}
        <div style={{ height: sorted.length * ROW_H, position: 'relative' }}>
          {visible.map((s, i) => {
            const icon = (s.domain && p.icons.sites[s.domain]) || (s.appId != null ? p.icons.apps[s.appId] : undefined);
            return (
              <div
                key={s.id}
                className={`row ${p.selectedId === s.id ? 'selected' : ''}`}
                style={{ top: (first + i) * ROW_H, height: ROW_H }}
                onClick={() => p.onSelect(s.id)}
                onMouseEnter={(e) => p.onHover(s, e.clientX, e.clientY)}
                onMouseMove={(e) => p.onHover(s, e.clientX, e.clientY)}
                onMouseLeave={() => p.onHover(null, 0, 0)}
              >
                <span className="cell-check" onClick={(e) => e.stopPropagation()}>
                  <input type="checkbox" checked={p.checked.has(s.id)} onChange={() => p.onToggle(s.id)} />
                </span>
                <span className="cell-icon">{icon ? <img src={icon} alt="" width={16} height={16} /> : <span className="icon-ph" />}</span>
                <span className="c-title" title={s.title}>
                  {s.title || p.appNames.get(s.appId ?? -1) || ''}
                </span>
                <span className="c-time">{fmtClock(s.startMs)}</span>
                <span className="c-time">{fmtClock(s.endMs)}</span>
                <span className="c-dur">{fmtDuration(s.endMs - s.startMs)}</span>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
