// Card Selector Component
// Quick navigation to specific cards by medication name

import { useState, useMemo, useCallback } from 'react'
import { cn } from '@/lib/utils'
import { ChevronDown, Package, ShoppingCart, Check, X } from 'lucide-react'
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from '@/components/ui/command'
import { ScrollArea } from '@/components/ui/scroll-area'
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover'
import { Button } from '@/components/ui/button'
import type { OfferWithMatches, RequestWithMatches } from './types'

interface CardSelectorProps {
  groupedByOffer: Array<OfferWithMatches>
  groupedByRequest: Array<RequestWithMatches>
  anchorMode: 'offer' | 'request'
  currentAnchorIndex: number
  onNavigate: (anchorIndex: number, relatedIndex?: number) => void
  className?: string
}

interface SelectableCard {
  id: string
  name: string
  type: 'offer' | 'request'
  anchorIndex: number
  matchCount: number
  confidence: number
  source?: string
  // For medication grouping
  itemCount?: number // How many offers/requests share this medication name
  firstIndex?: number // Index of the first item in this medication group
}

export function CardSelector({
  groupedByOffer,
  groupedByRequest,
  anchorMode,
  currentAnchorIndex,
  onNavigate,
  className,
}: CardSelectorProps) {
  const [open, setOpen] = useState(false)
  const [searchValue, setSearchValue] = useState('')

  // Build list of all selectable cards - grouped by medication name
  const cards = useMemo<SelectableCard[]>(() => {
    const result: SelectableCard[] = []
    const medicationMap = new Map<
      string,
      {
        indices: number[]
        totalMatches: number
        totalConfidence: number
        source?: string
      }
    >()

    if (anchorMode === 'offer') {
      // First pass: group by medication name
      groupedByOffer.forEach((group, index) => {
        const medicationName = group.offer.product.trim().toLowerCase()

        if (!medicationMap.has(medicationName)) {
          medicationMap.set(medicationName, {
            indices: [],
            totalMatches: 0,
            totalConfidence: 0,
            source: group.offer.source,
          })
        }

        const medGroup = medicationMap.get(medicationName)!
        medGroup.indices.push(index)
        medGroup.totalMatches += group.matches.length
        medGroup.totalConfidence += group.matches.reduce(
          (sum, m) => sum + m.confidence,
          0,
        )
      })

      // Second pass: create cards from grouped data
      medicationMap.forEach((medGroup) => {
        const firstIndex = medGroup.indices[0]
        if (firstIndex === undefined) return

        const firstGroup = groupedByOffer[firstIndex]
        if (!firstGroup) return

        const avgConfidence =
          medGroup.totalMatches > 0
            ? medGroup.totalConfidence / medGroup.totalMatches
            : 0

        result.push({
          id: firstGroup.offer.id,
          name: firstGroup.offer.product,
          type: 'offer',
          anchorIndex: firstIndex,
          matchCount: medGroup.totalMatches,
          confidence: avgConfidence,
          source: medGroup.source,
          itemCount: medGroup.indices.length,
          firstIndex: firstIndex,
        })
      })
    } else {
      // First pass: group by medication name
      groupedByRequest.forEach((group, index) => {
        const medicationName = group.request.product.trim().toLowerCase()

        if (!medicationMap.has(medicationName)) {
          medicationMap.set(medicationName, {
            indices: [],
            totalMatches: 0,
            totalConfidence: 0,
          })
        }

        const medGroup = medicationMap.get(medicationName)!
        medGroup.indices.push(index)
        medGroup.totalMatches += group.matches.length
        medGroup.totalConfidence += group.matches.reduce(
          (sum, m) => sum + m.confidence,
          0,
        )
      })

      // Second pass: create cards from grouped data
      medicationMap.forEach((medGroup) => {
        const firstIndex = medGroup.indices[0]
        if (firstIndex === undefined) return

        const firstGroup = groupedByRequest[firstIndex]
        if (!firstGroup) return

        const avgConfidence =
          medGroup.totalMatches > 0
            ? medGroup.totalConfidence / medGroup.totalMatches
            : 0

        result.push({
          id: firstGroup.request.id,
          name: firstGroup.request.product,
          type: 'request',
          anchorIndex: firstIndex,
          matchCount: medGroup.totalMatches,
          confidence: avgConfidence,
          itemCount: medGroup.indices.length,
          firstIndex: firstIndex,
        })
      })
    }

    // Sort by anchor index (offer/request number)
    return result.sort((a, b) => a.anchorIndex - b.anchorIndex)
  }, [groupedByOffer, groupedByRequest, anchorMode])

  // Filter cards based on search
  const filteredCards = useMemo(() => {
    if (!searchValue) return cards

    const search = searchValue.toLowerCase()
    return cards.filter((card) => card.name.toLowerCase().includes(search))
  }, [cards, searchValue])

  const currentCard = useMemo(() => {
    // Find the card that contains the current anchor index
    return cards.find((card) => {
      if (anchorMode === 'offer') {
        const medicationName = groupedByOffer[currentAnchorIndex]?.offer.product
          .trim()
          .toLowerCase()
        return card.name.trim().toLowerCase() === medicationName
      } else {
        const medicationName = groupedByRequest[
          currentAnchorIndex
        ]?.request.product
          .trim()
          .toLowerCase()
        return card.name.trim().toLowerCase() === medicationName
      }
    })
  }, [cards, currentAnchorIndex, anchorMode, groupedByOffer, groupedByRequest])

  const handleSelect = useCallback(
    (card: SelectableCard) => {
      onNavigate(card.anchorIndex, 0)
      setOpen(false)
      setSearchValue('')
    },
    [onNavigate],
  )

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button
          variant="outline"
          role="combobox"
          aria-expanded={open}
          className={cn(
            'justify-between gap-2 min-w-[280px] max-w-[400px]',
            'bg-linear-to-r from-secondary/80 to-secondary/40',
            'border-border/50 hover:border-border',
            'transition-all duration-200',
            className,
          )}
        >
          <div className="flex items-center gap-2 min-w-0 flex-1">
            {anchorMode === 'offer' ? (
              <Package className="w-4 h-4 text-teal shrink-0" />
            ) : (
              <ShoppingCart className="w-4 h-4 text-amber shrink-0" />
            )}
            <span className="truncate text-sm font-medium">
              {currentCard?.name || 'Select card...'}
            </span>
          </div>
          <div className="flex items-center gap-2 shrink-0">
            {currentCard && (
              <span className="text-xs text-muted-foreground">
                {currentAnchorIndex + 1}/{cards.length}
              </span>
            )}
            <ChevronDown className="w-4 h-4 text-muted-foreground" />
          </div>
        </Button>
      </PopoverTrigger>
      <PopoverContent className="w-[400px] p-0" align="start" sideOffset={8}>
        <Command shouldFilter={false}>
          <div className="flex items-center border-b border-border/50 px-3">
            <CommandInput
              placeholder={`Search ${anchorMode === 'offer' ? 'offers' : 'requests'}...`}
              value={searchValue}
              onValueChange={setSearchValue}
              className="border-0 focus:ring-0"
            />
          </div>
          <CommandList className="max-h-none overflow-visible">
            <CommandEmpty>
              <div className="py-6 text-center text-sm text-muted-foreground">
                No cards found
              </div>
            </CommandEmpty>
            <ScrollArea className="h-[400px]">
              <div className="p-1">
                <CommandGroup>
                  {filteredCards.map((card) => {
                    // Check if current anchor index belongs to this medication group
                    const currentMedicationName =
                      anchorMode === 'offer'
                        ? groupedByOffer[currentAnchorIndex]?.offer.product
                            .trim()
                            .toLowerCase()
                        : groupedByRequest[currentAnchorIndex]?.request.product
                            .trim()
                            .toLowerCase()

                    const isSelected =
                      card.name.trim().toLowerCase() === currentMedicationName

                    const confidenceColor =
                      card.confidence >= 80
                        ? 'text-emerald-400'
                        : card.confidence >= 50
                          ? 'text-amber-400'
                          : 'text-red-400'

                    return (
                      <CommandItem
                        key={card.id}
                        value={card.id}
                        onSelect={() => handleSelect(card)}
                        className={cn(
                          'flex items-center gap-3 px-3 py-2.5 cursor-pointer',
                          'transition-colors duration-150',
                          isSelected && 'bg-teal/10 border-l-2 border-teal',
                        )}
                      >
                        {/* Icon */}
                        <div
                          className={cn(
                            'w-8 h-8 rounded-lg flex items-center justify-center shrink-0',
                            card.type === 'offer'
                              ? 'bg-teal/20 text-teal'
                              : 'bg-amber/20 text-amber',
                          )}
                        >
                          {card.type === 'offer' ? (
                            <Package className="w-4 h-4" />
                          ) : (
                            <ShoppingCart className="w-4 h-4" />
                          )}
                        </div>

                        {/* Content */}
                        <div className="flex-1 min-w-0">
                          <div className="flex items-center gap-2 mb-0.5">
                            <p className="text-sm font-medium text-foreground truncate">
                              {card.name}
                            </p>
                            {card.itemCount && card.itemCount > 1 && (
                              <span className="shrink-0 px-1.5 py-0.5 rounded-full bg-violet-500/20 text-violet-400 text-[10px] font-bold">
                                ×{card.itemCount}
                              </span>
                            )}
                            {isSelected && (
                              <Check className="w-3.5 h-3.5 text-teal shrink-0" />
                            )}
                          </div>
                          <div className="flex items-center gap-2 text-xs text-muted-foreground">
                            <span>
                              {card.matchCount} match
                              {card.matchCount !== 1 ? 'es' : ''}
                            </span>
                            <span>•</span>
                            <span className={confidenceColor}>
                              {card.confidence.toFixed(0)}% avg
                            </span>
                            {card.source && (
                              <>
                                <span>•</span>
                                <span className="truncate">{card.source}</span>
                              </>
                            )}
                          </div>
                        </div>

                        {/* Index badge */}
                        <div className="shrink-0 px-2 py-0.5 rounded-full bg-secondary text-xs font-medium text-muted-foreground">
                          #{card.anchorIndex + 1}
                        </div>
                      </CommandItem>
                    )
                  })}
                </CommandGroup>
              </div>
            </ScrollArea>
          </CommandList>

          {/* Footer with stats */}
          <div className="border-t border-border/50 px-3 py-2 bg-secondary/30">
            <div className="flex items-center justify-between text-xs text-muted-foreground">
              <span>
                Showing {filteredCards.length} of {cards.length}{' '}
                {anchorMode === 'offer' ? 'offers' : 'requests'}
              </span>
              {searchValue && (
                <button
                  onClick={() => setSearchValue('')}
                  className="flex items-center gap-1 text-teal hover:text-teal/80 transition-colors"
                >
                  <X className="w-3 h-3" />
                  Clear
                </button>
              )}
            </div>
          </div>
        </Command>
      </PopoverContent>
    </Popover>
  )
}
