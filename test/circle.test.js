import { test } from "node:test";
import assert from "node:assert/strict";
import { wedgePath } from "../src/renderers/circle.js";

test("full fraction is a full circle", () => {
  assert.match(wedgePath(1, 50), /^M 50 0 A 50 50 0 1 1 50 100 A 50 50 0 1 1 50 0 Z$/);
});

test("empty fraction draws nothing", () => {
  assert.equal(wedgePath(0, 50), "");
});

test("half fraction is a half disc with small arc", () => {
  assert.equal(wedgePath(0.5, 50), "M 50 50 L 50 0 A 50 50 0 0 0 50 100 Z");
});

test("fractions are clamped", () => {
  assert.equal(wedgePath(-1, 50), "");
  assert.equal(wedgePath(2, 50), wedgePath(1, 50));
});

test("fractions just below 1 still draw a full circle", () => {
  assert.equal(wedgePath(0.9999999, 50), wedgePath(1, 50));
});

test("fractions just above 0 draw nothing", () => {
  assert.equal(wedgePath(0.0000001, 50), "");
});

test("the moving edge sweeps clockwise as time passes", () => {
  // With 3/4 left, the 12-to-3 o'clock quarter is gone: the edge sits at 3 o'clock.
  assert.equal(wedgePath(0.75, 50), "M 50 50 L 50 0 A 50 50 0 1 0 100 50 Z");
});
