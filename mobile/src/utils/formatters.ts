export const formatUptime = (seconds: number | null): string => {
  if (seconds === null) return "Unknown uptime";
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ${minutes % 60}m`;
  const days = Math.floor(hours / 24);
  return `${days}d ${hours % 24}h`;
};

export const formatPorts = (ports: string[]): string => {
  if (ports.length === 0) return "No ports exposed";
  return ports.join(", ");
};

export const toTitleCase = (value: string): string =>
  value.replace(/(^\w|_\w)/g, (match) => match.replace("_", " ").toUpperCase());
