import React from 'react'
import { toast } from 'sonner'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { Plus, Loader2, Database, Info } from 'lucide-react'
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
import { Label } from '@/components/ui/label'
import {
  CreateMasterRequestSchema,
  type MedicationMaster,
} from '@/schema/curation'
import { useCreateMaster } from '@/hooks/use-curation'
import { cn } from '@/lib/utils'

/** Convert Arabic-Indic numerals (٠١٢٣٤٥٦٧٨٩) to Western numerals (0123456789) */
const normalizeArabicNumerals = (text: string): string => {
  const arabicIndicMap: Record<string, string> = {
    '٠': '0',
    '١': '1',
    '٢': '2',
    '٣': '3',
    '٤': '4',
    '٥': '5',
    '٦': '6',
    '٧': '7',
    '٨': '8',
    '٩': '9',
  }
  return text.replace(/[٠-٩]/g, (char) => arabicIndicMap[char] || char)
}

interface CreateMasterDialogProps {
  isOpen: boolean
  onClose: () => void
  aliasId: string | null
  aliasName: string | null
  onSuccess?: (master: MedicationMaster) => void
}

export const CreateMasterDialog: React.FC<CreateMasterDialogProps> = ({
  isOpen,
  onClose,
  aliasId,
  aliasName,
  onSuccess,
}) => {
  const { mutate: createMaster, isPending } = useCreateMaster()

  // Capture aliasId when dialog opens to prevent race conditions
  const [capturedAliasId, setCapturedAliasId] = React.useState<string | null>(
    null,
  )

  const isArabic = (text: string) => /[\u0600-\u06FF]/.test(text)

  // Normalize the alias name (convert Arabic-Indic numerals to Western)
  const normalizedAliasName = aliasName
    ? normalizeArabicNumerals(aliasName)
    : null
  const isNameAr = normalizedAliasName ? isArabic(normalizedAliasName) : false

  const {
    register,
    handleSubmit,
    formState: { errors },
    reset,
  } = useForm({
    resolver: zodResolver(CreateMasterRequestSchema),
    defaultValues: {
      name: isNameAr ? '' : normalizedAliasName || '',
      nameAr: isNameAr ? normalizedAliasName || '' : '',
      activeIngredient: '',
      strength: '',
      manufacturer: '',
    },
  })

  // Reset form and capture aliasId when dialog opens
  React.useEffect(() => {
    if (isOpen) {
      // Capture the aliasId when dialog opens
      setCapturedAliasId(aliasId)
      if (normalizedAliasName) {
        reset({
          name: isNameAr ? '' : normalizedAliasName,
          nameAr: isNameAr ? normalizedAliasName : '',
          activeIngredient: '',
          strength: '',
          manufacturer: '',
        })
      }
    }
  }, [isOpen, aliasId, normalizedAliasName, reset])

  const onSubmit = (data: any) => {
    const effectiveAliasId = capturedAliasId || aliasId

    // Allow creation even without aliasId - we'll create an alias using aliasName
    if (!effectiveAliasId && !normalizedAliasName) {
      toast.error('Cannot create master: Missing Alias information')
      return
    }

    createMaster(
      {
        aliasId: effectiveAliasId,
        aliasName: normalizedAliasName || undefined,
        data,
      },
      {
        onSuccess: (response) => {
          toast.success('Master record created successfully')
          onSuccess?.(response.master)
          onClose()
        },
        onError: (error: any) => {
          toast.error(error?.message || 'Failed to create master record')
        },
      },
    )
  }

  const onInvalid = () => {
    toast.error('Please fix the errors in the form')
  }

  return (
    <Dialog open={isOpen} onOpenChange={onClose}>
      <DialogContent className="sm:max-w-[500px] glass-card border-white/10 p-0 overflow-hidden">
        <DialogHeader className="p-6 bg-white/3 border-b border-white/5">
          <div className="flex items-center gap-3 mb-1">
            <div className="bg-teal/20 p-2 rounded-xl">
              <Database className="w-5 h-5 text-teal" />
            </div>
            <DialogTitle className="text-xl font-bold tracking-tight">
              Create Master Record
            </DialogTitle>
          </div>
          <DialogDescription className="text-xs font-medium opacity-60">
            Define a brand new canonical medication record for the alias{' '}
            <span className="text-teal font-mono">"{normalizedAliasName}"</span>
            .
          </DialogDescription>
        </DialogHeader>

        <form
          onSubmit={handleSubmit(onSubmit, onInvalid)}
          className="p-6 space-y-5"
        >
          <div className="grid grid-cols-2 gap-4">
            <div className="space-y-2 col-span-2">
              <Label className="text-xs font-bold uppercase opacity-60">
                Canonical Name (EN)
              </Label>
              <Input
                {...register('name')}
                placeholder="Panadol Advance 500mg"
                className={cn(
                  'bg-white/5 border-white/5 focus:border-teal/50 transition-all font-medium',
                  errors.name && 'border-destructive/50',
                )}
              />
              {errors.name && (
                <p className="text-[10px] text-destructive font-bold">
                  {errors.name.message as string}
                </p>
              )}
            </div>

            <div className="space-y-2">
              <Label className="text-xs font-bold uppercase opacity-60">
                Arabic Name
              </Label>
              <Input
                {...register('nameAr')}
                placeholder="بنادول ادفانس ٥٠٠ مجم"
                className="bg-white/5 border-white/5 focus:border-teal/50 text-right font-medium"
                dir="rtl"
              />
            </div>

            <div className="space-y-2">
              <Label className="text-xs font-bold uppercase opacity-60">
                Strength
              </Label>
              <Input
                {...register('strength')}
                placeholder="500mg"
                className="bg-white/5 border-white/5 focus:border-teal/50 font-medium"
              />
            </div>

            <div className="space-y-2">
              <Label className="text-xs font-bold uppercase opacity-60">
                Active Ingredient
              </Label>
              <Input
                {...register('activeIngredient')}
                placeholder="Paracetamol"
                className="bg-white/5 border-white/5 focus:border-teal/50 font-medium"
              />
            </div>

            <div className="space-y-2">
              <Label className="text-xs font-bold uppercase opacity-60">
                Manufacturer
              </Label>
              <Input
                {...register('manufacturer')}
                placeholder="GSK"
                className="bg-white/5 border-white/5 focus:border-teal/50 font-medium"
              />
            </div>
          </div>

          <div className="bg-amber-400/5 border border-amber-400/10 p-3 rounded-xl flex gap-3 text-amber-400">
            <Info className="w-5 h-5 shrink-0 mt-0.5" />
            <p className="text-[10px] leading-relaxed font-medium">
              Creating a new master medication should only be done if no
              existing canonical record matches. This change will affect all
              future parsing logic.
            </p>
          </div>

          <DialogFooter className="gap-2 border-t border-white/5 pt-5 -mx-6 px-6 bg-white/1">
            <Button
              type="button"
              variant="secondary"
              onClick={onClose}
              className="bg-white/5 border-white/5 hover:bg-white/10"
            >
              Cancel
            </Button>
            <Button
              type="submit"
              className="bg-teal hover:bg-teal/80 text-white min-w-[140px]"
              disabled={isPending}
            >
              {isPending ? (
                <>
                  <Loader2 className="w-3 h-3 mr-2 animate-spin" />
                  Creating...
                </>
              ) : (
                <>
                  <Plus className="w-3 h-3 mr-2" />
                  Create Master
                </>
              )}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
