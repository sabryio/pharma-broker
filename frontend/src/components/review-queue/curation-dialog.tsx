import React, { useState, useEffect } from 'react'
import {
  Search,
  Plus,
  Check,
  Loader2,
  Sparkles,
  AlertCircle,
} from 'lucide-react'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Badge } from '@/components/ui/badge'
import {
  getCurationSuggestions,
  approveMedicationAlias,
  type Suggestion,
} from '@/api/curation'
import { toast } from 'sonner'
import { cn } from '@/lib/utils'
import { CreateMasterDialog } from '../medication-curation/create-master-dialog'

interface CurationDialogProps {
  isOpen: boolean
  onClose: () => void
  medicationRaw: string
  aliasId?: string | null
  onSuccess?: () => void
}

export const CurationDialog: React.FC<CurationDialogProps> = ({
  isOpen,
  onClose,
  medicationRaw,
  aliasId,
  onSuccess,
}) => {
  const [suggestions, setSuggestions] = useState<Suggestion[]>([])
  const [isLoading, setIsLoading] = useState(false)
  const [isLinking, setIsLinking] = useState(false)
  const [searchQuery, setSearchQuery] = useState(medicationRaw)
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [isCreateDialogOpen, setIsCreateDialogOpen] = useState(false)

  useEffect(() => {
    if (isOpen && medicationRaw) {
      handleSearch(medicationRaw)
    }
  }, [isOpen, medicationRaw])

  const handleSearch = async (query: string) => {
    setIsLoading(true)
    try {
      const data = await getCurationSuggestions(query)
      setSuggestions(data)
    } catch (err) {
      toast.error('Failed to fetch suggestions')
    } finally {
      setIsLoading(false)
    }
  }

  const handleLink = async (masterId: string) => {
    if (!aliasId) {
      toast.info('Linking simulated', {
        description: `Linked "${medicationRaw}" to master. (No alias ID provided)`,
      })
      onSuccess?.()
      onClose()
      return
    }

    setIsLinking(true)
    try {
      await approveMedicationAlias(aliasId, masterId)
      toast.success('Medication successfully curated', {
        description: `Linked "${medicationRaw}" to master record.`,
      })
      onSuccess?.()
      onClose()
    } catch (err) {
      toast.error('Failed to curate medication')
    } finally {
      setIsLinking(false)
    }
  }

  return (
    <Dialog open={isOpen} onOpenChange={onClose}>
      <DialogContent className="sm:max-max-w-[500px] glass-card border-white/10">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Sparkles className="w-5 h-5 text-teal" />
            Curate Medication
          </DialogTitle>
          <DialogDescription>
            Map the raw name{' '}
            <span className="text-foreground font-mono font-bold">
              "{medicationRaw}"
            </span>{' '}
            to a canonical master record.
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4 py-4">
          <div className="relative">
            <Search className="absolute left-3 top-2.5 h-4 w-4 text-muted-foreground" />
            <Input
              placeholder="Search master medications..."
              className="pl-9 bg-black/20 border-white/5"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && handleSearch(searchQuery)}
            />
          </div>

          <div className="max-h-[300px] overflow-y-auto space-y-2 pr-2 custom-scrollbar">
            {isLoading ? (
              <div className="flex flex-col items-center justify-center py-8 opacity-50">
                <Loader2 className="w-6 h-6 animate-spin text-teal mb-2" />
                <p className="text-xs">Finding matches...</p>
              </div>
            ) : suggestions.length > 0 ? (
              suggestions.map((s) => (
                <button
                  key={s.master.id}
                  onClick={() => setSelectedId(s.master.id)}
                  className={cn(
                    'w-full flex items-center justify-between p-3 rounded-xl border transition-all text-left group',
                    selectedId === s.master.id
                      ? 'bg-teal/10 border-teal/50'
                      : 'bg-white/5 border-white/5 hover:bg-white/10',
                  )}
                >
                  <div className="space-y-1">
                    <p className="text-sm font-semibold flex items-center gap-2">
                      {s.master.name}
                      {s.score > 0.9 && (
                        <Badge className="bg-emerald/20 text-emerald border-none h-4 px-1 text-[9px]">
                          Exact Match
                        </Badge>
                      )}
                    </p>
                    <p className="text-[10px] text-muted-foreground opacity-70">
                      {s.master.activeIngredient || 'No active ingredient info'}{' '}
                      • {s.master.strength || 'N/A'}
                    </p>
                  </div>
                  <div className="flex flex-col items-end gap-1">
                    <span className="text-[9px] font-mono text-muted-foreground">
                      {Math.round(s.score * 100)}%
                    </span>
                    {selectedId === s.master.id && (
                      <Check className="w-4 h-4 text-teal" />
                    )}
                  </div>
                </button>
              ))
            ) : (
              <div className="text-center py-8 opacity-50 flex flex-col items-center">
                <AlertCircle className="w-8 h-8 mb-2" />
                <p className="text-sm italic">No suggestions found</p>
              </div>
            )}
          </div>
        </div>

        <DialogFooter className="gap-2 sm:justify-between">
          <Button
            variant="ghost"
            className="text-xs"
            onClick={() => setIsCreateDialogOpen(true)}
          >
            <Plus className="w-3 h-3 mr-1" /> Create New Master
          </Button>
          <div className="flex gap-2">
            <Button variant="secondary" onClick={onClose}>
              Cancel
            </Button>
            <Button
              disabled={!selectedId || isLinking}
              onClick={() => selectedId && handleLink(selectedId)}
              className="bg-teal hover:bg-teal/80 text-white min-w-[120px]"
            >
              {isLinking ? (
                <>
                  <Loader2 className="w-3 h-3 mr-2 animate-spin" />
                  Linking...
                </>
              ) : (
                'Link & Approve'
              )}
            </Button>
          </div>
        </DialogFooter>
      </DialogContent>

      <CreateMasterDialog
        isOpen={isCreateDialogOpen}
        onClose={() => setIsCreateDialogOpen(false)}
        aliasId={aliasId ?? null}
        aliasName={medicationRaw}
        onSuccess={(master) => {
          handleLink(master.id)
          setIsCreateDialogOpen(false)
        }}
      />
    </Dialog>
  )
}
