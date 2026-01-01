import { ArrowUpRight, ArrowDownLeft, Sparkles, Clock } from 'lucide-react'
import { cn } from '@/lib/utils'

const activities = [
  {
    id: 1,
    type: 'match',
    title: 'New Match Found',
    description: 'Augmentin 1g matched with Cairo Pharma',
    time: '2 min ago',
    icon: Sparkles,
  },
  {
    id: 2,
    type: 'offer',
    title: 'New Offer Listed',
    description: 'Panadol Extra - 100 boxes @ 80 EGP',
    time: '15 min ago',
    icon: ArrowUpRight,
  },
  {
    id: 3,
    type: 'request',
    title: 'Request Fulfilled',
    description: 'Metformin 500mg - Order completed',
    time: '32 min ago',
    icon: ArrowDownLeft,
  },
  {
    id: 4,
    type: 'match',
    title: 'High Confidence Match',
    description: '94% match for Amoxicillin 500mg',
    time: '1 hr ago',
    icon: Sparkles,
  },
  {
    id: 5,
    type: 'offer',
    title: 'Price Update',
    description: 'Lipitor 20mg price adjusted',
    time: '2 hr ago',
    icon: ArrowUpRight,
  },
]

export function RecentActivity() {
  return (
    <div className="glass-card p-6 rounded-xl h-full">
      <div className="flex items-center justify-between mb-6">
        <div>
          <h3 className="text-lg font-semibold text-foreground">
            Real-time Activity
          </h3>
          <p className="text-sm text-muted-foreground">Live updates</p>
        </div>
        <div className="flex items-center gap-2">
          <div className="w-2 h-2 rounded-full bg-emerald animate-pulse" />
          <span className="text-xs text-emerald">Live</span>
        </div>
      </div>

      <div className="space-y-4">
        {activities.map((activity, index) => (
          <div
            key={activity.id}
            className={cn(
              'flex items-start gap-3 p-3 rounded-lg transition-all duration-200',
              'hover:bg-secondary/50 cursor-pointer',
              'animate-fade-in',
            )}
            style={{ animationDelay: `${index * 100}ms` }}
          >
            <div
              className={cn(
                'p-2 rounded-lg shrink-0',
                activity.type === 'match' && 'bg-emerald/10 text-emerald',
                activity.type === 'offer' && 'bg-teal/10 text-teal',
                activity.type === 'request' && 'bg-amber/10 text-amber',
              )}
            >
              <activity.icon className="w-4 h-4" />
            </div>
            <div className="flex-1 min-w-0">
              <p className="text-sm font-medium text-foreground truncate">
                {activity.title}
              </p>
              <p className="text-xs text-muted-foreground truncate">
                {activity.description}
              </p>
            </div>
            <div className="flex items-center gap-1 text-xs text-muted-foreground shrink-0">
              <Clock className="w-3 h-3" />
              {activity.time}
            </div>
          </div>
        ))}
      </div>
    </div>
  )
}
