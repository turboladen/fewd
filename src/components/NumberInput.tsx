import { useEffect, useState } from 'react'

interface NumberInputProps {
  value: number
  onChange: (value: number) => void
  min?: number
  step?: number | string
  className?: string
  required?: boolean
  title?: string
}

export function NumberInput({
  value,
  onChange,
  min = 1,
  step,
  className = 'input-sm w-20',
  required,
  title,
}: NumberInputProps) {
  const [raw, setRaw] = useState(String(value))

  useEffect(() => {
    setRaw(String(value))
  }, [value])

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
