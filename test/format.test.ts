import { test } from "vitest";
import assert from "node:assert/strict";
import { formatRemaining } from "../src/format";

test("formatRemaining shows minutes under an hour", () => {
  assert.equal(formatRemaining(42 * 60), "42 min");
});

test("formatRemaining rounds partial minutes up", () => {
  assert.equal(formatRemaining(41 * 60 + 1), "42 min");
  assert.equal(formatRemaining(1), "1 min");
});

test("formatRemaining shows hours with zero-padded minutes", () => {
  assert.equal(formatRemaining(60 * 60), "1 h 00 min");
  assert.equal(formatRemaining(65 * 60), "1 h 05 min");
});
