import { useEffect, useRef, useState } from "react";
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
  const [hovered, setHovered] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const current = languages.find((option) => option.value === language) ?? languages[0];
  const className = ["language-toggle", menuOpen ? "is-open" : "", hovered ? "is-hovered" : ""]
    .filter(Boolean)
    .join(" ");

  function clearHoverState() {
    setHovered(false);
    if (menuOpen) {
      onMenuOpenChange(false);
    }
  }

  function showHoverState() {
    setHovered(true);
  }

  useEffect(() => {
    function handleDocumentMouseOut(event: MouseEvent) {
      if (event.relatedTarget === null) {
        clearHoverState();
      }
    }

    function handleDocumentMouseMove(event: MouseEvent) {
      const root = rootRef.current;
      if (root && event.target instanceof Node && !root.contains(event.target)) {
        clearHoverState();
      }
    }

    document.addEventListener("mouseout", handleDocumentMouseOut);
    document.addEventListener("mousemove", handleDocumentMouseMove);
    return () => {
      document.removeEventListener("mouseout", handleDocumentMouseOut);
      document.removeEventListener("mousemove", handleDocumentMouseMove);
    };
  }, [menuOpen, onMenuOpenChange]);

  return (
    <div
      ref={rootRef}
      className={className}
      onMouseEnter={showHoverState}
      onMouseMove={showHoverState}
      onMouseLeave={clearHoverState}
      onPointerEnter={showHoverState}
      onPointerMove={showHoverState}
      onPointerLeave={clearHoverState}
    >
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
