import React from 'react'
import { Check, Sparkles, Loader2, AlertCircle } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { cn } from '@/lib/utils'
import type { Suggestion } from '@/schema/curation'

interface SuggestionPanelProps {
  suggestions: Suggestion[]
  isLoading: boolean
  selectedId: string | null
  onSelect: (id: string) => void
  onApprove: (masterId: string) => void
  onCreateNew: () => void
}

export const SuggestionPanel: React.FC<SuggestionPanelProps> = ({
  suggestions,
  isLoading,
  selectedId,
  onSelect,
  onApprove,
  onCreateNew,
}) => {
  if (isLoading) {
    return (
      <div className="flex flex-col items-center justify-center h-full py-12 opacity-50">
        <Loader2 className="w-8 h-8 animate-spin text-teal mb-4" />
        <p className="text-sm font-medium tracking-wide">
          Finding AI Matches...
        </p>
      </div>
    )
  }

  if (suggestions.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center h-full py-12 text-center">
        <div className="bg-white/5 p-4 rounded-full mb-4">
          <AlertCircle className="w-8 h-8 text-amber-400 opacity-50" />
        </div>
        <h3 className="text-sm font-semibold mb-1">No matches found</h3>
        <p className="text-xs text-muted-foreground mb-6 max-w-[200px]">
          We couldn't find a canonical record for this medication.
        </p>
        <Button
          variant="outline"
          size="sm"
          onClick={onCreateNew}
          className="bg-teal/10 border-teal/20 text-teal hover:bg-teal hover:text-white transition-all"
        >
          Create New Master
        </Button>
      </div>
    )
  }

  return (
    <div className="flex flex-col h-full">
      <div className="flex items-center justify-between mb-4">
        <h3 className="text-xs font-bold uppercase tracking-widest text-muted-foreground flex items-center gap-2">
          <Sparkles className="w-4 h-4 text-teal" /> AI Suggestions
        </h3>
        <Badge variant="outline" className="text-[10px] font-mono opacity-60">
          {suggestions.length} FOUND
        </Badge>
      </div>

      <div className="space-y-3 flex-1 overflow-y-auto pr-2 custom-scrollbar">
        {suggestions.map((s) => (
          <button
            key={s.master.id}
            onClick={() => onSelect(s.master.id)}
            className={cn(
              'w-full flex items-center justify-between p-4 rounded-2xl border transition-all duration-300 text-left group relative overflow-hidden',
              selectedId === s.master.id
                ? 'bg-teal/10 border-teal/40 ring-1 ring-teal/20'
                : 'bg-white/3 border-white/5 hover:bg-white/6 hover:border-white/10',
            )}
          >
            {/* Selection Glow */}
            {selectedId === s.master.id && (
              <div className="absolute top-0 right-0 w-24 h-24 bg-teal/20 blur-3xl rounded-full -mr-12 -mt-12 pointer-events-none" />
            )}

            <div className="space-y-1 z-10">
              <p className="text-sm font-bold flex items-center gap-2">
                {s.master.name}
                {s.score > 0.9 && (
                  <Badge className="bg-emerald/20 text-emerald border-none h-4 px-1.5 text-[9px]">
                    High Confidence
                  </Badge>
                )}
              </p>
              <div className="flex flex-wrap gap-x-3 gap-y-1 text-[10px] text-muted-foreground font-medium opacity-70">
                <span>{s.master.activeIngredient || 'Generic'}</span>
                <span>•</span>
                <span>{s.master.strength || 'N/A'}</span>
                {s.master.manufacturer && (
                  <>
                    <span>•</span>
                    <span>{s.master.manufacturer}</span>
                  </>
                )}
              </div>
            </div>
            <div className="flex flex-col items-end gap-2 z-10">
              <div className="text-[10px] font-mono font-bold text-muted-foreground bg-white/5 px-1.5 py-0.5 rounded">
                {Math.round(s.score * 100)}% Match
              </div>
              {selectedId === s.master.id && (
                <Check className="w-5 h-5 text-teal animate-in zoom-in duration-300" />
              )}
            </div>
          </button>
        ))}
      </div>

      <div className="pt-6 mt-6 border-t border-white/5 space-y-3">
        <Button
          disabled={!selectedId}
          onClick={() => selectedId && onApprove(selectedId)}
          className="w-full bg-teal hover:bg-teal/80 text-white font-bold h-11 rounded-xl shadow-lg shadow-teal/20"
        >
          Approve Mapping
        </Button>
        <Button
          variant="ghost"
          onClick={onCreateNew}
          className="w-full text-xs text-muted-foreground hover:text-foreground transition-colors"
        >
          No match? Create New Master
        </Button>
      </div>
    </div>
  )
}
