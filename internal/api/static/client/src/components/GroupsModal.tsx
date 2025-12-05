import { useState } from 'react'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Switch } from '@/components/ui/switch'
import { useGroups, useSyncGroups, useToggleGroup } from '@/lib/api'
import { timeAgo } from '@/lib/sse'

export function GroupsModal() {
  const [open, setOpen] = useState(false)
  const { data: groups, isLoading } = useGroups()
  const syncGroups = useSyncGroups()
  const toggleGroup = useToggleGroup()

  const handleSync = () => {
    syncGroups.mutate()
  }

  const handleToggle = (jid: string, monitored: boolean) => {
    toggleGroup.mutate({ jid, monitored })
  }

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button
          size="icon"
          className="fixed bottom-6 left-6 h-14 w-14 rounded-full shadow-lg z-50"
        >
          📋
        </Button>
      </DialogTrigger>
      <DialogContent className="max-w-lg max-h-[80vh] flex flex-col">
        <DialogHeader>
          <DialogTitle>📋 WhatsApp Groups</DialogTitle>
        </DialogHeader>

        <div className="border-b pb-3 mb-3">
          <Button onClick={handleSync} disabled={syncGroups.isPending}>
            {syncGroups.isPending ? 'Syncing...' : '🔄 Sync from WhatsApp'}
          </Button>
        </div>

        <div className="flex-1 overflow-y-auto space-y-2">
          {isLoading && (
            <p className="text-muted-foreground text-center py-8">Loading...</p>
          )}

          {!isLoading && (!groups || groups.length === 0) && (
            <p className="text-muted-foreground text-center py-8">
              No groups found. Click "Sync" to fetch from WhatsApp.
            </p>
          )}

          {groups?.map((group) => (
            <div
              key={group.jid}
              className="flex items-center justify-between p-3 bg-secondary rounded-lg"
            >
              <div>
                <p className="font-semibold text-sm">{group.name}</p>
                <p className="text-xs text-muted-foreground">
                  {group.message_count || 0} messages · Last:{' '}
                  {group.last_message ? timeAgo(group.last_message) : 'never'}
                </p>
              </div>
              <Switch
                checked={group.monitored}
                onCheckedChange={(checked: boolean) =>
                  handleToggle(group.jid, checked)
                }
                disabled={toggleGroup.isPending}
              />
            </div>
          ))}
        </div>
      </DialogContent>
    </Dialog>
  )
}
