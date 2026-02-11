import { useState } from 'react'
import { IconClose } from './Icon'

export function TagInput({
  label,
  value,
  onChange,
  placeholder,
}: {
  label?: string
  value: string[]
  onChange: (value: string[]) => void
  placeholder?: string
}) {
  const [input, setInput] = useState('')

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && input.trim()) {
      e.preventDefault()
      if (!value.includes(input.trim())) {
        onChange([...value, input.trim()])
      }
      setInput('')
    }
    if (e.key === 'Backspace' && !input && value.length > 0) {
      onChange(value.slice(0, -1))
    }
  }

  const handleRemove = (index: number) => {
    onChange(value.filter((_, i) => i !== index))
  }

  return (
    <div>
      {label && (
        <label className='block text-sm font-medium text-stone-700 mb-1'>
          {label}
        </label>
      )}
      <div className='flex flex-wrap gap-1.5 mb-1.5'>
        {value.map((tag, i) => (
          <span
            key={i}
            className='tag'
          >
            {tag}
            <button
              onClick={() => handleRemove(i)}
              className='tag-remove'
              type='button'
            >
              <IconClose className='w-3 h-3' />
            </button>
          </span>
        ))}
      </div>
      <input
        type='text'
        value={input}
        onChange={(e) => setInput(e.target.value)}
        onKeyDown={handleKeyDown}
        placeholder={placeholder
          ?? (label ? `Add ${label.toLowerCase()} (press Enter)` : 'Add item (press Enter)')}
        className='input w-full'
      />
    </div>
  )
}
