import { useState } from 'react'

interface NumberInputProps {
  value: number
  onChange: (value: number) => void
  min?: number
  step?: number | string
  className?: string
  required?: boolean
  title?: string
  'aria-label'?: string
}

export function NumberInput({
  value,
  onChange,
  min = 1,
  step,
  className = 'input-sm w-20',
  required,
  title,
  'aria-label': ariaLabel,
}: NumberInputProps) {
  // `raw` is a string buffer that allows in-progress edits ("1.", ""); resync it when
  // the `value` prop changes externally via React's adjust-state-during-render pattern.
  const [raw, setRaw] = useState(String(value))
  const [lastValue, setLastValue] = useState(value)
  if (value !== lastValue) {
    setLastValue(value)
    setRaw(String(value))
  }

  const parse = (s: string) => {
    return step !== undefined ? parseFloat(s) : parseInt(s)
  }

  return (
    <input
      type='number'
      min={min}
      step={step}
      required={required}
      title={title}
      aria-label={ariaLabel}
      value={raw}
      onChange={(e) => {
        setRaw(e.target.value)
        const parsed = parse(e.target.value)
        if (!isNaN(parsed)) {
          onChange(parsed)
        }
      }}
      onBlur={() => {
        const parsed = parse(raw)
        const final = isNaN(parsed) || parsed < min ? min : parsed
        setRaw(String(final))
        onChange(final)
      }}
      className={className}
    />
  )
}
