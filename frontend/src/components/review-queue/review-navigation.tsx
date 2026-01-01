import { ChevronLeft, ChevronRight } from 'lucide-react'
import { cn } from '@/lib/utils'

interface ReviewNavigationProps {
  currentIndex: number
  total: number
  onPrev: () => void
  onNext: () => void
  onSelect: (index: number) => void
}

export function ReviewNavigation({
  currentIndex,
  total,
  onPrev,
  onNext,
  onSelect,
}: ReviewNavigationProps) {
  return (
    <div className="flex items-center justify-between">
      <button
        onClick={onPrev}
        className="flex items-center gap-2 px-4 py-2 rounded-lg bg-secondary hover:bg-secondary/80 text-foreground transition-all hover-lift"
      >
        <ChevronLeft className="w-4 h-4" /> Previous
      </button>
      <div className="flex items-center gap-2">
        {Array.from({ length: total }).map((_, idx) => (
          <button
            key={idx}
            onClick={() => onSelect(idx)}
            className={cn(
              'w-2.5 h-2.5 rounded-full transition-all duration-300',
              idx === currentIndex
                ? 'bg-teal w-6'
                : 'bg-muted hover:bg-muted-foreground',
            )}
          />
        ))}
      </div>
      <button
        onClick={onNext}
        className="flex items-center gap-2 px-4 py-2 rounded-lg bg-secondary hover:bg-secondary/80 text-foreground transition-all hover-lift"
      >
        Next <ChevronRight className="w-4 h-4" />
      </button>
    </div>
  )
}
