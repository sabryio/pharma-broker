import { createFileRoute } from '@tanstack/react-router'
import { AlertCircle, Loader2, Plus, Users } from 'lucide-react'
import type { Group } from '@/schema/groups'
import { DashboardLayout } from '@/components/layout/dashboard-layout'
import { Switch } from '@/components/ui/switch'
import { cn } from '@/lib/utils'
import { useGroups, useUpdateGroup } from '@/hooks/use-groups'

export const Route = createFileRoute('/groups')({
  component: Groups,
})

function Groups() {
  const { data, isLoading, error } = useGroups()
  const updateGroup = useUpdateGroup()

  const groups = data?.groups ?? []
  const monitoredCount = groups.filter((g) => g.monitoring).length

  const toggleMonitoring = (group: Group) => {
    console.log('toggleMonitoring clicked', group.jid, group.monitoring)
    updateGroup.mutate({
      jid: group.jid,
      request: { monitoring: !group.monitoring },
    })
  }

  const toggleParsing = (group: Group) => {
    console.log('toggleParsing clicked', group.jid, group.parsing)
    updateGroup.mutate({
      jid: group.jid,
      request: { parsing: !group.parsing },
    })
  }

  if (isLoading) {
    return (
      <DashboardLayout>
        <div className="flex items-center justify-center h-64">
          <Loader2 className="w-8 h-8 animate-spin text-teal" />
        </div>
      </DashboardLayout>
    )
  }

  if (error) {
    return (
      <DashboardLayout>
        <div className="flex flex-col items-center justify-center h-64 gap-4">
          <AlertCircle className="w-12 h-12 text-destructive" />
          <p className="text-muted-foreground">Failed to load groups</p>
          <p className="text-sm text-destructive">{error.message}</p>
        </div>
      </DashboardLayout>
    )
  }

  return (
    <DashboardLayout>
      <div className="space-y-6">
        <div className="flex items-center justify-between">
          <div>
            <h1 className="text-2xl font-bold text-foreground">
              Groups Management
            </h1>
            <p className="text-muted-foreground">
              Monitor and manage trading groups
            </p>
          </div>
          <button className="flex items-center gap-2 px-4 py-2 rounded-lg bg-teal text-primary-foreground font-medium hover:bg-teal/90 transition-colors">
            <Plus className="w-4 h-4" />
            Add Group
          </button>
        </div>

        {/* Stats */}
        <div className="flex gap-4">
          <div className="glass-card p-4 rounded-xl inline-flex items-center gap-2">
            <span className="text-muted-foreground">Total Groups:</span>
            <span className="text-2xl font-bold text-teal">
              {groups.length}
            </span>
          </div>
          <div className="glass-card p-4 rounded-xl inline-flex items-center gap-2">
            <span className="text-muted-foreground">Monitoring Active:</span>
            <span className="text-2xl font-bold text-emerald">
              {monitoredCount}
            </span>
          </div>
        </div>

        {/* Groups Grid */}
        {groups.length === 0 ? (
          <div className="glass-card p-12 rounded-xl text-center">
            <Users className="w-12 h-12 mx-auto mb-4 text-muted-foreground" />
            <p className="text-muted-foreground">No groups found</p>
            <p className="text-sm text-muted-foreground mt-1">
              Groups will appear here when synced from WhatsApp
            </p>
          </div>
        ) : (
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
            {groups.map((group, index) => (
              <div
                key={group.id}
                className={cn(
                  'glass-card p-5 rounded-xl transition-all duration-300',
                  'hover:border-teal/50 animate-fade-in',
                )}
                style={{ animationDelay: `${index * 50}ms` }}
              >
                <div className="mb-4">
                  <span className="text-xs text-muted-foreground">
                    Group Name:
                  </span>
                  <h3 className="text-lg font-semibold text-foreground">
                    {group.name}
                  </h3>
                  {group.description && (
                    <p className="text-sm text-muted-foreground mt-1 line-clamp-2">
                      {group.description}
                    </p>
                  )}
                </div>

                <div className="flex items-center justify-between mb-4">
                  <div>
                    <span className="text-xs text-muted-foreground">
                      Members:
                    </span>
                    <p className="text-xl font-bold text-foreground">
                      {group.member_count.toLocaleString()}
                    </p>
                  </div>
                  <div>
                    <span className="text-xs text-muted-foreground">
                      Messages:
                    </span>
                    <p className="text-xl font-bold text-foreground">
                      {group.message_count.toLocaleString()}
                    </p>
                  </div>
                </div>

                <div className="space-y-3 pt-4 border-t border-border relative z-10">
                  <div className="flex items-center justify-between">
                    <span className="text-sm text-muted-foreground">
                      Monitoring
                    </span>
                    <div className="flex items-center gap-2">
                      <span
                        className={cn(
                          'text-xs font-medium',
                          group.monitoring
                            ? 'text-emerald'
                            : 'text-muted-foreground',
                        )}
                      >
                        {group.monitoring ? 'Active' : 'Inactive'}
                      </span>
                      <Switch
                        checked={group.monitoring}
                        onCheckedChange={() => toggleMonitoring(group)}
                        disabled={updateGroup.isPending}
                        className="data-[state=checked]:bg-emerald"
                      />
                    </div>
                  </div>
                  <div className="flex items-center justify-between">
                    <span className="text-sm text-muted-foreground">
                      Parsing
                    </span>
                    <div className="flex items-center gap-2">
                      <span
                        className={cn(
                          'text-xs font-medium',
                          group.parsing
                            ? 'text-emerald'
                            : 'text-muted-foreground',
                        )}
                      >
                        {group.parsing ? 'Active' : 'Inactive'}
                      </span>
                      <Switch
                        checked={group.parsing}
                        onCheckedChange={() => toggleParsing(group)}
                        disabled={updateGroup.isPending}
                        className="data-[state=checked]:bg-emerald"
                      />
                    </div>
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </DashboardLayout>
  )
}
