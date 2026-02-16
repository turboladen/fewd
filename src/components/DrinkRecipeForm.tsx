import { useState } from 'react'
import type { DrinkRecipeFormData } from '../types/drinkRecipe'
import { IngredientInput } from './IngredientInput'
import { NumberInput } from './NumberInput'
import { TagInput } from './TagInput'

export function DrinkRecipeForm({
  initialData,
  onSubmit,
  onCancel,
  submitLabel,
}: {
  initialData: DrinkRecipeFormData
  onSubmit: (data: DrinkRecipeFormData) => void
  onCancel: () => void
  submitLabel: string
}) {
  const [form, setForm] = useState<DrinkRecipeFormData>(initialData)
  const [validationError, setValidationError] = useState<string | null>(null)

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    if (form.ingredients.length === 0 || form.ingredients.every((i) => !i.name.trim())) {
      setValidationError('At least 1 ingredient with a name required')
      return
    }
    setValidationError(null)
    onSubmit(form)
  }

  return (
    <form onSubmit={handleSubmit} className='space-y-3'>
      {/* Name + Icon */}
      <div className='grid grid-cols-1 md:grid-cols-2 gap-3'>
        <div>
          <label className='block text-sm font-medium text-stone-700 mb-1'>Name</label>
          <input
            type='text'
            required
            value={form.name}
            onChange={(e) => setForm({ ...form, name: e.target.value })}
            className='input w-full'
            placeholder='e.g. Old Fashioned'
          />
        </div>
        <div>
          <label className='block text-sm font-medium text-stone-700 mb-1'>Icon (emoji)</label>
          <input
            type='text'
            value={form.icon}
            onChange={(e) => setForm({ ...form, icon: e.target.value })}
            placeholder='e.g. 🥃'
            className='input w-24'
          />
        </div>
      </div>

      {/* Description */}
      <div>
        <label className='block text-sm font-medium text-stone-700 mb-1'>Description</label>
        <textarea
          value={form.description}
          onChange={(e) => setForm({ ...form, description: e.target.value })}
          placeholder='Brief description of the drink'
          className='input w-full'
          rows={2}
        />
      </div>

      {/* Servings + Technique + Glassware */}
      <div className='grid grid-cols-1 sm:grid-cols-3 gap-3'>
        <div>
          <label className='block text-sm font-medium text-stone-700 mb-1'>Servings</label>
          <NumberInput
            value={form.servings}
            onChange={(servings) => setForm({ ...form, servings })}
            min={1}
            required
            className='input-sm w-20'
          />
        </div>
        <div>
          <label className='block text-sm font-medium text-stone-700 mb-1'>Technique</label>
          <input
            type='text'
            value={form.technique}
            onChange={(e) => setForm({ ...form, technique: e.target.value })}
            placeholder='Shaken, Stirred, Built...'
            className='input w-full'
          />
        </div>
        <div>
          <label className='block text-sm font-medium text-stone-700 mb-1'>Glassware</label>
          <input
            type='text'
            value={form.glassware}
            onChange={(e) => setForm({ ...form, glassware: e.target.value })}
            placeholder='Coupe, Rocks, Collins...'
            className='input w-full'
          />
        </div>
      </div>

      {/* Ingredients */}
      <IngredientInput
        label='Ingredients'
        value={form.ingredients}
        onChange={(ingredients) => setForm({ ...form, ingredients })}
      />

      {/* Instructions */}
      <div>
        <label className='block text-sm font-medium text-stone-700 mb-1'>Instructions</label>
        <textarea
          required
          value={form.instructions}
          onChange={(e) => setForm({ ...form, instructions: e.target.value })}
          className='input w-full'
          rows={4}
          placeholder='Step-by-step instructions...'
        />
      </div>

      {/* Garnish */}
      <div>
        <label className='block text-sm font-medium text-stone-700 mb-1'>Garnish</label>
        <input
          type='text'
          value={form.garnish}
          onChange={(e) => setForm({ ...form, garnish: e.target.value })}
          placeholder='e.g. Orange peel, Cherry'
          className='input w-full'
        />
      </div>

      {/* Tags */}
      <TagInput
        label='Tags'
        value={form.tags}
        onChange={(tags) => setForm({ ...form, tags })}
      />

      {/* Notes */}
      <div>
        <label className='block text-sm font-medium text-stone-700 mb-1'>Notes</label>
        <textarea
          value={form.notes}
          onChange={(e) => setForm({ ...form, notes: e.target.value })}
          className='input w-full'
          rows={2}
        />
      </div>

      {/* Non-alcoholic */}
      <label className='flex items-center gap-2 cursor-pointer'>
        <input
          type='checkbox'
          checked={form.is_non_alcoholic}
          onChange={(e) => setForm({ ...form, is_non_alcoholic: e.target.checked })}
        />
        <span className='text-sm text-stone-700'>Non-alcoholic</span>
      </label>

      {/* Validation error */}
      {validationError && (
        <div className='panel-error text-red-700 text-sm'>
          {validationError}
        </div>
      )}

      {/* Actions */}
      <div className='flex gap-2'>
        <button type='submit' className='btn-md btn-primary'>
          {submitLabel}
        </button>
        <button type='button' onClick={onCancel} className='btn-md btn-outline'>
          Cancel
        </button>
      </div>
    </form>
  )
}
