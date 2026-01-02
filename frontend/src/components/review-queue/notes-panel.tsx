// Notes Panel Component
// Rich notes editor with templates and tags

import { useState, useCallback } from 'react'
import { cn } from '@/lib/utils'
import { useUpdateMatchNotes } from '@/hooks/use-participants'
import { toast } from 'sonner'
import {
  StickyNote,
  Save,
  X,
  Tag,
  Clock,
  Loader2,
  ChevronDown,
  ChevronUp,
  Sparkles,
} from 'lucide-react'

interface NotesPanelProps {
  matchId: string
  initialNotes?: string | null
  onNotesChange?: (notes: string) => void
  compact?: boolean
}

// Quick note templates
const noteTemplates = [
  { id: 'price', label: 'Price mismatch', icon: '💰' },
  { id: 'quantity', label: 'Quantity issue', icon: '📦' },
  { id: 'expired', label: 'Expired product', icon: '⏰' },
  { id: 'different', label: 'Different medication', icon: '💊' },
  { id: 'dosage', label: 'Dosage mismatch', icon: '⚖️' },
  { id: 'verified', label: 'Manually verified', icon: '✅' },
  { id: 'followup', label: 'Needs follow-up', icon: '📋' },
]

// Tag colors
const tagColors: Record<string, string> = {
  price: 'bg-amber-500/20 text-amber-400 border-amber-500/30',
  quantity: 'bg-blue-500/20 text-blue-400 border-blue-500/30',
  expired: 'bg-red-500/20 text-red-400 border-red-500/30',
  different: 'bg-violet-500/20 text-violet-400 border-violet-500/30',
  dosage: 'bg-orange-500/20 text-orange-400 border-orange-500/30',
  verified: 'bg-emerald-500/20 text-emerald-400 border-emerald-500/30',
  followup: 'bg-cyan-500/20 text-cyan-400 border-cyan-500/30',
}

// Extract tags from notes
function extractTags(notes: string): string[] {
  const tagPattern = /#(\w+)/g
  const matches = notes.match(tagPattern)
  return matches ? matches.map((t) => t.slice(1).toLowerCase()) : []
}

export function NotesPanel({
  matchId,
  initialNotes,
  onNotesChange,
  compact = false,
}: NotesPanelProps) {
  const [notes, setNotes] = useState(initialNotes || '')
  const [isEditing, setIsEditing] = useState(false)
  const [expanded, setExpanded] = useState(false)
  const updateNotes = useUpdateMatchNotes()

  const tags = extractTags(notes)
  const hasChanges = notes !== (initialNotes || '')

  const handleSave = useCallback(async () => {
    try {
      await updateNotes.mutateAsync({ id: matchId, notes })
      onNotesChange?.(notes)
      setIsEditing(false)
      toast.success('Notes saved')
    } catch {
      toast.error('Failed to save notes')
    }
  }, [matchId, notes, updateNotes, onNotesChange])

  const handleCancel = useCallback(() => {
    setNotes(initialNotes || '')
    setIsEditing(false)
  }, [initialNotes])

  const addTemplate = useCallback((template: typeof noteTemplates[0]) => {
    setNotes((prev) => {
      const newNote = `${template.icon} ${template.label}`
      return prev ? `${prev}\n${newNote}` : newNote
    })
    setIsEditing(true)
  }, [])

  const addTag = useCallback((tagId: string) => {
    setNotes((prev) => {
      if (prev.includes(`#${tagId}`)) return prev
      return prev ? `${prev} #${tagId}` : `#${tagId}`
    })
    setIsEditing(true)
  }, [])

  if (compact) {
    return (
      <div className="flex items-center gap-2">
        <button
          onClick={() => setExpanded(!expanded)}
          className={cn(
            'flex items-center gap-1.5 px-2 py-1 rounded-lg text-xs transition-colors',
            notes
              ? 'bg-amber-500/20 text-amber-400'
              : 'bg-secondary/50 text-muted-foreground hover:text-foreground',
          )}
        >
          <StickyNote className="w-3 h-3" />
          {notes ? 'Has notes' : 'Add note'}
        </button>

        {/* Tags preview */}
        {tags.length > 0 && (
          <div className="flex items-center gap-1">
            {tags.slice(0, 2).map((tag) => (
              <span
                key={tag}
                className={cn(
                  'px-1.5 py-0.5 rounded text-[10px] font-medium border',
                  tagColors[tag] || 'bg-secondary text-muted-foreground border-border',
                )}
              >
                #{tag}
              </span>
            ))}
            {tags.length > 2 && (
              <span className="text-[10px] text-muted-foreground">
                +{tags.length - 2}
              </span>
            )}
          </div>
        )}
      </div>
    )
  }

  return (
    <div className="space-y-3">
      {/* Header */}
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full flex items-center justify-between p-3 rounded-xl bg-secondary/30 hover:bg-secondary/50 transition-colors"
      >
        <div className="flex items-center gap-2">
          <StickyNote className="w-4 h-4 text-amber" />
          <span className="font-medium text-foreground">Notes & Annotations</span>
          {notes && (
            <span className="px-2 py-0.5 rounded-full bg-amber-500/20 text-amber-400 text-xs">
              {notes.split('\n').filter(Boolean).length} notes
            </span>
          )}
        </div>
        {expanded ? (
          <ChevronUp className="w-4 h-4 text-muted-foreground" />
        ) : (
          <ChevronDown className="w-4 h-4 text-muted-foreground" />
        )}
      </button>

      {/* Expanded content */}
      {expanded && (
        <div className="space-y-3 animate-in slide-in-from-top-2 duration-200">
          {/* Quick templates */}
          <div className="flex flex-wrap gap-1.5">
            {noteTemplates.map((template) => (
              <button
                key={template.id}
                onClick={() => addTemplate(template)}
                className="flex items-center gap-1 px-2 py-1 rounded-lg bg-secondary/50 hover:bg-secondary text-xs text-muted-foreground hover:text-foreground transition-colors"
              >
                <span>{template.icon}</span>
                <span>{template.label}</span>
              </button>
            ))}
          </div>

          {/* Tags */}
          <div className="flex items-center gap-2">
            <Tag className="w-3.5 h-3.5 text-muted-foreground" />
            <div className="flex flex-wrap gap-1">
              {Object.keys(tagColors).map((tagId) => (
                <button
                  key={tagId}
                  onClick={() => addTag(tagId)}
                  className={cn(
                    'px-2 py-0.5 rounded-full text-[10px] font-medium border transition-all',
                    tags.includes(tagId)
                      ? tagColors[tagId]
                      : 'bg-secondary/30 text-muted-foreground border-transparent hover:border-border',
                  )}
                >
                  #{tagId}
                </button>
              ))}
            </div>
          </div>

          {/* Text area */}
          <div className="relative">
            <textarea
              value={notes}
              onChange={(e) => {
                setNotes(e.target.value)
                setIsEditing(true)
              }}
              placeholder="Add notes about this match..."
              className={cn(
                'w-full min-h-[100px] p-3 rounded-xl resize-none',
                'bg-secondary/30 border border-border/50',
                'text-sm text-foreground placeholder:text-muted-foreground',
                'focus:outline-none focus:ring-2 focus:ring-teal/50 focus:border-teal/50',
                'transition-all',
              )}
            />

            {/* Character count */}
            <div className="absolute bottom-2 right-2 text-[10px] text-muted-foreground">
              {notes.length} chars
            </div>
          </div>

          {/* Actions */}
          {isEditing && hasChanges && (
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2 text-xs text-muted-foreground">
                <Clock className="w-3 h-3" />
                <span>Unsaved changes</span>
              </div>
              <div className="flex items-center gap-2">
                <button
                  onClick={handleCancel}
                  className="flex items-center gap-1 px-3 py-1.5 rounded-lg bg-secondary/50 text-muted-foreground hover:text-foreground text-sm transition-colors"
                >
                  <X className="w-3.5 h-3.5" />
                  Cancel
                </button>
                <button
                  onClick={handleSave}
                  disabled={updateNotes.isPending}
                  className="flex items-center gap-1 px-3 py-1.5 rounded-lg bg-teal/20 text-teal hover:bg-teal/30 text-sm font-medium transition-colors disabled:opacity-50"
                >
                  {updateNotes.isPending ? (
                    <Loader2 className="w-3.5 h-3.5 animate-spin" />
                  ) : (
                    <Save className="w-3.5 h-3.5" />
                  )}
                  Save
                </button>
              </div>
            </div>
          )}

          {/* Existing tags display */}
          {tags.length > 0 && (
            <div className="flex items-center gap-2 pt-2 border-t border-border/30">
              <Sparkles className="w-3.5 h-3.5 text-muted-foreground" />
              <span className="text-xs text-muted-foreground">Active tags:</span>
              <div className="flex flex-wrap gap-1">
                {tags.map((tag) => (
                  <span
                    key={tag}
                    className={cn(
                      'px-2 py-0.5 rounded-full text-[10px] font-medium border',
                      tagColors[tag] || 'bg-secondary text-muted-foreground border-border',
                    )}
                  >
                    #{tag}
                  </span>
                ))}
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  )
}
