import { describe, it, expect } from 'vitest';
import { formatUptime, toTitleCase, formatPorts } from './formatters';

describe('formatUptime', () => {
  it('should return "Unknown uptime" for null', () => {
    expect(formatUptime(null)).toBe("Unknown uptime");
  });

  it('should format seconds correctly', () => {
    expect(formatUptime(30)).toBe("30s");
    expect(formatUptime(1)).toBe("1s");
    expect(formatUptime(59)).toBe("59s");
  });

  it('should format minutes correctly', () => {
    expect(formatUptime(60)).toBe("1m");
    expect(formatUptime(90)).toBe("1m"); // The function doesn't show remaining seconds
    expect(formatUptime(3599)).toBe("59m");
  });

  it('should format hours correctly', () => {
    expect(formatUptime(3600)).toBe("1h 0m");
    expect(formatUptime(7200)).toBe("2h 0m");
    expect(formatUptime(3661)).toBe("1h 1m");
    expect(formatUptime(86399)).toBe("23h 59m");
  });

  it('should format days correctly', () => {
    expect(formatUptime(86400)).toBe("1d 0h");
    expect(formatUptime(172800)).toBe("2d 0h");
    expect(formatUptime(90000)).toBe("1d 1h"); // The function doesn't show remaining minutes
  });

  it('should handle edge cases', () => {
    expect(formatUptime(0)).toBe("0s");
    expect(formatUptime(-1)).toBe("-1s"); // Negative values show as seconds
  });
});

describe('toTitleCase', () => {
  it('should convert snake_case to Title Case', () => {
    expect(toTitleCase("hello_world")).toBe("Hello World");
  });

  it('should convert leading underscores to spaces (lowercase)', () => {
    expect(toTitleCase("_private_method")).toBe(" private Method");
  });

  it('should handle camelCase', () => {
    // The regex doesn't handle camelCase properly, it only handles underscore + word boundary
    expect(toTitleCase("helloWorld")).toBe("HelloWorld");
  });

  it('should handle mixed cases', () => {
    expect(toTitleCase("HELLO_WORLD")).toBe("HELLO WORLD");
  });

  it('should handle empty string', () => {
    expect(toTitleCase("")).toBe("");
  });

  it('should handle single word', () => {
    expect(toTitleCase("hello")).toBe("Hello");
  });

  it('should handle already title case', () => {
    expect(toTitleCase("Hello World")).toBe("Hello World");
  });
});

describe('formatPorts', () => {
  it('should return "No ports exposed" for empty array', () => {
    expect(formatPorts([])).toBe("No ports exposed");
  });

  it('should join ports with comma', () => {
    expect(formatPorts(['80:80', '443:443'])).toBe("80:80, 443:443");
  });

  it('should handle single port', () => {
    expect(formatPorts(['8080:80'])).toBe("8080:80");
  });
});
