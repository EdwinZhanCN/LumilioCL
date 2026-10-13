import type { CSSProperties } from "react";
import type { StudioText } from "../../i18n/studio";
import { previewVars, type Resolved } from "./model";

type Props = { theme: Resolved; t: StudioText["sample"]; label: string };

/** A launcher page drawn with the site's instrument elements (instrument.css),
 * re-coloured through the custom properties of the edited theme. It shows
 * where each colour, corner and face goes; it is not the launcher itself. */
export function Preview({ theme, t, label }: Props) {
  const style = previewVars(theme) as CSSProperties;
  return (
    <div className="studio-preview" data-background={theme.windowBackground} aria-label={label} role="img">
      <div className="studio-window" style={style}>
        <div className="faceplate studio-face">
          <div className="display studio-face__display">
            <span className="display__legend">{t.played}</span>
            <span className="digits" data-ghost="88.8">
              <span>12.5</span>
            </span>
            <span className="display__legend">{t.hours}</span>
            <div className="meter" aria-hidden="true">
              {[1, 1, 1, 1, 1, 0, 0, 0].map((lit, index) => (
                <i key={index} data-lit={lit === 1} />
              ))}
            </div>
          </div>
          <div className="studio-face__copy">
            <div className="studio-face__title">{t.title}</div>
            <div className="silk">{t.meta}</div>
            <div className="studio-face__state">
              <span className="led" data-lit="true" data-tone="green" />
              <span className="mono">{t.running}</span>
              <span className="led" />
              <span className="mono muted">{t.stopped}</span>
            </div>
          </div>
          <div className="studio-face__keys">
            <span className="key key--orange">{t.play}</span>
            <span className="key">{t.folder}</span>
            <span className="key key--black">{t.more}</span>
          </div>
        </div>

        <div className="studio-tabs">
          {t.tabs.map((tab, index) => (
            <span key={tab} className="studio-tab" data-open={index === 1}>
              {tab}
            </span>
          ))}
        </div>

        <div className="studio-row">
          <div className="segments">
            {t.segments.map((segment, index) => (
              <span key={segment} className="segment">
                <span className="led" data-lit={index === 0} />
                <span className="key key--small" aria-pressed={index === 0}>
                  {segment}
                </span>
              </span>
            ))}
          </div>
          <span className="studio-field">{t.search}</span>
        </div>

        <div className="studio-list">
          {t.rows.map(([name, version], index) => (
            <div key={name} className="studio-list__row">
              <span>{name}</span>
              <span className="mono muted">{version}</span>
              {index === 1 ? <span className="tag tag--orange">{t.update}</span> : <span className="tag">{t.enabled}</span>}
            </div>
          ))}
        </div>

        <p className="studio-danger-text">{t.failed}</p>

        <div className="studio-dialog">
          <div className="studio-dialog__title">{t.dialogTitle}</div>
          <p className="muted">{t.dialogBody}</p>
          <div className="studio-dialog__keys">
            <span className="key key--small">{t.cancel}</span>
            <span className="key key--small studio-danger-key">{t.delete}</span>
          </div>
        </div>

        <div className="studio-toast">
          <span className="led" data-lit="true" data-tone="green" />
          {t.toast}
        </div>
      </div>
    </div>
  );
}
