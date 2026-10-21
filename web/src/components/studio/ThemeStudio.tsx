import { useEffect, useMemo, useRef, useState, type ChangeEvent, type CSSProperties, type ReactNode } from "react";
import { studioText, type StudioText } from "../../i18n/studio";
import { COLOR_GROUPS } from "./groups";
import {
  BUILTIN_THEMES,
  MAX_RADIUS,
  checkContrast,
  fileName,
  normalizeColor,
  parseFile,
  problems,
  resolve,
  startFrom,
  writeFile,
  type Fonts,
  type ThemeSpec,
  type Tone,
  type WindowBackground,
} from "./model";
import { Preview } from "./Preview";
import "../../styles/fonts.css";
import "../../styles/instrument.css";
import "./studio.css";

/** The draft, kept as the theme file itself (a per-browser convenience). */
const DRAFT = "lumilio-theme-studio";

type Props = { lang: string; guide: string };

function builtin(tone: Tone): ThemeSpec {
  return BUILTIN_THEMES.find((theme) => theme.appearance === tone)!;
}

/** Edits a launcher theme file and shows each theme on a launcher page. */
export default function ThemeStudio({ lang, guide }: Props) {
  const t = studioText(lang);
  const [themes, setThemes] = useState<ThemeSpec[]>(() => [startFrom(builtin("light"), t.untitled)]);
  const [selected, setSelected] = useState(0);
  const [restored, setRestored] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const picker = useRef<HTMLInputElement>(null);

  // Browser storage may be missing or refuse; the studio works without it.
  useEffect(() => {
    try {
      const saved = localStorage.getItem(DRAFT);
      const parsed = saved ? parseFile(saved) : null;
      const good = parsed?.ok ? parsed.themes.flatMap((theme) => (theme.ok ? [theme.theme] : [])) : [];
      if (good.length) setThemes(good);
    } catch {
      // Nothing to restore.
    }
    setRestored(true);
  }, []);
  useEffect(() => {
    if (!restored) return;
    try {
      localStorage.setItem(DRAFT, writeFile(themes));
    } catch {
      // The draft is only a convenience.
    }
  }, [themes, restored]);

  const index = Math.min(selected, themes.length - 1);
  const theme = themes[index];
  const resolved = useMemo(() => resolve(theme), [theme]);
  const contrast = useMemo(() => checkContrast(resolved), [resolved]);
  const issues = problems(themes);
  const text = writeFile(themes);

  const update = (change: (theme: ThemeSpec) => ThemeSpec) =>
    setThemes((list) => list.map((each, at) => (at === index ? change(each) : each)));
  const setColor = (key: string, value: string) =>
    update((each) => ({ ...each, colors: { ...each.colors, [key]: value } }));
  const setFont = (slot: keyof Fonts, value: string) =>
    update((each) => ({ ...each, fonts: { ...each.fonts, [slot]: value } }));

  function add() {
    // The usual next step is the other appearance under the same name.
    const tone: Tone = theme.appearance === "light" ? "dark" : "light";
    setThemes((list) => [...list, startFrom(builtin(tone), theme.name)]);
    setSelected(themes.length);
  }

  function remove() {
    setThemes((list) => list.filter((_, at) => at !== index));
    setSelected(Math.max(0, index - 1));
  }

  async function load(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file) return;
    const parsed = parseFile(await file.text());
    if (!parsed.ok) return setNotice(`${t.importFailed}: ${parsed.error}`);
    const good = parsed.themes.flatMap((each) => (each.ok ? [each.theme] : []));
    const bad = parsed.themes.flatMap((each) => (each.ok ? [] : [each.error]));
    if (!good.length) return setNotice(`${t.importFailed}: ${bad.join("; ") || "themes: []"}`);
    setThemes(good);
    setSelected(0);
    setNotice(bad.length ? `${t.importSkipped}: ${bad.join("; ")}` : null);
  }

  function download() {
    const url = URL.createObjectURL(new Blob([text], { type: "application/json" }));
    const link = document.createElement("a");
    link.href = url;
    link.download = fileName(themes);
    link.click();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  }

  function copy() {
    navigator.clipboard.writeText(text).then(
      () => {
        setCopied(true);
        setTimeout(() => setCopied(false), 1500);
      },
      () => setNotice(t.copyFailed),
    );
  }

  return (
    <div className="studio not-content">
      <div className="studio__bar">
        <div className="studio__tabs" role="tablist" aria-label={t.themes}>
          {themes.map((each, at) => (
            <button
              key={at}
              type="button"
              role="tab"
              aria-selected={at === index}
              className="studio__chip"
              onClick={() => setSelected(at)}
            >
              <span
                className="studio__swatch"
                style={{ background: resolve(each).colors.page, borderColor: resolve(each).colors.orange }}
              />
              {each.name.trim() || "—"}
              <span className="studio__tone">{t.tones[each.appearance]}</span>
            </button>
          ))}
          <button type="button" className="studio__button" onClick={add}>
            + {t.add}
          </button>
        </div>
        <div className="studio__actions">
          <button type="button" className="studio__button" onClick={() => picker.current?.click()}>
            {t.importFile}
          </button>
          <input ref={picker} type="file" accept=".json,application/json" hidden onChange={load} />
          <button type="button" className="studio__button" onClick={copy}>
            {copied ? t.copied : t.copy}
          </button>
          <button type="button" className="studio__button studio__button--primary" onClick={download}>
            {t.download}
          </button>
        </div>
      </div>
      {notice && (
        <p className="studio__notice" role="status">
          {notice}
        </p>
      )}
      {issues.length > 0 && (
        <ul className="studio__problems">
          {issues.map((issue) => (
            <li key={`${issue.kind}-${issue.index}`}>
              <strong>{themes[issue.index].name.trim() || `#${issue.index + 1}`}</strong> {t.tones[themes[issue.index].appearance]}:{" "}
              {t.problems[issue.kind]}
            </li>
          ))}
        </ul>
      )}

      <div className="studio__body">
        <div className="studio__editor">
          <Section title={t.sections.theme}>
            <Field label={t.name}>
              <input value={theme.name} onChange={(event) => update((each) => ({ ...each, name: event.target.value }))} />
            </Field>
            <Field label={t.appearance}>
              <div className="studio__segments" role="group" aria-label={t.appearance}>
                {(["light", "dark"] as const).map((tone) => (
                  <button
                    key={tone}
                    type="button"
                    aria-pressed={theme.appearance === tone}
                    onClick={() => update((each) => ({ ...each, appearance: tone }))}
                  >
                    {t.tones[tone]}
                  </button>
                ))}
              </div>
            </Field>
            <Field label={t.startFrom} help={t.startHelp}>
              <StartFrom t={t} onApply={(preset) => update((each) => ({ ...startFrom(preset, each.name), source: each.source }))} />
            </Field>
            <Field label={t.source} help={t.sourceHelp}>
              <input
                value={theme.source ?? ""}
                onChange={(event) => update((each) => ({ ...each, source: event.target.value }))}
              />
            </Field>
            {themes.length > 1 && (
              <button type="button" className="studio__button studio__button--quiet" onClick={remove}>
                {t.remove}
              </button>
            )}
          </Section>

          <Section title={t.sections.colours} help={t.colourHelp}>
            {resolved.missing.length > 0 && <p className="studio__hint">{t.missing(resolved.missing.length)}</p>}
            {resolved.solid.length > 0 && <p className="studio__hint">{t.solid(resolved.solid.length)}</p>}
            {COLOR_GROUPS.map((group) => (
              <fieldset key={group.id} className="studio__group">
                <legend>{t.groups[group.id]}</legend>
                {group.keys.map((key) => (
                  <ColorRow
                    key={key}
                    name={key}
                    label={t.colours[key]}
                    value={resolved.colors[key]}
                    inherited={!theme.colors[key]}
                    invalid={t.notColour}
                    onChange={(value) => setColor(key, value)}
                  />
                ))}
              </fieldset>
            ))}
          </Section>

          <Section title={t.sections.shape}>
            <Radius label={t.radius} value={resolved.radius} onChange={(radius) => update((each) => ({ ...each, radius }))} />
            <Radius label={t.radiusLg} value={resolved.radiusLg} onChange={(radius_lg) => update((each) => ({ ...each, radius_lg }))} />
            <p className="studio__hint">{t.radiusHelp}</p>
            {(["sans", "mono", "cjk"] as const).map((slot) => (
              <Field key={slot} label={t.fonts[slot]}>
                <input
                  value={theme.fonts[slot] ?? ""}
                  placeholder={slot === "sans" ? "Space Grotesk" : slot === "mono" ? "JetBrains Mono" : ""}
                  onChange={(event) => setFont(slot, event.target.value)}
                />
              </Field>
            ))}
            <p className="studio__hint">{t.fontHelp}</p>
            <Field label={t.window} help={t.windowHelp}>
              <select
                value={theme.window_background ?? "opaque"}
                onChange={(event) =>
                  update((each) => ({ ...each, window_background: event.target.value as WindowBackground }))
                }
              >
                {(["opaque", "transparent", "blurred"] as const).map((value) => (
                  <option key={value} value={value}>
                    {t.windows[value]}
                  </option>
                ))}
              </select>
            </Field>
          </Section>

          <Section title={t.sections.contrast} help={t.contrastHelp}>
            <p className="studio__hint">
              {t.contrastPassed(contrast.filter((result) => result.ok).length, contrast.length)}
            </p>
            <ul className="studio__contrast">
              {[...contrast]
                .sort((a, b) => Number(a.ok) - Number(b.ok))
                .map(({ rule, ratio, floor, ok }) => (
                  <li key={rule.id} data-ok={ok}>
                    <span aria-hidden="true">{ok ? "✓" : "!"}</span>
                    <span>{t.rules[rule.id]}</span>
                    <span className="studio__ratio">
                      {ratio.toFixed(2)} / {floor}
                    </span>
                  </li>
                ))}
            </ul>
          </Section>
        </div>

        <div className="studio__side">
          <Preview theme={resolved} t={t.sample} label={t.sections.preview} />
          <p className="studio__hint">
            {t.install} <a href={guide}>{t.installLink} →</a>
          </p>
          <details className="studio__json">
            <summary>{t.file}</summary>
            <pre>{text}</pre>
          </details>
          <p className="studio__hint">{t.draftKept}</p>
        </div>
      </div>
    </div>
  );
}

function Section({ title, help, children }: { title: string; help?: string; children: ReactNode }) {
  return (
    <section className="studio__section">
      <h2>{title}</h2>
      {help && <p className="studio__hint">{help}</p>}
      {children}
    </section>
  );
}

function Field({ label, help, children }: { label: string; help?: string; children: ReactNode }) {
  return (
    <label className="studio__field">
      <span>{label}</span>
      {children}
      {help && <small>{help}</small>}
    </label>
  );
}

function StartFrom({ t, onApply }: { t: StudioText; onApply: (theme: ThemeSpec) => void }) {
  const [choice, setChoice] = useState(0);
  return (
    <span className="studio__inline">
      <select value={choice} onChange={(event) => setChoice(Number(event.target.value))}>
        {BUILTIN_THEMES.map((theme, at) => (
          <option key={theme.name + theme.appearance} value={at}>
            {theme.name} · {t.tones[theme.appearance]}
          </option>
        ))}
      </select>
      <button type="button" className="studio__button" onClick={() => onApply(BUILTIN_THEMES[choice])}>
        {t.apply}
      </button>
    </span>
  );
}

function Radius({ label, value, onChange }: { label: string; value: number; onChange: (value: number) => void }) {
  return (
    <Field label={`${label}: ${value}px`}>
      <input
        type="range"
        min={0}
        max={MAX_RADIUS}
        step={1}
        value={value}
        onChange={(event) => onChange(Number(event.target.value))}
      />
    </Field>
  );
}

type ColorRowProps = {
  name: string;
  label: string;
  value: string;
  inherited: boolean;
  invalid: string;
  onChange: (value: string) => void;
};

/** One colour: a picker for the hue and a hex field that also takes opacity. */
function ColorRow({ name, label, value, inherited, invalid, onChange }: ColorRowProps) {
  const [draft, setDraft] = useState(value);
  useEffect(() => setDraft(value), [value]);
  const valid = normalizeColor(draft) !== null;
  const alpha = value.length === 9 ? value.slice(7) : "";
  return (
    <div className="studio__color" data-inherited={inherited}>
      <span className="studio__well" style={{ "--well": value } as CSSProperties}>
        <input
          type="color"
          value={value.slice(0, 7)}
          aria-label={label}
          onChange={(event) => onChange(`${event.target.value}${alpha}`)}
        />
      </span>
      <span className="studio__color-name">
        {label}
        <code>{name}</code>
      </span>
      <input
        className="studio__hex"
        value={draft}
        spellCheck={false}
        aria-label={`${label} (${name})`}
        aria-invalid={!valid}
        title={valid ? undefined : invalid}
        onChange={(event) => {
          setDraft(event.target.value);
          const color = normalizeColor(event.target.value);
          if (color) onChange(color);
        }}
      />
    </div>
  );
}
