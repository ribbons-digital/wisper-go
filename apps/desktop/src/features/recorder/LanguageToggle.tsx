import type { RecognitionLanguage } from "../../types/pipeline";

const GLOBE_ICON_URL = new URL("../../assets/globe_icon_white_transparent.svg", import.meta.url).href;

type LanguageOption = {
  value: RecognitionLanguage;
  label: string;
};

type Props = {
  language: RecognitionLanguage;
  languages: readonly LanguageOption[];
  menuOpen: boolean;
  nativeHovered?: boolean;
  onCycle: () => void;
  onSelect: (language: RecognitionLanguage) => void;
  onMenuOpenChange: (open: boolean) => void;
  onNativeHoverEnd?: () => void;
};

export function LanguageToggle({
  language,
  languages,
  menuOpen,
  nativeHovered = false,
  onCycle,
  onSelect,
  onMenuOpenChange,
  onNativeHoverEnd,
}: Props) {
  const current = languages.find((option) => option.value === language) ?? languages[0];

  function closeMenuOnHoverOff() {
    if (nativeHovered) {
      onNativeHoverEnd?.();
    }
    if (menuOpen) {
      onMenuOpenChange(false);
    }
  }

  const className = [
    "language-toggle",
    menuOpen ? "is-open" : null,
    nativeHovered ? "is-native-hovered" : null,
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <div className={className} onMouseLeave={closeMenuOnHoverOff}>
      {menuOpen ? (
        <div className="language-menu" role="menu" aria-label="Recognition language">
          {languages.map((option) => {
            const selected = option.value === language;
            return (
              <button
                key={option.value}
                type="button"
                role="menuitemradio"
                aria-checked={selected}
                className="language-menu-item"
                onClick={() => onSelect(option.value)}
              >
                <span>{option.label}</span>
                {selected ? <span aria-hidden="true">✓</span> : null}
              </button>
            );
          })}
        </div>
      ) : null}
      <div className="language-toggle-bar">
        <button
          type="button"
          className="language-chevron"
          aria-label="Choose recognition language"
          aria-expanded={menuOpen}
          onClick={() => onMenuOpenChange(!menuOpen)}
        >
          ⌃
        </button>
        <button
          type="button"
          className="language-current"
          aria-label={`Recognition language: ${current.label}`}
          onClick={onCycle}
        >
          {languageIndicator(language)}
        </button>
      </div>
    </div>
  );
}

function languageIndicator(language: RecognitionLanguage) {
  if (language === "auto") {
    return <img className="language-current-icon" src={GLOBE_ICON_URL} alt="" aria-hidden="true" />;
  }
  if (language === "zh") {
    return "ZH/Mix";
  }
  return language.toUpperCase();
}
