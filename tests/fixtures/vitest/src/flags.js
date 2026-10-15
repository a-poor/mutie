export function enabled() {
  return true;
}

export function fallback(value) {
  if (value) return true;
  return false;
}
