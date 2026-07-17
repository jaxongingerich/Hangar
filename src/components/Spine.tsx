interface Props {
  /** Edits per day, oldest→today, 14 entries. */
  data: number[];
  color: string;
}

/** The activity "spine": 14 days of edit ticks — a project's heartbeat. */
export function Spine({ data, color }: Props) {
  const max = Math.max(1, ...data);
  return (
    <div
      className="flex flex-col-reverse gap-[3px] items-center shrink-0"
      title="Activity, last 14 days"
    >
      {data.map((count, i) => (
        <div
          key={i}
          className="rounded-full"
          style={{
            width: 6,
            height: 2,
            background: count > 0 ? color : "var(--color-line)",
            opacity: count > 0 ? 0.35 + 0.65 * (count / max) : 1,
          }}
        />
      ))}
    </div>
  );
}
