interface QueueProgressProps {
  pending: number
  total: number
}

export function QueueProgress({ pending, total }: QueueProgressProps) {
  const reviewed = total - pending
  const progress = total > 0 ? (reviewed / total) * 100 : 0

  return (
    <div className="flex items-center gap-4">
      <div className="flex items-center gap-2">
        <span className="text-sm text-muted-foreground">Queue:</span>
        <span className="text-lg font-bold text-amber">{pending}</span>
        <span className="text-sm text-muted-foreground">pending</span>
      </div>
      <div className="flex-1 h-2 bg-secondary rounded-full overflow-hidden">
        <div
          className="h-full bg-linear-to-r from-teal to-emerald transition-all duration-500"
          style={{ width: `${progress}%` }}
        />
      </div>
      <span className="text-sm text-muted-foreground">{reviewed} reviewed</span>
    </div>
  )
}
