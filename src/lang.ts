const LABELS: Record<string, string> = { en: "EN", zh: "中文" };

export function languageLabel(lang: string): string {
  return LABELS[lang] ?? lang.toUpperCase();
}
