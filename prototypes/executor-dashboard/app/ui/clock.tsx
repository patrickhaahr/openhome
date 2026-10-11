import { useEffect, useState } from "react";

const timeFormat = new Intl.DateTimeFormat("en-GB", {
  hour: "2-digit",
  minute: "2-digit",
  hour12: false,
});

/** One device-local clock shared by Home and the persistent sidebar. */
export function useCurrentTime(): Date {
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    function refresh(): void {
      window.clearTimeout(timer);
      setNow(new Date());
      timer = window.setTimeout(refresh, 60_000 - (Date.now() % 60_000));
    }
    let timer = window.setTimeout(refresh, 60_000 - (Date.now() % 60_000));
    document.addEventListener("visibilitychange", refresh);
    return () => {
      window.clearTimeout(timer);
      document.removeEventListener("visibilitychange", refresh);
    };
  }, []);
  return now;
}

export function Clock({
  now,
  className,
}: {
  readonly now: Date;
  readonly className: string;
}): React.JSX.Element {
  return (
    <time className={className} dateTime={now.toISOString()} aria-label="Current time">
      {timeFormat.format(now)}
    </time>
  );
}
