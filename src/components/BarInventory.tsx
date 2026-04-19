import { useState } from 'react'
import {
  useBarItems,
  useBulkCreateBarItems,
  useClearBarItems,
  useCreateBarItem,
  useDeleteBarItem,
} from '../hooks/useBarItems'
import type { BarItemCategory } from '../types/barItem'
import { BAR_ITEM_CATEGORIES, COMMON_BAR_ITEMS } from '../types/barItem'
import { EmptyState } from './EmptyState'
import { IconClose, IconPlus } from './Icon'
import { useToast } from './Toast'

export function BarInventory() {
  const { data: barItems, isLoading } = useBarItems()
  const createMutation = useCreateBarItem()
  const bulkCreateMutation = useBulkCreateBarItems()
  const deleteMutation = useDeleteBarItem()
  const clearMutation = useClearBarItems()
  const { toast } = useToast()

  const [newName, setNewName] = useState('')
  const [newCategory, setNewCategory] = useState<BarItemCategory>('spirit')
  const [showClear, setShowClear] = useState(false)

  const handleAdd = (e: React.FormEvent) => {
    e.preventDefault()
    if (!newName.trim()) return
    createMutation.mutate(
      { name: newName.trim(), category: newCategory },
      {
        onSuccess: () => {
          setNewName('')
          toast('Added to bar')
        },
      },
    )
  }

  const handleBulkAdd = (category: BarItemCategory, items: string[]) => {
    const existingNames = new Set(
      (barItems ?? [])
        .filter((i) => i.category === category)
        .map((i) => i.name.toLowerCase()),
    )
    const newItems = items
      .filter((name) => !existingNames.has(name.toLowerCase()))
      .map((name) => ({ name, category }))

    if (newItems.length === 0) {
      toast('All items already in your bar')
      return
    }

    bulkCreateMutation.mutate(
      { items: newItems },
      {
        onSuccess: () => {
          toast(`Added ${newItems.length} items`)
        },
      },
    )
  }

  const handleDelete = (id: string) => {
    deleteMutation.mutate(id)
  }

  const handleClear = () => {
    clearMutation.mutate(undefined, {
      onSuccess: () => {
        setShowClear(false)
        toast('Bar cleared')
      },
    })
  }

  // Group items by category
  const grouped = (barItems ?? []).reduce<Record<string, typeof barItems>>((acc, item) => {
    if (!acc[item.category]) acc[item.category] = []
    acc[item.category]!.push(item)
    return acc
  }, {})

  if (isLoading) {
    return <div className='p-6 text-stone-500 animate-pulse'>Loading bar inventory...</div>
  }

  const hasItems = (barItems?.length ?? 0) > 0

  return (
    <div className='p-6 space-y-6'>
      {/* Top row: Add form + Quick Add side by side */}
      <div className='grid grid-cols-1 md:grid-cols-2 gap-4'>
        {/* Add to Bar */}
        <div className='card p-4'>
          <h3 className='font-semibold text-stone-900 mb-3'>Add to Bar</h3>
          <form onSubmit={handleAdd} className='space-y-2'>
            <div className='flex gap-2'>
              <select
                value={newCategory}
                onChange={(e) => setNewCategory(e.target.value as BarItemCategory)}
                className='input-sm w-32'
              >
                {BAR_ITEM_CATEGORIES.map((cat) => (
                  <option key={cat.value} value={cat.value}>{cat.label}</option>
                ))}
              </select>
              <input
                type='text'
                value={newName}
                onChange={(e) => setNewName(e.target.value)}
                placeholder='Item name'
                className='input-sm flex-1'
              />
            </div>
            <button
              type='submit'
              disabled={!newName.trim() || createMutation.isPending}
              className='btn-sm btn-primary w-full'
            >
              <IconPlus className='w-3.5 h-3.5' />
              Add
            </button>
          </form>
        </div>

        {/* Quick Add */}
        <div className='card p-4'>
          <h3 className='font-semibold text-stone-900 mb-3'>Quick Add</h3>
          <div className='flex flex-wrap gap-1.5'>
            {BAR_ITEM_CATEGORIES.map((cat) => {
              const commonItems = COMMON_BAR_ITEMS[cat.value]
              if (!commonItems || commonItems.length === 0) return null
              return (
                <button
                  key={cat.value}
                  onClick={() => handleBulkAdd(cat.value, commonItems)}
                  disabled={bulkCreateMutation.isPending}
                  title={commonItems.join(', ')}
                  className='btn-sm btn-outline text-xs'
                >
                  <IconPlus className='w-3 h-3' />
                  {cat.label}
                </button>
              )
            })}
          </div>
          <p className='text-xs text-stone-400 mt-2'>
            Hover to preview. Already-owned items are skipped.
          </p>
        </div>
      </div>

      {/* Full-width inventory below */}
      {!hasItems
        ? (
          <EmptyState
            emoji='🍸'
            title='Bar is empty'
            description='Add spirits, mixers, and other ingredients to start getting cocktail suggestions.'
          />
        )
        : (
          <div className='space-y-3'>
            <div className='flex items-center justify-between'>
              <h3 className='font-semibold text-stone-900'>
                Your Bar
                <span className='text-sm font-normal text-stone-400 ml-2'>
                  {barItems?.length} items
                </span>
              </h3>
              {showClear
                ? (
                  <span className='flex gap-2 items-center text-sm'>
                    <span className='text-red-600'>Clear all?</span>
                    <button
                      onClick={handleClear}
                      className='text-red-700 font-semibold hover:underline'
                    >
                      Yes
                    </button>
                    <button
                      onClick={() => setShowClear(false)}
                      className='text-stone-500 hover:underline'
                    >
                      No
                    </button>
                  </span>
                )
                : (
                  <button
                    onClick={() => setShowClear(true)}
                    className='btn-xs btn-ghost text-stone-400'
                  >
                    Clear All
                  </button>
                )}
            </div>

            <div className='grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-3'>
              {BAR_ITEM_CATEGORIES.map((cat) => {
                const items = grouped[cat.value]
                if (!items || items.length === 0) return null
                return (
                  <div
                    key={cat.value}
                    className='card p-3 space-y-2'
                  >
                    <div className='flex items-center gap-1.5'>
                      <span className='text-base'>{cat.emoji}</span>
                      <h4 className='text-sm font-semibold text-stone-700'>
                        {cat.label}
                      </h4>
                      <span className='text-xs text-stone-400'>({items.length})</span>
                    </div>
                    <div className='flex flex-wrap gap-1'>
                      {items.map((item) => (
                        <span
                          key={item.id}
                          className={`inline-flex items-center gap-1 text-xs px-2 py-0.5 rounded-full ${cat.tagColor}`}
                        >
                          {item.name}
                          <button
                            onClick={() => handleDelete(item.id)}
                            className='opacity-40 hover:opacity-100 transition-opacity'
                            aria-label={`Remove ${item.name}`}
                          >
                            <IconClose className='w-2.5 h-2.5' />
                          </button>
                        </span>
                      ))}
                    </div>
                  </div>
                )
              })}
            </div>
          </div>
        )}
    </div>
  )
}
