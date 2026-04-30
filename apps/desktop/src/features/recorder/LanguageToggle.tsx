import type { RecognitionLanguage } from "../../types/pipeline";

type LanguageOption = {
  value: RecognitionLanguage;
  label: string;
};

type Props = {
  language: RecognitionLanguage;
  languages: readonly LanguageOption[];
  menuOpen: boolean;
  onCycle: () => void;
  onSelect: (language: RecognitionLanguage) => void;
  onMenuOpenChange: (open: boolean) => void;
};

export function LanguageToggle({
  language,
  languages,
  menuOpen,
  onCycle,
  onSelect,
  onMenuOpenChange,
}: Props) {
  const current = languages.find((option) => option.value === language) ?? languages[0];

  return (
    <div className={menuOpen ? "language-toggle is-open" : "language-toggle"}>
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
    return "🌐";
  }
  return language.toUpperCase();
}
