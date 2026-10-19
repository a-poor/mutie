import type { Flag } from './types.ts';
import { identity } from './identity.js';
export function enabled(): true { return true }
export function disabled(): false { return false }
export function fallback<T extends Flag>(value: T): boolean { return identity(value) || true satisfies boolean }
