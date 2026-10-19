export function enabled(): boolean { return true; }
export function fallback(value: string): string | boolean { return value.trim() || false; }
