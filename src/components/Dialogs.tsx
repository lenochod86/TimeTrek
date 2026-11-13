import { disable, enable, isEnabled } from '@tauri-apps/plugin-autostart';
import { useEffect, useState, type ReactNode } from 'react';
import { api, type AboutInfo } from '../api';
import { t } from '../i18n';
import { IconClose } from './Icons';

export function Modal({ title, onClose, children, wide }: { title: string; onClose: () => void; children: ReactNode; wide?: boolean }) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === 'Escape' && onClose();
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [onClose]);
  return (
    <div className="modal-backdrop" onMouseDown={onClose}>
      <div className={`modal ${wide ? 'wide' : ''}`} onMouseDown={(e) => e.stopPropagation()}>
        <div className="modal-head">
          <span>{title}</span>
          <button className="icon-btn" onClick={onClose}>
            <IconClose />
          </button>
        </div>
        <div className="modal-body">{children}</div>
      </div>
    </div>
  );
}

function Switch({ checked, onChange, disabled }: { checked: boolean; onChange: (v: boolean) => void; disabled?: boolean }) {
  return (
    <button
      role="switch"
      aria-checked={checked}
      disabled={disabled}
      className={`switch ${checked ? 'on' : ''}`}
      onClick={() => onChange(!checked)}
    >
      <span />
    </button>
  );
}

export function SettingsDialog({ onClose }: { onClose: () => void }) {
  const [autostart, setAutostart] = useState<boolean | null>(null);

  useEffect(() => {
    isEnabled().then(setAutostart).catch(() => setAutostart(false));
  }, []);

  const toggle = async (v: boolean) => {
    setAutostart(v);
    try {
      if (v) await enable();
      else await disable();
    } catch {
      setAutostart(!v);
    }
  };

  return (
    <Modal title={t.settings} onClose={onClose}>
      <div className="settings">
        <ul className="settings-groups">
          <li className="active">{t.settingsGeneral}</li>
        </ul>
        <div className="settings-pane">
          <h3>{t.settingsGeneral}</h3>
          <div className="setting-row">
            <div>
              <div>{t.settingsAutostart}</div>
              <div className="hint">{t.settingsAutostartHint}</div>
            </div>
            <Switch checked={!!autostart} disabled={autostart === null} onChange={toggle} />
          </div>
        </div>
      </div>
    </Modal>
  );
}

export function AboutDialog({ onClose }: { onClose: () => void }) {
  const [info, setInfo] = useState<AboutInfo | null>(null);
  useEffect(() => {
    api.getAppInfo().then(setInfo).catch(() => {});
  }, []);
  return (
    <Modal title={t.about} onClose={onClose}>
      <div className="about">
        <img src="/icon.png" alt="" width={72} height={72} />
        <div>
          <h2>TimeTrek</h2>
          <p>{t.aboutText}</p>
          {info && (
            <>
              <p className="hint">
                {t.aboutVersion}: {info.version}
              </p>
              <p className="hint">
                {t.aboutData}: <code>{info.dataDir}</code>
              </p>
            </>
          )}
        </div>
      </div>
    </Modal>
  );
}
