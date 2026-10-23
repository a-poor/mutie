import { test } from 'node:test';
import assert from 'node:assert/strict';
import { impactScore, computeMetrics } from '../src/reports/impact.ts';
import type { Achievement } from '../src/achievement.ts';

const achievement = (type: string): Achievement => ({
  id: type, source: 'test', type, title: type, description: '', url: '',
  date: '2026-01-01T00:00:00.000Z', tags: [], metadata: {},
});

test('impact type bonuses apply exactly to the matching achievement', () => {
  assert.equal(impactScore(achievement('issue_closed')), 1);
  assert.equal(impactScore(achievement('kudos_received')), 21);
  assert.equal(impactScore(achievement('blog_created')), 16);
  assert.equal(impactScore(achievement('pr_merged')), 4);
});

test('closed issue count includes only closed issues', () => {
  assert.equal(computeMetrics([achievement('pr_merged')]).issuesClosed, 0);
  assert.equal(computeMetrics([
    achievement('issue_closed'), achievement('issue_closed'),
    achievement('issue_resolved'), achievement('pr_merged'),
  ]).issuesClosed, 2);
});
