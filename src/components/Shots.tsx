import { useEffect, useMemo, useState } from 'react';
import { api, type Segment, type Shot } from '../api';
import { loadShot } from '../hooks';
import { t } from '../i18n';
import { fmtClock } from '../utils/time';
import { Modal } from './Dialogs';
import { IconChevLeft, IconChevRight } from './Icons';

const POPUP_W = 380;

interface PreviewProps {
  segment: Segment;
  appName: string;
  icon?: string;
  x: number;
  y: number;
  locked: boolean;
}

/** Hover popup with the screenshot that belongs to a list row. */
export function ShotPreview({ segment, appName, icon, x, y, locked }: PreviewProps) {
  const [img, setImg] = useState<string | null | undefined>(undefined);

  useEffect(() => {
    let cancel = false;
    setImg(undefined);
    // Jen snímek, který k tomuto záznamu opravdu patří; žádné náhradní snímky.
    api
      .getSegmentShots(segment.id)
      .then(async (shots) => (shots.length ? loadShot(shots[Math.floor(shots.length / 2)].id) : null))
      .then((u) => !cancel && setImg(u ?? null))
      .catch(() => !cancel && setImg(null));
    return () => {
      cancel = true;
    };
  }, [segment.id]);

  const left = Math.max(8, Math.min(x + 16, window.innerWidth - POPUP_W - 8));
  const top = Math.max(8, Math.min(y + 18, window.innerHeight - 330));

  return (
    <div className={`preview ${locked ? 'locked' : ''}`} style={{ left, top, width: POPUP_W }}>
      <div className="preview-head">
        {icon && <img src={icon} alt="" width={14} height={14} />}
        <strong>{appName}</strong>
        {segment.domain && <span className="domain">{segment.domain}</span>}
      </div>
      <div className="preview-title">{segment.title}</div>
      <div className="preview-img">
        {img ? <img src={img} alt="" /> : <div className="preview-empty">{img === undefined ? '…' : t.noShot}</div>}
      </div>
      <div className="preview-foot">{locked ? t.previewLocked : t.previewHint}</div>
    </div>
  );
}

interface ViewerProps {
  dayStart: number;
  dayEnd: number;
  focus: Segment | null;
  segments: Map<number, Segment>;
  appNames: Map<number, string>;
  onClose: () => void;
}

/** Full size viewer that steps through every screenshot of the day. */
export function ShotViewer({ dayStart, dayEnd, focus, segments, appNames, onClose }: ViewerProps) {
  const [shots, setShots] = useState<Shot[] | null>(null);
  const [index, setIndex] = useState(0);
  const [img, setImg] = useState<string | null>(null);

  useEffect(() => {
    api.getRangeShots(dayStart, dayEnd).then((list) => {
      setShots(list);
      if (focus && list.length) {
        const own = list.findIndex((s) => s.segmentId === focus.id);
        if (own >= 0) return setIndex(own);
        let best = 0;
        list.forEach((s, i) => {
          if (Math.abs(s.ts - focus.startMs) < Math.abs(list[best].ts - focus.startMs)) best = i;
        });
        setIndex(best);
      }
    });
  }, [dayStart, dayEnd, focus]);

  const cur = shots?.[index];

  useEffect(() => {
    let cancel = false;
    setImg(null);
    if (cur) loadShot(cur.id).then((u) => !cancel && setImg(u));
    return () => {
      cancel = true;
    };
  }, [cur]);

  const go = useMemo(
    () => (d: number) => setIndex((i) => (shots ? Math.max(0, Math.min(shots.length - 1, i + d)) : 0)),
    [shots],
  );

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'ArrowLeft') go(-1);
      else if (e.key === 'ArrowRight') go(1);
      else if (e.key === 'Home') go(-1e9);
      else if (e.key === 'End') go(1e9);
      else if (e.key === 'F12') {
        e.preventDefault();
        onClose();
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [go, onClose]);

  const seg = cur ? segments.get(cur.segmentId) : undefined;

  return (
    <Modal title={t.viewerTitle} onClose={onClose} wide>
      {shots && shots.length === 0 && <div className="empty">{t.viewerEmpty}</div>}
      {cur && (
        <div className="viewer">
          <div className="viewer-bar">
            <button className="icon-btn" onClick={() => go(-1)} disabled={index === 0}>
              <IconChevLeft />
            </button>
            <span className="viewer-info">
              <strong>{fmtClock(cur.ts)}</strong>
              {seg && <span> · {appNames.get(seg.appId ?? -1)}</span>}
              {seg?.domain && <span> · {seg.domain}</span>}
              <span className="hint"> · {index + 1} / {shots!.length}</span>
            </span>
            <button className="icon-btn" onClick={() => go(1)} disabled={index >= shots!.length - 1}>
              <IconChevRight />
            </button>
          </div>
          {seg && <div className="viewer-title">{seg.title}</div>}
          <div className="viewer-img">{img ? <img src={img} alt="" /> : <div className="preview-empty">…</div>}</div>
        </div>
      )}
    </Modal>
  );
}
