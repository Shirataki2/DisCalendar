const SCHEME_PATTERN = /^[A-Za-z][A-Za-z0-9+.-]*:(?:$|\S)/;

export function locationUrl(value: string): string | null {
  value = value.trim();
  if (!SCHEME_PATTERN.test(value)) return null;
  try {
    const url = new URL(value);
    return (url.protocol === "http:" || url.protocol === "https:") && url.host
      ? url.href
      : null;
  } catch {
    return null;
  }
}

export function isValidLocation(value: string): boolean {
  value = value.trim();
  return !SCHEME_PATTERN.test(value) || locationUrl(value) !== null;
}
