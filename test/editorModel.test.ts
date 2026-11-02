import { test } from "vitest";
import assert from "node:assert/strict";
import { validateRows } from "../src/editorModel";

const ok = { time: null, title: null };

test("valid rows have no errors", () => {
  const v = validateRows([
    { time: "12:00", title: "Lunch" },
    { time: "08:00", title: "Wake up" },
  ]);
  assert.equal(v.ok, true);
  assert.deepEqual(v.rows, [ok, ok]);
  assert.equal(v.form, null);
});

test("bad time formats are flagged per row", () => {
  for (const bad of ["8:00", "24:00", "12:60", "1200", "", " 12:00", "１２:００"]) {
    const v = validateRows([
      { time: bad, title: "a" },
      { time: "10:00", title: "b" },
    ]);
    assert.equal(v.ok, false, bad);
    assert.notEqual(v.rows[0].time, null, bad);
    assert.deepEqual(v.rows[1], ok, bad);
  }
});

test("empty or blank titles are flagged", () => {
  const v = validateRows([
    { time: "09:00", title: "  " },
    { time: "10:00", title: "b" },
  ]);
  assert.equal(v.ok, false);
  assert.notEqual(v.rows[0].title, null);
});

test("duplicate times flag every row that shares the time", () => {
  const v = validateRows([
    { time: "09:00", title: "a" },
    { time: "10:00", title: "b" },
    { time: "09:00", title: "c" },
  ]);
  assert.equal(v.ok, false);
  assert.notEqual(v.rows[0].time, null);
  assert.equal(v.rows[1].time, null);
  assert.notEqual(v.rows[2].time, null);
});

test("fewer than two rows is a form error", () => {
  const v = validateRows([{ time: "09:00", title: "a" }]);
  assert.equal(v.ok, false);
  assert.notEqual(v.form, null);
});

test("blank titles use the same whitespace set as the backend", () => {
  const blank = (title: string) =>
    validateRows([
      { time: "09:00", title },
      { time: "10:00", title: "b" },
    ]).rows[0].title !== null;
  // Rust's char::is_whitespace treats NEL as whitespace but not BOM
  assert.equal(blank("\u0085"), true);
  assert.equal(blank("﻿"), false);
});
