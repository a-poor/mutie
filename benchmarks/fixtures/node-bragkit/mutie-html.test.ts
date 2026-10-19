// Test-only additions for bragkit at the commit recorded in node-typescript-results.json.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { htmlWrap } from '../src/reports/pdf.ts';

function body(markdown: string): string {
  const html: string = htmlWrap(markdown);
  const match: RegExpMatchArray | null = html.match(/<body>\n([\s\S]*?)\n<\/body>/);
  assert.ok(match);
  return match[1];
}

test('plain report headings do not close an unopened list', (): void => {
  assert.equal(body('# Title\n\nParagraph'), '<h1>Title</h1>\n<p>Paragraph</p>');
});

test('list items share one container and separated lists reopen', (): void => {
  assert.equal(
    body('- first\n- second\n\nParagraph\n\n- third\n\n## End'),
    '<ul>\n<li>first</li>\n<li>second</li>\n</ul>\n<p>Paragraph</p>\n<ul>\n<li>third</li>\n</ul>\n<h2>End</h2>',
  );
});
