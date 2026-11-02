import { test } from "vitest";
import assert from "node:assert/strict";
import { isTaskSwitch } from "../src/view";

const s = (title: string, time: string, nextTime = "23:00") => ({
  current: { title, time },
  next: { title: "n", time: nextTime },
});

test("first state is not a switch", () => {
  assert.equal(isTaskSwitch(null, s("A", "12:00")), false);
});

test("same current task is not a switch", () => {
  assert.equal(isTaskSwitch(s("A", "12:00"), s("A", "12:00")), false);
});

test("current task moving forward is a switch", () => {
  assert.equal(isTaskSwitch(s("A", "12:00"), s("B", "13:30")), true);
});

test("tasks with the same title still switch", () => {
  assert.equal(isTaskSwitch(s("Break", "12:00"), s("Break", "15:00")), true);
});

test("editing the next task's time is not a switch", () => {
  assert.equal(isTaskSwitch(s("A", "12:00", "13:30"), s("A", "12:00", "13:45")), false);
});
