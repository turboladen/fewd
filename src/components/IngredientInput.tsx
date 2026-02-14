import type { Ingredient, IngredientAmount } from '../types/recipe'
import { IconClose } from './Icon'

export function IngredientRow({
  ingredient,
  onChange,
  onRemove,
}: {
  ingredient: Ingredient
  onChange: (updated: Ingredient) => void
  onRemove: () => void
}) {
  const isRange = ingredient.amount.type === 'range'

  return (
    <div className='flex flex-wrap gap-2 items-start'>
      <button
        type='button'
        onClick={() => {
          if (isRange) {
            const amt = ingredient.amount as { type: 'range'; min: number; max: number }
            onChange({ ...ingredient, amount: { type: 'single', value: amt.min } })
          } else {
            const amt = ingredient.amount as { type: 'single'; value: number }
            onChange({ ...ingredient, amount: { type: 'range', min: amt.value, max: amt.value } })
          }
        }}
        className={`text-xs px-1.5 py-1 rounded-lg border whitespace-nowrap ${
          isRange
            ? 'bg-primary-50 border-primary-300 text-primary-700'
            : 'bg-white border-stone-300 text-stone-500 hover:border-primary-300'
        }`}
        title={isRange ? 'Switch to exact amount' : 'Switch to range (e.g. 1-2)'}
      >
        {isRange ? 'Range' : 'Exact'}
      </button>
      <div className='flex gap-1 w-20'>
        {isRange
          ? (
            <>
              <input
                type='number'
                step='any'
                value={(ingredient.amount as { type: 'range'; min: number; max: number }).min}
                onChange={(e) =>
                  onChange({
                    ...ingredient,
                    amount: {
                      type: 'range',
                      min: parseFloat(e.target.value) || 0,
                      max: (ingredient.amount as { type: 'range'; min: number; max: number }).max,
                    },
                  })}
                className='input-sm w-9'
                placeholder='min'
              />
              <span className='text-stone-400 self-center'>-</span>
              <input
                type='number'
                step='any'
                value={(ingredient.amount as { type: 'range'; min: number; max: number }).max}
                onChange={(e) =>
                  onChange({
                    ...ingredient,
                    amount: {
                      type: 'range',
                      min: (ingredient.amount as { type: 'range'; min: number; max: number }).min,
                      max: parseFloat(e.target.value) || 0,
                    },
                  })}
                className='input-sm w-9'
                placeholder='max'
              />
            </>
          )
          : (
            <input
              type='number'
              step='any'
              value={(ingredient.amount as { type: 'single'; value: number }).value}
              onChange={(e) =>
                onChange({
                  ...ingredient,
                  amount: { type: 'single', value: parseFloat(e.target.value) || 0 },
                })}
              className='input-sm w-20'
              placeholder='Amt'
            />
          )}
      </div>
      <input
        type='text'
        value={ingredient.unit}
        onChange={(e) => onChange({ ...ingredient, unit: e.target.value })}
        className='input-sm w-20'
        placeholder='Unit'
      />
      <input
        type='text'
        value={ingredient.name}
        onChange={(e) => onChange({ ...ingredient, name: e.target.value })}
        className='input-sm flex-1 min-w-[150px]'
        placeholder='Ingredient name'
      />
      <button
        type='button'
        onClick={onRemove}
        className='btn-sm btn-danger'
      >
        <IconClose className='w-4 h-4' />
      </button>
    </div>
  )
}

export function IngredientInput({
  value,
  onChange,
  label,
}: {
  value: Ingredient[]
  onChange: (value: Ingredient[]) => void
  label?: string
}) {
  const addIngredient = () => {
    onChange([
      ...value,
      {
        name: '',
        amount: { type: 'single', value: 1 } as IngredientAmount,
        unit: '',
        notes: undefined,
      },
    ])
  }

  const updateIngredient = (index: number, updated: Ingredient) => {
    const newList = [...value]
    newList[index] = updated
    onChange(newList)
  }

  const removeIngredient = (index: number) => {
    onChange(value.filter((_, i) => i !== index))
  }

  return (
    <div>
      {label !== undefined && (
        <label className='block text-sm font-medium text-stone-700 mb-1'>
          {label}
        </label>
      )}
      <div className='space-y-2'>
        {value.map((ing, i) => (
          <IngredientRow
            key={i}
            ingredient={ing}
            onChange={(updated) => updateIngredient(i, updated)}
            onRemove={() => removeIngredient(i)}
          />
        ))}
      </div>
      <button
        type='button'
        onClick={addIngredient}
        className='mt-2 text-primary-600 text-sm hover:underline'
      >
        + Add ingredient
      </button>
    </div>
  )
}
