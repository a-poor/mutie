import {test} from 'node:test';
import assert from 'node:assert/strict';
import {impactScore} from '../src/reports/impact.ts';
import type {Achievement} from '../src/achievement.ts';

const pr = (metadata: Record<string, unknown>): Achievement =>
  ({type: 'pr_merged', metadata}) as unknown as Achievement;

test('impact score uses exact uncapped additions and story-point weights', () => {
  assert.equal(impactScore(pr({additions: 100})), 5);
  assert.equal(impactScore(pr({storyPoints: 3})), 10);
});
