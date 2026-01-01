import type { ReactNode } from 'react'
import { AppSidebar } from './app-sidebar'
import { Bell, Search, User } from 'lucide-react'
import { ScrollArea } from '@/components/ui/scroll-area'

interface DashboardLayoutProps {
  children: ReactNode
}

export function DashboardLayout({ children }: DashboardLayoutProps) {
  return (
    <div className="flex h-screen w-full overflow-hidden bg-background network-bg">
      {/* Fixed Sidebar - uses h-screen internally, stays fixed */}
      <AppSidebar />

      {/* Main content area - flex-1 with min-h-0 enables proper overflow */}
      <div className="flex flex-1 flex-col min-h-0 min-w-0">
        {/* Fixed Header */}
        <DashboardHeader />

        {/* Scrollable content area with custom ScrollArea */}
        <ScrollArea className="flex-1 min-h-0">
          <main className="p-6">{children}</main>
        </ScrollArea>
      </div>
    </div>
  )
}

interface HeaderAction {
  id: string
  icon: React.ComponentType<{ className?: string }>
  hasBadge?: boolean
  onClick?: () => void
}

function DashboardHeader() {
  const headerActions: HeaderAction[] = [
    {
      id: 'notifications',
      icon: Bell,
      hasBadge: true,
    },
  ]

  const userProfile: UserProfile = {
    name: 'Admin User',
    region: 'Cairo Region',
    icon: User,
  }

  return (
    <header className="shrink-0 h-16 border-b border-border bg-background/80 backdrop-blur-sm flex items-center justify-between px-6 sticky top-0 z-10">
      <SearchBox />

      <div className="flex items-center gap-4">
        {headerActions.map((action) => (
          <HeaderActionButton key={action.id} {...action} />
        ))}

        <UserBlock {...userProfile} />
      </div>
    </header>
  )
}

function SearchBox() {
  return (
    <div className="relative">
      <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
      <input
        type="text"
        placeholder="Search medications, offers, requests..."
        className="w-80 h-10 pl-10 pr-4 rounded-lg bg-secondary/50 border border-border text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-teal/30 focus:border-teal/50 transition-all"
      />
    </div>
  )
}

function HeaderActionButton({
  icon: Icon,
  hasBadge,
  onClick,
}: {
  icon: React.ComponentType<{ className?: string }>
  hasBadge?: boolean
  onClick?: () => void
}) {
  return (
    <button
      onClick={onClick}
      className="relative p-2 rounded-lg hover:bg-secondary/50 transition-colors"
    >
      <Icon className="w-5 h-5 text-muted-foreground" />
      {hasBadge && (
        <span className="absolute top-1 right-1 w-2 h-2 rounded-full bg-amber" />
      )}
    </button>
  )
}

interface UserProfile {
  name: string
  region: string
  icon: React.ComponentType<{ className?: string }>
}

function UserBlock({ name, region, icon: Icon }: UserProfile) {
  return (
    <div className="flex items-center gap-3 pl-4 border-l border-border">
      <div className="text-right">
        <p className="text-sm font-medium text-foreground">{name}</p>
        <p className="text-xs text-muted-foreground">{region}</p>
      </div>

      <div className="w-9 h-9 rounded-full bg-linear-to-br from-teal/20 to-emerald/20 border border-teal/30 flex items-center justify-center">
        <Icon className="w-4 h-4 text-teal" />
      </div>
    </div>
  )
}
