import { describe, expect, it } from "vitest";

import { formatCountdown } from "../countdown";

describe("formatCountdown", () => {
  it("returns 'expired' at or past zero", () => {
    expect(formatCountdown(0)).toBe("expired");
    expect(formatCountdown(-5_000)).toBe("expired");
  });

  it("formats sub-minute durations as seconds only", () => {
    expect(formatCountdown(42_000)).toBe("42s");
    expect(formatCountdown(999)).toBe("0s");
    expect(formatCountdown(1_000)).toBe("1s");
    expect(formatCountdown(59_999)).toBe("59s");
  });

  it("formats sub-hour durations as minutes and seconds", () => {
    expect(formatCountdown(4 * 60_000 + 12_000)).toBe("4m 12s");
    expect(formatCountdown(60_000)).toBe("1m 0s");
    expect(formatCountdown(59 * 60_000 + 59_000)).toBe("59m 59s");
  });

  it("formats sub-day durations as hours and minutes", () => {
    expect(formatCountdown(23 * 3_600_000 + 59 * 60_000)).toBe("23h 59m");
    expect(formatCountdown(3_600_000)).toBe("1h 0m");
  });

  it("formats multi-day durations as days and hours", () => {
    expect(formatCountdown(2 * 86_400_000 + 4 * 3_600_000)).toBe("2d 4h");
    expect(formatCountdown(86_400_000)).toBe("1d 0h");
  });

  it("truncates instead of rounding up", () => {
    expect(formatCountdown(59_500)).toBe("59s");
    expect(formatCountdown(3_599_999)).toBe("59m 59s");
  });
});
