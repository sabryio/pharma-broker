import { cn } from '@/lib/utils'
import { Link, useLocation } from '@tanstack/react-router'
import {
  Activity,
  ArrowRightLeft,
  BarChart3,
  Bot,
  Brain,
  Bug,
  ChevronLeft,
  ChevronRight,
  ClipboardCheck,
  FileText,
  LayoutDashboard,
  MessageSquareText,
  Pill,
  Settings,
  Sparkles,
  Users,
} from 'lucide-react'
import { useState } from 'react'

const navItems = [
  { title: 'Dashboard', icon: LayoutDashboard, path: '/' },
  { title: 'Offers', icon: FileText, path: '/offers' },
  { title: 'Requests', icon: ArrowRightLeft, path: '/requests' },
  { title: 'Raw Messages', icon: MessageSquareText, path: '/raw-messages' },
  { title: 'AI Parsing', icon: Brain, path: '/parsing-review' },
  { title: 'Review Queue', icon: ClipboardCheck, path: '/review-queue' },
  { title: 'AI Supervision', icon: Bot, path: '/supervision' },
  { title: 'AI Health', icon: Activity, path: '/ai-health' },
  { title: 'Matches', icon: Sparkles, path: '/matches' },
  { title: 'Groups', icon: Users, path: '/groups' },
  { title: 'Analytics', icon: BarChart3, path: '/analytics' },
  { title: 'Debug', icon: Bug, path: '/debug-recordings' },
  { title: 'Settings', icon: Settings, path: '/settings' },
]

export function AppSidebar() {
  const [collapsed, setCollapsed] = useState(false)
  const location = useLocation()

  return (
    <aside
      className={cn(
        'sticky top-0 flex flex-col h-screen shrink-0 bg-sidebar border-r border-sidebar-border transition-all duration-300',
        collapsed ? 'w-16' : 'w-64',
      )}
    >
      {/* Logo */}
      <div className="flex items-center gap-3 px-4 h-16 border-b border-sidebar-border">
        <div className="flex items-center justify-center w-10 h-10 rounded-lg bg-linear-to-br from-teal to-emerald">
          <Pill className="w-5 h-5 text-primary-foreground" />
        </div>
        {!collapsed && (
          <div className="flex flex-col animate-fade-in">
            <span className="text-lg font-semibold text-foreground">
              Pharma
            </span>
            <span className="text-lg font-semibold gradient-text -mt-1">
              Broker
            </span>
          </div>
        )}
      </div>

      {/* Navigation */}
      <nav className="flex-1 px-2 py-4 space-y-1 overflow-y-auto">
        {navItems.map((item, index) => {
          const isActive = location.pathname === item.path
          return (
            <Link
              key={item.path}
              to={item.path}
              className={cn(
                'group flex items-center gap-3 px-3 py-2.5 rounded-lg transition-all duration-200',
                'hover:bg-sidebar-accent',
                isActive
                  ? 'bg-sidebar-accent text-sidebar-primary border border-sidebar-primary/30 glow-teal'
                  : 'text-sidebar-foreground border border-transparent',
              )}
              style={{ animationDelay: `${index * 50}ms` }}
            >
              <item.icon
                className={cn(
                  'w-5 h-5 shrink-0 transition-colors',
                  isActive
                    ? 'text-teal'
                    : 'text-muted-foreground group-hover:text-foreground',
                )}
              />
              {!collapsed && (
                <span
                  className={cn(
                    'text-sm font-medium transition-colors',
                    isActive
                      ? 'text-foreground'
                      : 'group-hover:text-foreground',
                  )}
                >
                  {item.title}
                </span>
              )}
              {isActive && !collapsed && (
                <div className="ml-auto w-1.5 h-1.5 rounded-full bg-teal animate-pulse" />
              )}
            </Link>
          )
        })}
      </nav>

      {/* Collapse Toggle */}
      <button
        onClick={() => setCollapsed(!collapsed)}
        className="absolute -right-3 top-20 flex items-center justify-center w-6 h-6 rounded-full bg-secondary border border-border text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
      >
        {collapsed ? (
          <ChevronRight className="w-3 h-3" />
        ) : (
          <ChevronLeft className="w-3 h-3" />
        )}
      </button>

      {/* Footer */}
      <div className="p-4 border-t border-sidebar-border">
        <div
          className={cn(
            'flex items-center gap-3',
            collapsed && 'justify-center',
          )}
        >
          <div className="w-8 h-8 rounded-full bg-linear-to-br from-teal/20 to-emerald/20 border border-teal/30 flex items-center justify-center">
            <span className="text-xs font-medium text-teal">PB</span>
          </div>
          {!collapsed && (
            <div className="flex flex-col">
              <span className="text-xs font-medium text-foreground">
                PharmaBroker
              </span>
              <span className="text-xs text-muted-foreground">v1.0.0</span>
            </div>
          )}
        </div>
      </div>
    </aside>
  )
}
