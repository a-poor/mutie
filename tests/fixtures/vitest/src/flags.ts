export function enabled(): boolean {
  return true;
}

export function fallback(value: boolean): boolean {
  if (value) return true;
  return false;
}
